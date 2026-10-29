/** The most the server keeps of one stashed file. */
export const MAX_STASH_FILE_BYTES = 25 * 1024 * 1024;
/** The most the server keeps of one account's stashed files together. */
export const MAX_STASH_ACCOUNT_BYTES = 1024 * 1024 * 1024;
/** The longest text the server takes as text or a prompt, as StashAddText and StashAddPrompt say; longer text goes as a file. */
export const MAX_STASH_TEXT = 16_000;
