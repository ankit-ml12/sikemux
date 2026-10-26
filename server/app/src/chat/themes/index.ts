export type {
  Palette,
  Theme,
  ThemeChrome,
  ThemeEditor,
  ThemeHighlight,
  ThemeSummary,
  ThemeTerminal,
  UiTokens,
} from "@shared/wire.generated";

/** The theme a person has until they pick another, as the core falls back to it. */
export const DEFAULT_THEME_ID = "aura-noir";

export const HIGHLIGHT_KEYS = [
  "keyword",
  "string",
  "comment",
  "number",
  "function",
  "type",
  "variable",
  "property",
  "tag",
  "operator",
  "link",
  "invalid",
  "meta",
  "heading",
] as const;
export const TERMINAL_KEYS = [
  "background",
  "foreground",
  "cursor",
  "cursorAccent",
  "selectionBackground",
  "black",
  "red",
  "green",
  "yellow",
  "blue",
  "magenta",
  "cyan",
  "white",
  "brightBlack",
  "brightRed",
  "brightGreen",
  "brightYellow",
  "brightBlue",
  "brightMagenta",
  "brightCyan",
  "brightWhite",
] as const;
