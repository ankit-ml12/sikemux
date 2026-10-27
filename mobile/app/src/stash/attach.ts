import type { Attachment } from '@/chat/uploads';
import { fetchStashFile, type StashItem } from './api';
import { stashToken } from './store';
import { carriesFile, itemText } from './words';

/** A stash file brought down to this phone, for a chat that does not exist yet to take. */
export function downloadedFile(item: StashItem): Promise<Attachment> {
  const token = stashToken();
  if (!token) return Promise.reject(new Error('Sign in again.'));
  return fetchStashFile(token, item);
}

/**
 * What picking stash items does to a message: each item's note, then its link or text, join its words,
 * as the host puts them in a draft, and `file` gets each file ready to go with it. `words` are the ids
 * of items with no file, which leave the stash with the message's words.
 */
export function intoMessage(
  items: readonly StashItem[],
  draft: string,
  file: (item: StashItem) => Promise<Attachment>,
): { draft: string; words: string[]; files: Promise<Attachment[]> | null } {
  const said = items.filter((item) => !carriesFile(item));
  const filed = items.filter(carriesFile);
  const added = items.flatMap((item) => [item.note?.trim(), itemText(item).trim()]).filter(Boolean);
  return {
    draft: added.length ? [draft.trimEnd(), ...added].filter(Boolean).join('\n\n') : draft,
    words: said.map((item) => item.id),
    files: filed.length ? Promise.all(filed.map(file)) : null,
  };
}

/** The stash items a message carries, which leave the stash once it is sent. */
export function stashIdsOf(attachments: readonly Attachment[], words: readonly string[]): string[] {
  return [...attachments.flatMap((attachment) => (attachment.stashId ? [attachment.stashId] : [])), ...words];
}
