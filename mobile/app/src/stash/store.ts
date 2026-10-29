import { useEffect, useRef } from 'react';
import { useAuth } from '@clerk/expo';

import { externalValue, useExternalValue } from '@shared/lib/externalValue';

import type { TokenSource } from '@/account/api';
import { problem as problemOf } from '@/core/problem';
import { stashApi, type StashItem } from './api';

/**
 * What the phone knows of the stash: `loading` until the first read, `offline` while the last read failed.
 * `seenAt` is when the person last looked at it on any device; null before they ever have.
 */
export type Stash = { items: StashItem[]; seenAt: string | null; state: 'loading' | 'ready' | 'offline'; problem?: string };

const EMPTY: Stash = { items: [], seenAt: null, state: 'loading' };
/** Live events often come in a burst, such as a file and then its link preview; one read follows them. */
const SETTLE_MS = 300;

const stash = externalValue(EMPTY);
let token: TokenSource | undefined;
let reading = 0;
let settling: ReturnType<typeof setTimeout> | undefined;

/** Reads the untaken stash again; what was shown stays while it does. */
export async function refreshStash() {
  if (!token) return;
  const read = (reading += 1);
  const shown = stash.get();
  if (shown.state === 'offline' && !shown.items.length) stash.set(EMPTY);
  try {
    const items: StashItem[] = [];
    let seenAt: string | null = null;
    let cursor: string | undefined;
    do {
      const page = await stashApi(token).listStash({ taken: 'false', ...(cursor ? { cursor } : {}) });
      items.push(...page.items);
      seenAt = page.seenAt;
      cursor = page.cursor;
    } while (cursor);
    if (read === reading) stash.set({ items, seenAt, state: 'ready' });
  } catch (error) {
    if (read === reading) stash.set({ ...stash.get(), state: 'offline', problem: problemOf(error) });
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
  const now = stash.get();
  stash.set({ ...now, items: now.items.filter((item) => !ids.includes(item.id)) });
  await Promise.allSettled(ids.map((id) => stashApi(signedIn).updateStash(id, { taken: true })));
}

export async function removeStash(id: string) {
  if (!token) return;
  const before = stash.get().items;
  stash.set({ ...stash.get(), items: before.filter((item) => item.id !== id) });
  try {
    await stashApi(token).deleteStash(id);
  } catch (error) {
    stash.set({ ...stash.get(), items: before });
    throw error;
  }
}

/** The items stashed since the person last looked at the stash, newest first. */
export function newInStash(stash: Stash): StashItem[] {
  const seen = stash.seenAt === null ? null : Date.parse(stash.seenAt);
  return seen === null ? stash.items : stash.items.filter((item) => Date.parse(item.createdAt) > seen);
}

/** The person has looked at the stash: what it holds now is no longer new, on any device. */
export async function seeStash() {
  const now = stash.get();
  const newest = now.items[0]?.createdAt;
  if (!token || !newest || !newInStash(now).length) return;
  stash.set({ ...now, seenAt: newest });
  try {
    const { at } = await stashApi(token).markStashSeen({ at: newest });
    stash.set({ ...stash.get(), seenAt: at });
  } catch (error) {
    console.warn('sikemux: could not mark the stash seen on the account', error);
  }
}

/** The stash as the phone knows it now, outside a render. */
export function stashNow(): Stash {
  return stash.get();
}

export function useStash(): Stash {
  return useExternalValue(stash);
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
      stash.set(EMPTY);
      return;
    }
    token = () => latestGetToken.current();
    void refreshStash();
  }, [isSignedIn]);
}
