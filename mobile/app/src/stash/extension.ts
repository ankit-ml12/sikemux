import { useEffect } from 'react';
import { useAuth } from '@clerk/expo';

import { apiUrl } from '@/account/config';
import { deviceKeyHex } from '@/phone/identity';
import { stashNative } from '../../modules/stash';
import { onShareChats, shareChatsNow, type ShareChats } from './chats';

/** Hosts change their chats in bursts; the extension's copy follows once they settle. */
const SETTLE_MS = 1000;

/**
 * Keeps what the iOS share extension needs where it can read it while signed in: the chats used last,
 * where the accounts server is, and this phone's key. Signing out takes them all away.
 */
export function useShareExtension() {
  const { isSignedIn } = useAuth();
  useEffect(() => {
    const native = stashNative;
    if (!native?.setShared || !native.setDeviceKey) return;
    if (!isSignedIn) {
      native.setShared(null);
      native.setDeviceKey(null);
      return;
    }
    deviceKeyHex()
      .then((key) => native.setDeviceKey?.(key))
      .catch((error: unknown) => console.warn('sikemux: could not hand the share extension the key of this phone', error));
    let written = '';
    const write = (chats: ShareChats) => {
      // Hosts not reached yet since launch know no chats; the last ones written stay until one is.
      if (!chats.chats.length && written) return;
      const json = JSON.stringify({ apiUrl: apiUrl(), ...chats });
      if (json === written) return;
      written = json;
      native.setShared?.(json);
    };
    write(shareChatsNow());
    let settling: ReturnType<typeof setTimeout> | undefined;
    const stop = onShareChats((chats) => {
      clearTimeout(settling);
      settling = setTimeout(() => write(chats), SETTLE_MS);
    });
    return () => {
      stop();
      clearTimeout(settling);
    };
  }, [isSignedIn]);
}
