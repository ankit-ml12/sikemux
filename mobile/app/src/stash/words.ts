import { formatAge, formatBytes } from '@shared/lib/format';

import type { StashItem } from './api';

export function siteOf(url: string): string {
  return url.replace(/^[a-z]+:\/\//i, '').split(/[/?#]/)[0] ?? url;
}

/** What an item is called in a list: a page's title, text's first line, or a file's name. */
export function itemTitle(item: StashItem): string {
  if (item.kind === 'link') return item.preview?.title || (item.url ?? '').replace(/^https?:\/\//, '');
  if (item.kind === 'text') return (item.text ?? '').trim().split('\n')[0] ?? '';
  return item.name ?? 'file';
}

/** The line under it: where a link goes or what the item is, then how long ago it was stashed. */
export function itemPlace(item: StashItem): string {
  const what =
    item.kind === 'link' ? item.preview?.site || siteOf(item.url ?? '') : item.kind === 'text' ? 'Text' : formatBytes(item.size ?? 0);
  const age = formatAge(item.createdAt);
  return `${what} · ${age === 'now' ? 'just now' : age}`;
}

/** What a link or text puts in the message. */
export function itemText(item: StashItem): string {
  return item.kind === 'link' ? (item.url ?? '') : (item.text ?? '');
}

export const carriesFile = (item: StashItem) => item.kind === 'image' || item.kind === 'video' || item.kind === 'file';
