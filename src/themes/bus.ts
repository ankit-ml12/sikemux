import { readableColor } from "../lib/themeContrast";
import type { EditorView } from "@codemirror/view";
import type { TerminalTheme } from "../terminal/canvas";
import { invokeCommand } from "../api/invoke";
import { DEFAULT_THEME_ID, TERMINAL_KEYS, themeById, type Theme } from ".";

let current: Theme = themeById(DEFAULT_THEME_ID);
let currentOpacity = 1;

const views = new Set<EditorView>();
/** A terminal that draws with the theme's colours. */
export interface ThemedTerminal {
    setTheme(theme: TerminalTheme): void;
}

const terms = new Map<ThemedTerminal, () => boolean>();
const termThemes = new WeakMap<ThemedTerminal, TerminalTheme>();
let terminalTheme: TerminalTheme | null = null;

/** What an editor view needs from a theme change. Supplied by `editor/themeBridge`
 *  the first time a view registers, so nothing here pulls CodeMirror into the boot bundle. */
export interface EditorThemeBridge {
    rebuild(theme: Theme): void;
    push(view: EditorView): void;
}

let editorBridge: EditorThemeBridge | null = null;
let bridgeRequested = false;

export function setEditorThemeBridge(bridge: EditorThemeBridge): void {
    editorBridge = bridge;
    bridge.rebuild(current);
    views.forEach((view) => bridge.push(view));
}

function loadEditorBridge(): void {
    if (editorBridge || bridgeRequested) return;
    bridgeRequested = true;
    void import("../editor/themeBridge");
}

const customRegistry = new Map<string, Theme>();
const themeListeners = new Set<(theme: Theme) => void>();

/** Keep the bus aware of user-defined themes so {@link applyTheme} can resolve their ids. */
export function registerCustomThemes(list: readonly Theme[]): void {
    customRegistry.clear();
    for (const t of list) customRegistry.set(t.id, t);
}

function resolveTheme(id: string): Theme {
    return customRegistry.get(id) ?? themeById(id);
}

function applyTransparentState() {
    document.documentElement.classList.toggle("is-transparent", currentOpacity < 1);
}

const ANSI_KEYS = TERMINAL_KEYS.slice(TERMINAL_KEYS.indexOf("black"));

function terminalThemeFor(theme: Theme): TerminalTheme {
    const colors = theme.terminal;
    return {
        foreground: colors.foreground,
        background: colors.background,
        cursor: colors.cursor,
        cursorAccent: colors.cursorAccent,
        selectionBackground: colors.selectionBackground,
        ansi: ANSI_KEYS.map((key) => colors[key]),
    };
}

function rgb(color: string): [number, number, number] | null {
    const match = /^#([0-9a-f]{2})([0-9a-f]{2})([0-9a-f]{2})/i.exec(color.trim());
    return match ? [parseInt(match[1], 16), parseInt(match[2], 16), parseInt(match[3], 16)] : null;
}

/** The core answers programs that ask for the terminal's colours, such as one choosing light or dark output. */
function tellCoreTerminalColors(theme: TerminalTheme): void {
    const foreground = rgb(theme.foreground);
    const background = rgb(theme.background);
    const ansi = theme.ansi.map(rgb);
    if (!foreground || !background || ansi.length !== 16 || ansi.some((color) => !color)) return;
    void invokeCommand("pty_set_colors", {
        colors: { foreground, background, cursor: rgb(theme.cursor), ansi },
    }).catch(() => {});
}

/* The one terminal palette, which construction and later theme changes both use. */
export function currentTerminalTheme(): TerminalTheme {
    if (!terminalTheme) {
        terminalTheme = terminalThemeFor(current);
        tellCoreTerminalColors(terminalTheme);
    }
    return terminalTheme;
}

/** Gives a terminal the current palette if it does not have it yet. */
export function refreshTerminalTheme(term: ThemedTerminal): void {
    const theme = currentTerminalTheme();
    if (termThemes.get(term) === theme) return;
    termThemes.set(term, theme);
    term.setTheme(theme);
}

// A terminal out of sight takes the new palette once it is shown again.
function applyTerminalThemes() {
    terminalTheme = terminalThemeFor(current);
    tellCoreTerminalColors(terminalTheme);
    terms.forEach((visible, term) => {
        if (visible()) refreshTerminalTheme(term);
    });
}

export function currentTheme(): Theme {
    return current;
}

export function subscribeTheme(listener: (theme: Theme) => void): () => void {
    themeListeners.add(listener);
    return () => themeListeners.delete(listener);
}

export function registerView(view: EditorView): () => void {
    loadEditorBridge();
    editorBridge?.push(view);
    views.add(view);
    return () => views.delete(view);
}

export function refreshViewTheme(view: EditorView): void {
    editorBridge?.push(view);
}

export function registerTerminal(term: ThemedTerminal, visible: () => boolean): () => void {
    refreshTerminalTheme(term);
    terms.set(term, visible);
    return () => terms.delete(term);
}

function applyChrome(theme: Theme) {
    const c = theme.chrome;
    const root = document.documentElement.style;
    document.documentElement.classList.toggle("theme-light", !theme.dark);

    root.setProperty("--bg", c.bg);
    root.setProperty("--bg-dim", c.bgDim);
    root.setProperty("--bg-raised", c.bgRaised);
    root.setProperty("--ink", c.ink);
    root.setProperty("--ink-dim", c.inkDim);
    const muted = readableColor(c.inkDim, [c.bg, c.bgDim, c.bgRaised], c.ink);
    root.setProperty("--ink-muted", muted);
    root.setProperty("--text-tertiary", muted);
    root.setProperty("--acc", c.acc);
    root.setProperty("--acc-line", c.accLine);
    root.setProperty("--acc-dim", c.accDim);
    root.setProperty("--line", c.line);
    root.setProperty("--hl", c.hl);
    root.setProperty("--danger", readableColor(c.danger, [c.bg, c.bgDim, c.bgRaised], c.ink));

    root.setProperty("--void", c.bgDim);
    root.setProperty("--rail", c.bg);
    root.setProperty("--rail-2", c.bgRaised);
    root.setProperty("--pane", c.bg);
    root.setProperty("--line-soft", c.bgRaised);
    root.setProperty("--ink-faint", muted);
    root.setProperty("--acc-soft", c.accDim);
    root.setProperty("--live", readableColor(theme.dark ? "#78dca1" : "#247345", [c.bg, c.bgRaised], c.ink));
    root.setProperty("--warn", readableColor(theme.dark ? "#e9ba6c" : "#875508", [c.bg, c.bgRaised], c.ink));
    for (const [token, color] of Object.entries({
        "--git-modified": theme.dark ? "#e2c08d" : "#875508",
        "--git-untracked": theme.dark ? "#73c991" : "#247345",
        "--git-added": theme.dark ? "#81b88b" : "#247345",
        "--git-deleted": theme.dark ? "#ed806b" : "#aa3824",
        "--git-renamed": theme.dark ? "#94b1ef" : "#345bad",
    }))
        root.setProperty(token, readableColor(color, [c.bg, c.bgDim, c.bgRaised], c.ink));
    for (const [key, color] of Object.entries(theme.editor)) root.setProperty(`--editor-${key}`, color);
    for (const [key, color] of Object.entries(theme.highlight)) root.setProperty(`--syntax-${key}`, color);
    root.setProperty("--cmd", theme.highlight.function);
    root.setProperty("--terminal-background", theme.terminal.background);
    TERMINAL_KEYS.slice(TERMINAL_KEYS.indexOf("black")).forEach((key, index) => root.setProperty(`--terminal-ansi-${index}`, theme.terminal[key]));

    applyTransparentState();
}

let settling = 0;

function applyThemeObject(next: Theme): void {
    const root = document.documentElement;
    root.classList.add("theme-changing");
    current = next;
    applyChrome(next);
    editorBridge?.rebuild(next);
    if (editorBridge) views.forEach(editorBridge.push);
    applyTerminalThemes();
    themeListeners.forEach((listener) => listener(next));
    // Transitions stay off until the new colours have been drawn once, so nothing fades between themes.
    cancelAnimationFrame(settling);
    settling = requestAnimationFrame(() => {
        settling = requestAnimationFrame(() => root.classList.remove("theme-changing"));
    });
}

/* The colours on screen now, so putting the same ones on again does nothing. */
let worn = "";

function wear(theme: Theme): void {
    const colours = JSON.stringify(theme);
    if (colours === worn) return;
    worn = colours;
    applyThemeObject(theme);
}

export function applyTheme(id: string): void {
    wear(resolveTheme(id));
}

/** Apply a theme object directly without touching the persisted selection — used for live editing previews. */
export function previewTheme(theme: Theme): void {
    wear(theme);
}

export function applyWindowOpacity(opacity: number): void {
    const v = Math.max(0, Math.min(1, opacity));
    currentOpacity = v;
    document.documentElement.style.setProperty("--window-opacity", String(v));
    applyTransparentState();
    applyTerminalThemes();
}
