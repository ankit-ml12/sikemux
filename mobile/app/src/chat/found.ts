import { createContext } from 'react';

import type { MdElement, MdNode } from '@shared/markdown/types';

/** A place the host found, as UTF-16 offsets in the words its part shows. */
export type Range = readonly [number, number];

/** Words the find bar found, drawn marked; `current` is the place the reader is on. */
export type FoundNode = { t: 'found'; c: [string]; current: boolean };

/** Inline code holding places found. */
export type FoundCode = { t: 'code'; v: string; found: readonly (readonly [number, number, boolean])[] };

/** What the find bar found in the rows, by the text part each place is in, and the place the reader is on. */
export type Finding = { byPart: ReadonlyMap<string, readonly Range[]>; current: { part: string; range: Range } | null };

export const FindingContext = createContext<Finding | null>(null);

const SEPARATED = new Set(['ul', 'ol', 'quote', 'details']);

/* The host counts a part's words as its tree shows them: top-level blocks and the entries of lists,
   quotes and folds a line apart, code by its text, pictures and rules as nothing. */
function children(node: MdElement): readonly MdNode[] {
  if ('c' in node && Array.isArray(node.c)) return node.c;
  if (node.t === 'table') return [...(node.head ?? []), ...node.rows.flat()].flat();
  return [];
}

function shownLength(node: MdNode): number {
  if (typeof node === 'string') return node.length;
  if (node.t === 'code' || node.t === 'pre') return node.v.length;
  if (node.t === 'br') return 1;
  if (node.t === 'img' || node.t === 'hr' || node.t === 'fnref') return 0;
  const inner = children(node);
  const gaps = SEPARATED.has(node.t) ? Math.max(inner.length - 1, 0) : 0;
  return inner.reduce((sum, child) => sum + shownLength(child), gaps);
}

type Marking = { ranges: readonly Range[]; current: Range | null };

function within(marking: Marking, from: number, to: number) {
  return marking.ranges.filter(([start, end]) => start < to && end > from);
}

function markString(text: string, at: number, marking: Marking): MdNode[] {
  const hits = within(marking, at, at + text.length);
  if (!hits.length) return [text];
  const out: MdNode[] = [];
  let cursor = 0;
  for (const [start, end] of hits) {
    const from = Math.max(start - at, cursor);
    const to = Math.min(end - at, text.length);
    if (from > cursor) out.push(text.slice(cursor, from));
    if (to > from) out.push({ t: 'found', c: [text.slice(from, to)], current: marking.current?.[0] === start } as unknown as MdElement);
    cursor = Math.max(cursor, to);
  }
  if (cursor < text.length) out.push(text.slice(cursor));
  return out;
}

/* The top-level line between blocks comes only once some words are shown. */
function markNodes(nodes: readonly MdNode[], at: number, marking: Marking, separated: boolean, top = false): MdNode[] {
  let cursor = at;
  return nodes.flatMap((node, index) => {
    if (separated && index > 0 && (!top || cursor > at)) cursor += 1;
    const start = cursor;
    cursor += shownLength(node);
    return markNode(node, start, marking);
  });
}

function markNode(node: MdNode, at: number, marking: Marking): MdNode[] {
  if (typeof node === 'string') return markString(node, at, marking);
  const length = shownLength(node);
  if (!within(marking, at, at + length).length) return [node];
  if (node.t === 'code') {
    const found = within(marking, at, at + length).map(
      ([start, end]) => [Math.max(start - at, 0), Math.min(end - at, length), marking.current?.[0] === start] as const,
    );
    return [{ ...node, found } as FoundCode as unknown as MdElement];
  }
  if (node.t === 'table') {
    let cursor = at;
    const cells = (row: MdNode[][]) =>
      row.map((cell) => {
        const start = cursor;
        cursor += cell.reduce((sum, child) => sum + shownLength(child), 0);
        return markNodes(cell, start, marking, false);
      });
    const head = node.head && cells(node.head);
    return [{ ...node, head, rows: node.rows.map(cells) }];
  }
  if ('c' in node && Array.isArray(node.c)) {
    return [{ ...node, c: markNodes(node.c, at, marking, SEPARATED.has(node.t)) } as MdElement];
  }
  return [node];
}

/** `blocks` with the places found in their part marked, each untouched block kept as the object it was. */
export function markBlocks(blocks: MdElement[], ranges: readonly Range[] | undefined, current: Range | null): MdElement[] {
  if (!ranges?.length) return blocks;
  return markNodes(blocks, 0, { ranges, current }, true, true) as MdElement[];
}

function shownWords(node: MdNode): string {
  if (typeof node === 'string') return node;
  if (node.t === 'code' || node.t === 'pre') return node.v;
  if (node.t === 'br') return '\n';
  if (node.t === 'img' || node.t === 'hr' || node.t === 'fnref') return '';
  return children(node)
    .map(shownWords)
    .join(SEPARATED.has(node.t) ? '\n' : '');
}

/** The words a part's tree shows, as the host finds in them. */
function shownText(blocks: readonly MdNode[]): string {
  return blocks.reduce<string>((shown, block) => (shown ? `${shown}\n${shownWords(block)}` : shownWords(block)), '');
}

/** A sent message's text in runs, with the places found in its tree's words marked where they stand in the text as typed. */
export function markTyped(
  text: string,
  blocks: readonly MdElement[] | undefined,
  ranges: readonly Range[] | undefined,
  current: Range | null,
) {
  if (!ranges?.length) return [{ text, found: false, current: false }];
  const shown = blocks ? shownText(blocks) : text;
  // Markup only drops characters, so each one shown is the next match in the text as typed.
  const typed: number[] = [];
  let cursor = 0;
  for (const char of shown) {
    const at = text.indexOf(char, cursor);
    const step = char.length;
    for (let unit = 0; unit < step; unit += 1) typed.push(at < 0 ? cursor : at + unit);
    if (at >= 0) cursor = at + step;
  }
  typed.push(cursor);
  const runs: { text: string; found: boolean; current: boolean }[] = [];
  let last = 0;
  for (const [start, end] of ranges) {
    const from = typed[start] ?? text.length;
    const to = typed[end - 1] === undefined ? text.length : typed[end - 1] + 1;
    if (from < last) continue;
    if (from > last) runs.push({ text: text.slice(last, from), found: false, current: false });
    runs.push({ text: text.slice(from, to), found: true, current: current?.[0] === start });
    last = to;
  }
  if (last < text.length) runs.push({ text: text.slice(last), found: false, current: false });
  return runs;
}
