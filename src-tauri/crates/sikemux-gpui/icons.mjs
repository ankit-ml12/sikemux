#!/usr/bin/env node
// Writes the icons the GPUI screens draw as SVG files, by rendering the app's own
// React icon components, so each file is exactly what the app draws.

import { copyFile, mkdir, writeFile } from "node:fs/promises";
import { resolve } from "node:path";
import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { createServer } from "vite";

const crate = import.meta.dirname;
const root = resolve(crate, "../../..");
const out = resolve(crate, "assets/icons");

const ICONS = "/src/ui/Icons.tsx";
const SECTIONS = "/src/codehost/components/ActionsIcon.tsx";

/** File name → the module, its export and the props it is drawn with. */
const DRAWN = {
  logo: [ICONS, "Logo"],
  plus: [ICONS, "IconPlus"],
  folder: [ICONS, "IconFolder"],
  "panel-left": [ICONS, "IconPanelLeft"],
  "panel-right": [ICONS, "IconPanelRight"],
  search: [ICONS, "IconSearch"],
  terminal: [ICONS, "IconTerminal"],
  globe: [ICONS, "IconGlobe"],
  stack: [ICONS, "IconStack"],
  agent: [ICONS, "IconAgent"],
  agents: [ICONS, "IconAgents"],
  file: [ICONS, "IconFile"],
  editor: [ICONS, "IconEditor"],
  git: [ICONS, "IconGit"],
  board: [ICONS, "IconBoard"],
  check: [ICONS, "IconCheck"],
  share: [ICONS, "IconShare"],
  more: [ICONS, "IconMore"],
  eye: [ICONS, "IconEye"],
  close: [ICONS, "IconClose"],
  chevron: [ICONS, "IconChevron"],
  "arrow-down": [ICONS, "IconArrowDown"],
  copy: [ICONS, "IconCopy"],
  text: [ICONS, "IconText"],
  discard: [ICONS, "IconDiscard"],
  inbox: [ICONS, "IconInbox"],
  "log-in": [ICONS, "IconLogIn"],
  "log-out": [ICONS, "IconLogOut"],
  settings: [ICONS, "IconSettings"],
  command: [ICONS, "IconCommand"],
  focus: [ICONS, "IconFocus"],
  pencil: [ICONS, "IconPencil"],
  server: [ICONS, "IconServer"],
  window: [ICONS, "IconWindow"],
  claude: [ICONS, "IconClaude"],
  run: [ICONS, "IconRun"],
  stop: [ICONS, "IconStop"],
  refresh: [ICONS, "IconRefresh"],
  laptop: [ICONS, "IconLaptop"],
  plug: [ICONS, "IconPlug"],
  "pull-request": [ICONS, "IconPullRequest"],
  shield: [ICONS, "IconShield"],
  "shield-bolt": [ICONS, "IconShieldBolt"],
  "arrow-up": [ICONS, "IconArrowUp"],
  mic: [ICONS, "IconMic"],
  "file-plus": [ICONS, "IconFilePlus"],
  "folder-plus": [ICONS, "IconFolderPlus"],
  "collapse-all": [ICONS, "IconCollapseAll"],
  actions: [SECTIONS, "SectionIcon", { section: "actions" }],
  issues: [SECTIONS, "SectionIcon", { section: "issues" }],
  releases: [SECTIONS, "SectionIcon", { section: "releases" }],
  github: ["/src/plugins/github/components/GithubMark.tsx", "GithubMark"],
  aws: [ICONS, "IconAws"],
  rundeck: [ICONS, "IconRundeck"],
  database: ["/src/plugins/database/components/DatabaseMark.tsx", "DatabaseMark"],
  jira: ["/src/plugins/jira/components/JiraMark.tsx", "JiraMark"],
  linear: ["/src/plugins/linear/components/LinearMark.tsx", "LinearMark"],
  slack: ["/src/plugins/slack/components/SlackMark.tsx", "SlackMark"],
  vercel: ["/src/plugins/vercel/components/VercelMark.tsx", "VercelMark"],
  activity: [ICONS, "IconActivity"],
  book: [ICONS, "IconBook"],
  camera: [ICONS, "IconCamera"],
  click: [ICONS, "IconClick"],
  clock: [ICONS, "IconClock"],
  code: [ICONS, "IconCode"],
  dashboard: [ICONS, "IconDashboard"],
  download: [ICONS, "IconDownload"],
  highlighter: [ICONS, "IconHighlighter"],
  "home-button": [ICONS, "IconHomeButton"],
  hourglass: [ICONS, "IconHourglass"],
  issue: [ICONS, "IconIssue"],
  keyboard: [ICONS, "IconKeyboard"],
  link: [ICONS, "IconLink"],
  "log-lines": [ICONS, "IconLogLines"],
  message: [ICONS, "IconMessage"],
  mouse: [ICONS, "IconMouse"],
  move: [ICONS, "IconMove"],
  network: [ICONS, "IconNetwork"],
  panel: [ICONS, "IconPanel"],
  phone: [ICONS, "IconPhone"],
  pinch: [ICONS, "IconPinch"],
  pointer: [ICONS, "IconPointer"],
  record: [ICONS, "IconRecord"],
  resize: [ICONS, "IconResize"],
  rocket: [ICONS, "IconRocket"],
  rotate: [ICONS, "IconRotate"],
  "square-plus": [ICONS, "IconSquarePlus"],
  status: [ICONS, "IconStatus"],
  steps: [ICONS, "IconSteps"],
  swipe: [ICONS, "IconSwipe"],
  tabs: [ICONS, "IconTabs"],
  tap: [ICONS, "IconTap"],
  "touch-path": [ICONS, "IconTouchPath"],
  trash: [ICONS, "IconTrash"],
  unlink: [ICONS, "IconUnlink"],
  upload: [ICONS, "IconUpload"],
  warning: [ICONS, "IconWarning"],
  waterfall: [ICONS, "IconWaterfall"],
  workflow: [ICONS, "IconWorkflow"],
  wrench: [ICONS, "IconWrench"],
};
/** Marks the app draws as pictures of their own, copied as they are. */
const PICTURES = {
  codex: "src/assets/agent-logos/codex.svg",
  bruno: "src/assets/bruno.svg",
  signoz: "src/plugins/signoz/signoz.svg",
};
/** Marks with colours of their own beside a part in the text's colour, which a
 * picture cannot follow, so that part takes the rail's body ink. */
const MIXED = { aws: "#acabb4" };

const server = await createServer({
  root,
  logLevel: "error",
  server: { middlewareMode: true, hmr: false },
  appType: "custom",
  optimizeDeps: { noDiscovery: true, include: [] },
});
try {
  await mkdir(out, { recursive: true });
  for (const [file, [module, name, props = {}]] of Object.entries(DRAWN)) {
    const component = (await server.ssrLoadModule(module))[name];
    const markup = renderToStaticMarkup(createElement(component, { size: 16, ...props }))
      .replace(/^<svg[^>]*>/, (root) => root.replace(/ (width|height|class|aria-hidden)="[^"]*"/g, ""))
      .replace("<svg", '<svg xmlns="http://www.w3.org/2000/svg"')
      .replaceAll("currentColor", MIXED[file] ?? "currentColor");
    await writeFile(resolve(out, `${file}.svg`), `${markup}\n`);
  }
  for (const [file, path] of Object.entries(PICTURES)) await copyFile(resolve(root, path), resolve(out, `${file}.svg`));
  const { BATTERY_BOLT } = await server.ssrLoadModule(ICONS);
  const bolt = (extra) =>
    `<svg xmlns="http://www.w3.org/2000/svg" viewBox="7 -3 12 19" fill="currentColor"><path d="${BATTERY_BOLT}"${extra}></path></svg>\n`;
  await writeFile(resolve(out, "battery-bolt.svg"), bolt(""));
  await writeFile(resolve(out, "battery-bolt-gap.svg"), bolt(' stroke="currentColor" stroke-width="2.4" stroke-linejoin="round"'));
  const { MARK_INNER } = await server.ssrLoadModule(SECTIONS);
  for (const [outcome, d] of Object.entries(MARK_INNER)) {
    const mark = `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round"><path d="${d}"></path></svg>`;
    await writeFile(resolve(out, `mark-${outcome}.svg`), `${mark}\n`);
  }
} finally {
  await server.close();
}
