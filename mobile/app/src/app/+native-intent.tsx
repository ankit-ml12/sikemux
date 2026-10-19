import { whenOpen } from '@/lock/lock';

/** A link opened from outside waits until the app is unlocked, so it can't open a chat or start anything behind the lock. */
export async function redirectSystemPath({ path, initial }: { path: string; initial: boolean }): Promise<string> {
  if (!initial) await whenOpen();
  return path;
}
