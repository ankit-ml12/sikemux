import { useEffect, useReducer, useState } from "react";
import { markdownApi } from "../api/markdown";
import { keepUnchanged } from "../lib/keepUnchanged";
import { swallow } from "../state/toast";
import type { MarkdownOptions, MdElement } from "./types";

/* A finished message is read once. A transcript scrolled back and forth mounts
   the same messages again, and they should draw in the frame they mount, so a
   whole long transcript is kept, bounded by how much text that is. */
const REMEMBERED_CHARS_PER_KIND = 4_000_000;
const remembered = new Map<string, { texts: Map<string, MdElement[]>; chars: number }>();

function kindOf(options: MarkdownOptions): string {
    return `${+options.gfm}${+options.htmlAsText}${+options.fileLinks}`;
}

function recall(options: MarkdownOptions, text: string): MdElement[] | undefined {
    return remembered.get(kindOf(options))?.texts.get(text);
}

function remember(options: MarkdownOptions, text: string, blocks: MdElement[]): void {
    const kind = kindOf(options);
    let shelf = remembered.get(kind);
    if (!shelf) remembered.set(kind, (shelf = { texts: new Map(), chars: 0 }));
    if (shelf.texts.delete(text)) shelf.chars -= text.length;
    shelf.texts.set(text, blocks);
    shelf.chars += text.length;
    for (const oldest of shelf.texts.keys()) {
        if (shelf.chars <= REMEMBERED_CHARS_PER_KIND || oldest === text) break;
        shelf.texts.delete(oldest);
        shelf.chars -= oldest.length;
    }
}

/* Read in slices, so a transcript of thousands of messages never holds one
   frame for a single huge answer from the parser. */
const WARM_SLICE = 64;

export interface MarkdownText {
    readonly text: string;
    readonly options: MarkdownOptions;
}

/** Reads finished texts ahead of anything drawing them, in the order given. */
export async function warmMarkdown(texts: readonly MarkdownText[]): Promise<void> {
    const unread = texts.filter(({ text, options }) => text && !recall(options, text));
    for (let start = 0; start < unread.length; start += WARM_SLICE) {
        const slice = unread.slice(start, start + WARM_SLICE).filter(({ text, options }) => !recall(options, text));
        const read = await Promise.all(slice.map(({ text, options }) => markdownApi.parse({ text, options, skip: 0 }).catch(() => null)));
        read.forEach((blocks, index) => {
            if (blocks) remember(slice[index].options, slice[index].text, blocks);
        });
    }
}

export function forgetMarkdownForTests(): void {
    remembered.clear();
}

/** Keeps every block that came back unchanged as the object already drawn, so it is not drawn again. */
export function settleBlocks(previous: readonly MdElement[] | undefined, skip: number, tail: readonly MdElement[]): MdElement[] {
    const blocks = previous ? previous.slice(0, skip) : [];
    tail.forEach((block, index) => {
        const old = previous?.[skip + index];
        blocks.push(old ? keepUnchanged(old, block) : block);
    });
    return blocks;
}

/* The line being written may still join the block before it, as a table row
   or a heading's underline does, so the last two blocks are read again. */
const UNSETTLED_BLOCKS = 2;

interface Wanted {
    readonly text: string;
    readonly options: MarkdownOptions;
    readonly live: boolean;
}

interface Read extends Wanted {
    readonly blocks: readonly MdElement[];
}

/* One read at a time per message. Text that arrives meanwhile waits for the
   next frame and is read as a whole, so a fast stream costs a read a frame. */
class Reader {
    alive = true;
    /** What the message was last read as, here or by whoever handed its blocks over. */
    held: Read | null = null;
    private wanted: Wanted | null = null;
    private busy = false;

    constructor(private readonly show: () => void) {}

    /** Blocks read elsewhere, which a read still on its way is older than and the next read grows from. */
    give(read: Read): void {
        this.held = read;
        this.wanted = null;
    }

    want(wanted: Wanted): void {
        this.wanted = wanted;
        if (!this.busy) this.next();
    }

    private next(): void {
        const wanted = this.wanted;
        if (!wanted) return;
        this.wanted = null;
        const previous = this.held;
        const growing = wanted.live && previous?.options === wanted.options && wanted.text.startsWith(previous.text);
        const skip = growing ? Math.max(0, previous.blocks.length - UNSETTLED_BLOCKS) : 0;
        this.busy = true;
        markdownApi
            .parse({ text: wanted.text, options: wanted.options, skip })
            .then(
                (tail) => settleBlocks(previous?.blocks, skip, tail),
                (error: unknown): MdElement[] => {
                    swallow("read markdown")(error);
                    return [{ t: "p", c: [wanted.text] }];
                },
            )
            .then((blocks) => {
                if (!wanted.live) remember(wanted.options, wanted.text, blocks);
                this.busy = false;
                if (this.held === previous) {
                    this.held = { ...wanted, blocks };
                    if (this.alive) this.show();
                }
                if (this.wanted)
                    requestAnimationFrame(() => {
                        if (!this.busy) this.next();
                    });
            });
    }
}

/**
 * The blocks `text` reads as, or null until the first read comes back. While
 * `live`, the message is still being written and only its end is read again.
 * `given` is the blocks someone else already read `text` as; when they stop
 * coming, the message keeps them on screen until its own read replaces them.
 */
export function useMarkdownBlocks(text: string, options: MarkdownOptions, live: boolean, given?: readonly MdElement[]): readonly MdElement[] | null {
    const known = given ?? recall(options, text);
    const [, shown] = useReducer((count: number) => count + 1, 0);
    const [reader] = useState(() => new Reader(shown));
    useEffect(() => {
        reader.alive = true;
        return () => {
            reader.alive = false;
        };
    }, [reader]);
    useEffect(() => {
        if (known) reader.give({ text, options, live, blocks: known });
        else reader.want({ text, options, live });
    }, [reader, known, text, options, live]);
    return known ?? reader.held?.blocks ?? null;
}
