#!/usr/bin/env node
// Writes the icons the GPUI screens draw as SVG files, from the app's own in
// src/ui/Icons.tsx and src/assets/agent-logos.

import { copyFile, mkdir, readFile, writeFile } from "node:fs/promises";
import { resolve } from "node:path";

const crate = import.meta.dirname;
const root = resolve(crate, "../../..");
const out = resolve(crate, "assets/icons");
const source = await readFile(resolve(root, "src/ui/Icons.tsx"), "utf8");

const ICONS = {
  logo: "Logo",
  plus: "IconPlus",
  folder: "IconFolder",
  "panel-left": "IconPanelLeft",
  "panel-right": "IconPanelRight",
  search: "IconSearch",
  terminal: "IconTerminal",
  globe: "IconGlobe",
  stack: "IconStack",
  agent: "IconAgent",
  agents: "IconAgents",
  file: "IconFile",
  editor: "IconEditor",
  git: "IconGit",
  board: "IconBoard",
  check: "IconCheck",
  share: "IconShare",
  more: "IconMore",
  eye: "IconEye",
  close: "IconClose",
  chevron: "IconChevron",
  inbox: "IconInbox",
  "log-in": "IconLogIn",
  settings: "IconSettings",
  command: "IconCommand",
  focus: "IconFocus",
  pencil: "IconPencil",
  server: "IconServer",
  window: "IconWindow",
  claude: "IconClaude",
};
const LOGOS = { codex: "codex.svg" };

const jsx = (text) =>
  text
    .replace(/<\/?>/g, "")
    .replace(/([a-z]+[A-Z][a-zA-Z]*)=/g, (_, name) =>
      name === "viewBox" ? "viewBox=" : `${name.replace(/[A-Z]/g, (c) => `-${c.toLowerCase()}`)}=`,
    )
    .replace(/=\{([^}]*)\}/g, (_, value) => `="${value.replace(/^"|"$/g, "")}"`)
    .trim();

function svg(name) {
  const made = source.match(new RegExp(`export const ${name} = makeSvgIcon(24)?\\(\\n?([\\s\\S]*?)\\);\\n`));
  if (made) {
    const [, grid24, body] = made;
    const [, children, options = ""] = body.match(/^([\s\S]*?>)\s*,?\s*(\{[^}]*\})?,?\s*$/);
    const viewBox = grid24 ? "0 0 24 24" : (options.match(/viewBox: "([^"]+)"/)?.[1] ?? "0 0 16 16");
    const fill = options.match(/fill: "([^"]+)"/)?.[1] ?? "none";
    const inner = grid24 ? `<g stroke-width="2.1">${jsx(children)}</g>` : jsx(children);
    return `<svg xmlns="http://www.w3.org/2000/svg" viewBox="${viewBox}" fill="${fill}" stroke="currentColor" stroke-width="1.4" stroke-linecap="round" stroke-linejoin="round">${inner}</svg>\n`;
  }
  const [, body] = source.match(new RegExp(`export function ${name}\\([\\s\\S]*?return \\(([\\s\\S]*?)\\);\\n\\}`));
  return `${jsx(body)
    .replace(/<Svg\b([^>]*)>/, (_, attributes) => {
      const viewBox = attributes.match(/viewBox="([^"]+)"/)?.[1] ?? "0 0 16 16";
      const fill = attributes.match(/fill="([^"]+)"/)?.[1] ?? "none";
      return `<svg viewBox="${viewBox}" fill="${fill}" stroke="currentColor" stroke-width="1.4" stroke-linecap="round" stroke-linejoin="round">`;
    })
    .replace("</Svg>", "</svg>")
    .replace(/ (width|height|class-name|aria-hidden)="[^"]*"/g, "")
    .replace("<svg ", '<svg xmlns="http://www.w3.org/2000/svg" ')}\n`;
}

await mkdir(out, { recursive: true });
for (const [file, name] of Object.entries(ICONS)) await writeFile(resolve(out, `${file}.svg`), svg(name));
for (const [file, logo] of Object.entries(LOGOS))
  await copyFile(resolve(root, "src/assets/agent-logos", logo), resolve(out, `${file}.svg`));
