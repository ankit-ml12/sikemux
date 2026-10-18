import * as SecureStore from 'expo-secure-store';
import type { TokenCache } from '@clerk/expo';

/** Readable while the phone is locked, since a notification can wake the app then, but never restored to another phone. */
const OPTIONS = { keychainAccessible: SecureStore.AFTER_FIRST_UNLOCK_THIS_DEVICE_ONLY };

/** Clerk's sign-in, kept in this phone's Keychain or Keystore: an iPhone restored from a backup signs in again. */
export const tokenCache: TokenCache = {
  getToken: (key) => SecureStore.getItemAsync(key, OPTIONS).catch(() => null),
  async saveToken(key, token) {
    // SecureStore keeps an existing item's old accessibility when it overwrites it.
    await SecureStore.deleteItemAsync(key, OPTIONS);
    await SecureStore.setItemAsync(key, token, OPTIONS);
  },
  clearToken: (key) => SecureStore.deleteItemAsync(key, OPTIONS),
};
