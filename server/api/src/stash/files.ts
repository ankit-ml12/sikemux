import {
  createCipheriv,
  createDecipheriv,
  createHash,
  hkdfSync,
  randomBytes,
} from "node:crypto";
import { open, readdir, rename, rm, stat } from "node:fs/promises";
import { join } from "node:path";
import { pipeline, Readable } from "node:stream";

import { ApiFailure } from "../http.ts";
import type { Logger } from "../log.ts";
import { ITEM_ID } from "./items.ts";

const VERSION = Buffer.from([1]);
const NONCE_BYTES = 12;
const TAG_BYTES = 16;
const HEADER_BYTES = VERSION.length + NONCE_BYTES;
const PART = /^\.([A-Za-z0-9_-]{22})\.part$/;
/** How old a file without an item must be before a sweep deletes it, so uploads in flight survive. */
const SWEEP_GRACE_MS = 60 * 60 * 1000;

export interface Written {
  size: number;
  sha256: string;
}

function pathOf(dir: string, id: string): string {
  if (!ITEM_ID.test(id)) throw new Error(`${id} is not a stash item id`);
  return join(dir, id);
}

/**
 * Stashed files on disk, each named by its item's id. Every file is encrypted with AES-256-GCM
 * under its own key, derived from the server's key and the id: one version byte, the nonce, the
 * ciphertext, then the tag.
 */
export class StashFiles {
  readonly dir: string;
  private readonly key: Buffer;

  constructor({ dir, key }: { dir: string; key: Buffer }) {
    this.dir = dir;
    this.key = key;
  }

  private keyFor(id: string): Buffer {
    return Buffer.from(
      hkdfSync("sha256", this.key, "", `sikemux-stash|${id}`, 32),
    );
  }

  private partOf(id: string): string {
    pathOf(this.dir, id);
    return join(this.dir, `.${id}.part`);
  }

  /** Encrypts `body` into a file that `commit` makes the item's, refusing it past `maxBytes`. */
  async write(
    id: string,
    body: AsyncIterable<Uint8Array>,
    maxBytes: number,
  ): Promise<Written> {
    const part = this.partOf(id);
    const file = await open(part, "wx");
    const hash = createHash("sha256");
    const nonce = randomBytes(NONCE_BYTES);
    const cipher = createCipheriv("aes-256-gcm", this.keyFor(id), nonce);
    let size = 0;
    try {
      // The backup reads stashed files through the folder's group, which the process's umask hides.
      await file.chmod(0o640);
      await file.write(Buffer.concat([VERSION, nonce]));
      for await (const chunk of body) {
        size += chunk.byteLength;
        if (size > maxBytes)
          throw new ApiFailure(
            413,
            "payload_too_large",
            `A stashed file is limited to ${maxBytes} bytes.`,
          );
        hash.update(chunk);
        await file.write(cipher.update(chunk));
      }
      await file.write(Buffer.concat([cipher.final(), cipher.getAuthTag()]));
      await file.datasync();
    } catch (error) {
      await file.close();
      await rm(part, { force: true });
      throw error;
    }
    await file.close();
    return { size, sha256: hash.digest("hex") };
  }

  async commit(id: string): Promise<void> {
    await rename(this.partOf(id), pathOf(this.dir, id));
  }

  async discard(id: string): Promise<void> {
    await rm(this.partOf(id), { force: true });
  }

  /** The item's file, decrypted as it is read; null when there is none. A tampered file fails the stream at its end. */
  async read(id: string, log: Logger): Promise<ReadableStream | null> {
    const file = await open(pathOf(this.dir, id), "r").catch(
      (error: NodeJS.ErrnoException) => {
        if (error.code === "ENOENT") return null;
        throw error;
      },
    );
    if (!file) return null;
    try {
      const { size } = await file.stat();
      const length = size - HEADER_BYTES - TAG_BYTES;
      if (length < 0) throw new Error(`stash file ${id} is cut short`);
      const header = Buffer.alloc(HEADER_BYTES);
      const tag = Buffer.alloc(TAG_BYTES);
      await file.read(header, 0, HEADER_BYTES, 0);
      await file.read(tag, 0, TAG_BYTES, size - TAG_BYTES);
      if (header[0] !== VERSION[0])
        throw new Error(`stash file ${id} has an unknown version`);
      const decipher = createDecipheriv(
        "aes-256-gcm",
        this.keyFor(id),
        header.subarray(VERSION.length),
      );
      decipher.setAuthTag(tag);
      const source =
        length > 0
          ? file.createReadStream({
              start: HEADER_BYTES,
              end: HEADER_BYTES + length - 1,
            })
          : Readable.from([]);
      if (length === 0) await file.close();
      pipeline(source, decipher, (error) => {
        if (error)
          log.error({ err: error, id }, "a stashed file failed to read");
      });
      return Readable.toWeb(decipher) as ReadableStream;
    } catch (error) {
      await file.close();
      throw error;
    }
  }
}

/** Whether every id's file decrypts whole, as a backup is checked; logs each that does not. */
export async function checkStashFiles(
  files: StashFiles,
  ids: readonly string[],
  log: Logger,
): Promise<boolean> {
  let whole = true;
  for (const id of ids) {
    try {
      const stream = await files.read(id, log);
      if (!stream) throw new Error("the file is missing");
      for await (const _ of stream);
    } catch (error) {
      log.error({ err: error, id }, "a stashed file does not decrypt");
      whole = false;
    }
  }
  return whole;
}

export async function removeStashFiles(
  dir: string,
  ids: readonly string[],
): Promise<void> {
  await Promise.all(ids.map((id) => rm(pathOf(dir, id), { force: true })));
}

/**
 * Deletes files no item holds any more: those of items deleted while their file was being
 * removed, of purged accounts, and uploads that never finished.
 */
export async function sweepStashFiles(
  dir: string,
  held: (ids: string[]) => Promise<Set<string>>,
  log: Logger,
): Promise<number> {
  const names = await readdir(dir).catch((error: NodeJS.ErrnoException) => {
    if (error.code === "ENOENT") return [];
    throw error;
  });
  const cutoff = Date.now() - SWEEP_GRACE_MS;
  const old = async (name: string) =>
    (await stat(join(dir, name)).catch(() => null))?.mtimeMs ?? Infinity;
  const stale: string[] = [];
  const candidates: string[] = [];
  for (const name of names) {
    if (PART.test(name)) {
      if ((await old(name)) < cutoff) stale.push(name);
    } else if (ITEM_ID.test(name)) candidates.push(name);
  }
  for (let start = 0; start < candidates.length; start += 1_000) {
    const batch = candidates.slice(start, start + 1_000);
    const kept = await held(batch);
    for (const id of batch)
      if (!kept.has(id) && (await old(id)) < cutoff) stale.push(id);
  }
  await Promise.all(stale.map((name) => rm(join(dir, name), { force: true })));
  if (stale.length) log.info({ files: stale.length }, "swept stashed files");
  return stale.length;
}
