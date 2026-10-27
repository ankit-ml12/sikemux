import type { LookupFunction } from "node:net";
import { BlockList, isIPv4, isIPv6 } from "node:net";
import { lookup } from "node:dns/promises";
import http from "node:http";
import https from "node:https";
import type { StashPreview } from "@sikemux/protocol";

const TIMEOUT_MS = 5_000;
const MAX_BYTES = 1024 * 1024;
const MAX_REDIRECTS = 3;

const blockedV4 = new BlockList();
for (const [network, prefix] of [
  ["0.0.0.0", 8],
  ["10.0.0.0", 8],
  ["100.64.0.0", 10],
  ["127.0.0.0", 8],
  ["169.254.0.0", 16],
  ["172.16.0.0", 12],
  ["192.0.0.0", 24],
  ["192.0.2.0", 24],
  ["192.88.99.0", 24],
  ["192.168.0.0", 16],
  ["198.18.0.0", 15],
  ["198.51.100.0", 24],
  ["203.0.113.0", 24],
  ["224.0.0.0", 4],
  ["240.0.0.0", 4],
] as const)
  blockedV4.addSubnet(network, prefix, "ipv4");

const globalV6 = new BlockList();
globalV6.addSubnet("2000::", 3, "ipv6");
const blockedV6 = new BlockList();
for (const [network, prefix] of [
  ["2001::", 32],
  ["2001:db8::", 32],
  ["2002::", 16],
] as const)
  blockedV6.addSubnet(network, prefix, "ipv6");

/** Whether `address` is on the public internet, not this machine, a private network or a special range. */
export function isPublicAddress(address: string): boolean {
  const mapped = /^::ffff:(\d+\.\d+\.\d+\.\d+)$/i.exec(address)?.[1];
  if (mapped) return isPublicAddress(mapped);
  if (isIPv4(address)) return !blockedV4.check(address, "ipv4");
  if (isIPv6(address))
    return globalV6.check(address, "ipv6") && !blockedV6.check(address, "ipv6");
  return false;
}

/** Resolves a host as the connection does, refusing it unless every address it has is public. */
const publicLookup: LookupFunction = (hostname, options, callback) => {
  lookup(hostname, { all: true, verbatim: true })
    .then((addresses) => {
      const first = addresses[0];
      if (!first || addresses.some(({ address }) => !isPublicAddress(address)))
        throw new Error(`${hostname} is not a public address`);
      if (options.all) callback(null, addresses);
      else callback(null, first.address, first.family);
    })
    .catch((error: NodeJS.ErrnoException) => callback(error, "", 0));
};

interface Page {
  url: URL;
  html: string;
}

function fetchOnce(url: URL, signal: AbortSignal): Promise<Page | URL> {
  return new Promise((resolve, reject) => {
    const client = url.protocol === "https:" ? https : http;
    const request = client.get(
      url,
      {
        lookup: publicLookup,
        signal,
        headers: {
          accept: "text/html,application/xhtml+xml",
          "accept-encoding": "identity",
          "user-agent": "Sikemux-Preview/1 (+https://sikemux.com)",
        },
      },
      (response) => {
        const status = response.statusCode ?? 0;
        const location = response.headers.location;
        if (status >= 300 && status < 400 && location) {
          response.resume();
          resolve(new URL(location, url));
          return;
        }
        const type = response.headers["content-type"] ?? "";
        if (
          status !== 200 ||
          !/^(text\/html|application\/xhtml\+xml)/i.test(type)
        ) {
          response.resume();
          reject(new Error(`not a page: ${status} ${type}`));
          return;
        }
        const chunks: Buffer[] = [];
        let size = 0;
        const done = () =>
          resolve({ url, html: Buffer.concat(chunks).toString("utf8") });
        response.on("data", (chunk: Buffer) => {
          const room = MAX_BYTES - size;
          chunks.push(chunk.subarray(0, room));
          size += Math.min(chunk.length, room);
          if (size >= MAX_BYTES) {
            response.destroy();
            done();
          }
        });
        response.on("end", done);
        response.on("error", reject);
      },
    );
    request.on("error", reject);
  });
}

/**
 * Whether the server may fetch `url` at all: http or https on their usual ports. A connection to
 * an address written in the link skips the lookup, so such an address is checked here.
 */
export function isFetchable(url: URL): boolean {
  const literal = url.hostname.replace(/^\[(.*)\]$/, "$1");
  return (
    (url.protocol === "http:" || url.protocol === "https:") &&
    url.port === "" &&
    url.username === "" &&
    url.password === "" &&
    (!(isIPv4(literal) || isIPv6(literal)) || isPublicAddress(literal))
  );
}

const ENTITIES: Record<string, string> = {
  amp: "&",
  lt: "<",
  gt: ">",
  quot: '"',
  apos: "'",
  nbsp: " ",
};

function decode(text: string): string {
  return text
    .replace(/&(#x[0-9a-f]+|#\d+|[a-z]+);/gi, (entity, name: string) => {
      if (name[0] === "#") {
        const code =
          name[1]?.toLowerCase() === "x"
            ? parseInt(name.slice(2), 16)
            : parseInt(name.slice(1), 10);
        return code > 0 && code <= 0x10ffff ? String.fromCodePoint(code) : "";
      }
      return ENTITIES[name.toLowerCase()] ?? entity;
    })
    .replace(/\s+/g, " ")
    .trim();
}

function attributes(tag: string): Map<string, string> {
  const found = new Map<string, string>();
  for (const [, name, quoted, single, bare] of tag.matchAll(
    /([a-z:_-]+)\s*=\s*(?:"([^"]*)"|'([^']*)'|([^\s>]+))/gi,
  ))
    if (name) found.set(name.toLowerCase(), quoted ?? single ?? bare ?? "");
  return found;
}

/** What a page says about itself in its title and meta tags. */
export function readPreview(html: string, url: URL): StashPreview {
  const bodyAt = html.search(/<body[\s>]/i);
  const head = bodyAt < 0 ? html : html.slice(0, bodyAt);
  const meta = new Map<string, string>();
  for (const [tag] of head.matchAll(/<meta\b[^>]*>/gi)) {
    const attrs = attributes(tag);
    const key = attrs.get("property") ?? attrs.get("name");
    const content = attrs.get("content");
    if (key && content && !meta.has(key.toLowerCase()))
      meta.set(key.toLowerCase(), decode(content));
  }
  const title = /<title[^>]*>([^<]*)<\/title>/i.exec(head)?.[1];
  const clip = (text: string | undefined, max: number) =>
    (text ?? "").slice(0, max);
  const preview: StashPreview = {
    title: clip(
      meta.get("og:title") ?? (title ? decode(title) : url.hostname),
      300,
    ),
    site: clip(meta.get("og:site_name") ?? url.hostname, 300),
    description: clip(
      meta.get("og:description") ?? meta.get("description"),
      1000,
    ),
  };
  const image = meta.get("og:image") ?? meta.get("twitter:image");
  const imageUrl = image ? URL.parse(image, url.href) : null;
  if (
    imageUrl &&
    (imageUrl.protocol === "https:" || imageUrl.protocol === "http:") &&
    imageUrl.href.length <= 2048
  )
    preview.imageUrl = imageUrl.href;
  return preview;
}

/**
 * Reads a link's preview from its page, reaching only public addresses (after resolving them, and
 * again after each redirect), within 5 seconds and the page's first megabyte. Null when it cannot.
 */
export async function fetchPreview(link: string): Promise<StashPreview | null> {
  let url = URL.parse(link);
  const signal = AbortSignal.timeout(TIMEOUT_MS);
  for (let hop = 0; url && hop <= MAX_REDIRECTS; hop += 1) {
    if (!isFetchable(url)) return null;
    const answer = await fetchOnce(url, signal).catch(() => null);
    if (answer === null) return null;
    if (answer instanceof URL) url = answer;
    else return readPreview(answer.html, answer.url);
  }
  return null;
}
