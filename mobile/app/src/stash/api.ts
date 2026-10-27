import { Directory, File, Paths, UploadTask, UploadType } from 'expo-file-system';
import type { ApiError, StashItem, StashTarget } from '@protocol';
import { ApiProblem } from '@protocol/api';

import { api, type TokenSource } from '@/account/api';
import { apiUrl } from '@/account/config';
import type { Attachment } from '@/chat/uploads';

export type { StashItem, StashTarget };

/** The most the server keeps of one item. */
export const MAX_STASH_BYTES = 25 * 1024 * 1024;
/** The longest text the server takes as text; longer text goes as a file. */
export const MAX_STASH_TEXT = 16_000;

const OFFLINE = "Can't reach Sikemux. Check the phone is online.";

export const stashApi = (token: TokenSource) => api(token);

async function bearer(token: TokenSource): Promise<string> {
  const signedIn = await token();
  if (!signedIn) throw new ApiProblem('Sign in again.', 401);
  return signedIn;
}

function failure(status: number, body: string): ApiProblem {
  let detail: Partial<ApiError> | null = null;
  try {
    detail = JSON.parse(body) as Partial<ApiError>;
  } catch {}
  return new ApiProblem(detail?.error?.message ?? `The accounts server answered ${status}.`, status, detail?.error);
}

/** Stashes a file from this phone as its bytes, reporting how much has gone. */
export async function stashFile(
  token: TokenSource,
  file: { uri: string; name: string; mime: string },
  extra: { note?: string; target?: StashTarget },
  options: { onProgress?: (share: number) => void; signal?: AbortSignal } = {},
): Promise<StashItem> {
  const headers: Record<string, string> = {
    authorization: `Bearer ${await bearer(token)}`,
    'content-type': file.mime,
    'x-stash-name': encodeURIComponent(file.name),
  };
  if (extra.note) headers['x-stash-note'] = encodeURIComponent(extra.note);
  if (extra.target) headers['x-stash-target'] = encodeURIComponent(JSON.stringify(extra.target));
  const task = new UploadTask(new File(file.uri), `${apiUrl()}/v1/stash`, {
    httpMethod: 'POST',
    uploadType: UploadType.BINARY_CONTENT,
    headers,
    signal: options.signal,
    onProgress: ({ bytesSent, totalBytes }) => {
      if (totalBytes > 0) options.onProgress?.(bytesSent / totalBytes);
    },
  });
  let answer;
  try {
    answer = await task.uploadAsync();
  } catch (error) {
    if (options.signal?.aborted) throw error;
    throw new ApiProblem(OFFLINE);
  } finally {
    task.release();
  }
  if (answer.status < 200 || answer.status >= 300) throw failure(answer.status, answer.body);
  return JSON.parse(answer.body) as StashItem;
}

const DRAWN = new Set(['image/jpeg', 'image/png', 'image/gif', 'image/webp']);

/** Brings a stashed file down to this phone, ready to go with a message like one picked here. */
export async function fetchStashFile(token: TokenSource, item: StashItem): Promise<Attachment> {
  const folder = new Directory(Paths.cache, 'stash', item.id);
  if (!folder.exists) folder.create({ intermediates: true });
  const name = item.name ?? 'file';
  const destination = new File(folder, name);
  if (!destination.exists) {
    try {
      await File.downloadFileAsync(`${apiUrl()}/v1/stash/${encodeURIComponent(item.id)}/file`, destination, {
        headers: { authorization: `Bearer ${await bearer(token)}` },
      });
    } catch (error) {
      if (error instanceof ApiProblem) throw error;
      throw new ApiProblem(/\b[45]\d\d\b/.test(String(error)) ? "Couldn't get this from your stash." : OFFLINE);
    }
  }
  const mime = item.mime ?? 'application/octet-stream';
  return {
    id: `stash-${item.id}`,
    kind: DRAWN.has(mime) ? 'image' : 'file',
    name,
    mime,
    uri: destination.uri,
    size: destination.size ?? item.size ?? 0,
    stashId: item.id,
  };
}
