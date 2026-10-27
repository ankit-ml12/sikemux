import { useEffect, useRef, useSyncExternalStore } from 'react';
import { useAuth } from '@clerk/expo';

import type { TokenSource } from '@/account/api';
import { problem as problemOf } from '@/core/problem';
import { stashApi, type StashItem } from './api';

/** What the phone knows of the stash: `loading` until the first read, `offline` while the last read failed. */
export type Stash = { items: StashItem[]; state: 'loading' | 'ready' | 'offline'; problem?: string };

const EMPTY: Stash = { items: [], state: 'loading' };
/** Live events often come in a burst, such as a file and then its link preview; one read follows them. */
const SETTLE_MS = 300;

let now = EMPTY;
let token: TokenSource | undefined;
let reading = 0;
let settling: ReturnType<typeof setTimeout> | undefined;
const listeners = new Set<() => void>();

function publish(next: Stash) {
  now = next;
  listeners.forEach((listener) => listener());
}

function subscribe(listener: () => void) {
  listeners.add(listener);
  return () => listeners.delete(listener);
}

/** Reads the untaken stash again; what was shown stays while it does. */
export async function refreshStash() {
  if (!token) return;
  const read = (reading += 1);
  if (now.state === 'offline' && !now.items.length) publish(EMPTY);
  try {
    const items: StashItem[] = [];
    let cursor: string | undefined;
    do {
      const page = await stashApi(token).listStash({ taken: 'false', ...(cursor ? { cursor } : {}) });
      items.push(...page.items);
      cursor = page.cursor;
    } while (cursor);
    if (read === reading) publish({ items, state: 'ready' });
  } catch (error) {
    if (read === reading) publish({ items: now.items, state: 'offline', problem: problemOf(error) });
  }
}

/** The account's stash changed somewhere: read it again once the burst of changes settles. */
export function stashChanged() {
  clearTimeout(settling);
  settling = setTimeout(() => void refreshStash(), SETTLE_MS);
}

/** Takes items out of the stash on this phone at once, and on the server as used; one the server refused comes back on the next read. */
export async function takeStash(ids: readonly string[]) {
  if (!ids.length || !token) return;
  const signedIn = token;
  publish({ ...now, items: now.items.filter((item) => !ids.includes(item.id)) });
  await Promise.allSettled(ids.map((id) => stashApi(signedIn).updateStash(id, { taken: true })));
}

export async function removeStash(id: string) {
  if (!token) return;
  const before = now.items;
  publish({ ...now, items: before.filter((item) => item.id !== id) });
  try {
    await stashApi(token).deleteStash(id);
  } catch (error) {
    publish({ ...now, items: before });
    throw error;
  }
}

export function useStash(): Stash {
  return useSyncExternalStore(subscribe, () => now);
}

export function stashToken(): TokenSource | undefined {
  return token;
}

/** Reads the stash on signing in, so the composer's + can count it, and forgets it on signing out. */
export function useStashFeed() {
  const { isSignedIn, getToken } = useAuth();
  const latestGetToken = useRef(getToken);
  useEffect(() => {
    latestGetToken.current = getToken;
  });
  useEffect(() => {
    if (!isSignedIn) {
      token = undefined;
      publish(EMPTY);
      return;
    }
    token = () => latestGetToken.current();
    void refreshStash();
  }, [isSignedIn]);
}
