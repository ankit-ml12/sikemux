import { useSyncExternalStore } from 'react';
import { AppState, Platform, type AppStateStatus } from 'react-native';
import * as LocalAuthentication from 'expo-local-authentication';

/** How long the app may be away before it asks again, as password managers allow a quick switch to another app. */
export const AWAY_MS = 60_000;

/**
 * open: showing the app. covered: away or behind a system sheet, so the app switcher's picture shows nothing.
 * locked: waiting for Face ID, Touch ID or the passcode. noPasscode: the phone has no lock to ask for.
 * `opened` turns true the first time the app shows itself after it starts.
 */
export type LockState = { phase: 'open' | 'covered' | 'locked' | 'noPasscode'; asking: boolean; opened: boolean };

const guarded = Platform.OS !== 'web';
const active = () => AppState.currentState === 'active';

let state: LockState = guarded
  ? { phase: active() ? 'locked' : 'covered', asking: false, opened: false }
  : { phase: 'open', asking: false, opened: true };
let locked = guarded;
/** Undefined until the account is known at launch. */
let signedIn: boolean | undefined;
let leftAt: number | undefined;
const listeners = new Set<() => void>();
const waiting = new Set<() => void>();

function set(next: Partial<LockState>) {
  state = { ...state, ...next };
  listeners.forEach((listener) => listener());
  if (state.phase !== 'open') return;
  waiting.forEach((resolve) => resolve());
  waiting.clear();
}

function subscribe(listener: () => void) {
  listeners.add(listener);
  return () => listeners.delete(listener);
}

function open() {
  locked = false;
  leftAt = undefined;
  set({ phase: active() ? 'open' : 'covered', asking: false, opened: true });
}

export function useLock(): LockState {
  return useSyncExternalStore(subscribe, () => state);
}

/** Resolves once the app shows itself again, so a link opened from outside waits for the lock. */
export function whenOpen(): Promise<void> {
  if (state.phase === 'open') return Promise.resolve();
  return new Promise((resolve) => waiting.add(resolve));
}

/**
 * The lock keeps an account's hosts and chats. A phone signed in when the app starts asks first; signed out
 * there is nothing behind it, and someone who signs in on the phone has just proved who they are.
 */
export function lockFor(account: boolean) {
  if (!guarded || account === signedIn) return;
  const launched = signedIn === undefined;
  signedIn = account;
  if (account && launched) return;
  open();
}

/** Asks for Face ID, Touch ID or the phone's passcode, and opens the app once the person passes. */
export async function unlock(): Promise<void> {
  if (!locked || state.asking || !active()) return;
  set({ asking: true });
  const result = await LocalAuthentication.authenticateAsync({ promptMessage: 'Unlock Sikemux' }).catch(() => null);
  if (result?.success) {
    open();
    return;
  }
  const noLock = result?.error === 'passcode_not_set' || result?.error === 'not_enrolled';
  set({ phase: noLock ? 'noPasscode' : 'locked', asking: false });
}

function changed(next: AppStateStatus) {
  if (state.asking) return;
  if (next !== 'active') {
    if (next === 'background') leftAt ??= Date.now();
    if (!locked) set({ phase: 'covered' });
    return;
  }
  if (signedIn && leftAt !== undefined && Date.now() - leftAt >= AWAY_MS) locked = true;
  leftAt = undefined;
  if (!locked) {
    set({ phase: 'open' });
    return;
  }
  set({ phase: 'locked' });
  void unlock();
}

if (guarded) AppState.addEventListener('change', changed);
