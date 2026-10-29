import {
  codeToHtml,
  createCssVariablesTheme,
  createHighlighterCore,
  getTokenStyleObject,
  stringifyTokenStyle,
} from "shiki/core";
import type { LanguageRegistration } from "shiki/core";
import { createJavaScriptRegexEngine } from "shiki/engine/javascript";
import { createOnigurumaEngine } from "./oniguruma";

type GrammarModule = { default: LanguageRegistration[] };

/* The grammars the page ships with; a fence in any other language reads as plain text. */
const SHIPPED: Readonly<Record<string, () => Promise<GrammarModule>>> = {
  css: () => import("@shikijs/langs/css"),
  html: () => import("@shikijs/langs/html"),
  json: () => import("@shikijs/langs/json"),
  jsonc: () => import("@shikijs/langs/jsonc"),
  markdown: () => import("@shikijs/langs/markdown"),
  python: () => import("@shikijs/langs/python"),
  rust: () => import("@shikijs/langs/rust"),
  shellscript: () => import("@shikijs/langs/shellscript"),
  typescript: () => import("@shikijs/langs/typescript"),
  yaml: () => import("@shikijs/langs/yaml"),
};

export const bundledLanguages: Readonly<
  Record<string, () => Promise<GrammarModule>>
> = SHIPPED;

export function createHighlighter(
  options: Parameters<typeof createHighlighterCore>[0],
) {
  return createHighlighterCore({ ...options, langs: [] });
}

export {
  codeToHtml,
  createCssVariablesTheme,
  createJavaScriptRegexEngine,
  createOnigurumaEngine,
  getTokenStyleObject,
  stringifyTokenStyle,
};
