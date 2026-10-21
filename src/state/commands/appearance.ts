import { invokeCommand as invoke } from "../../api/invoke";
import { DEFAULT_THEME_ID, type Theme } from "../../themes";
import { applyTheme, applyWindowOpacity, previewTheme, registerCustomThemes } from "../../themes/bus";
import { applyTerminalFontSize, clampTerminalFontSize, DEFAULT_TERMINAL_FONT_SIZE } from "../../terminal/fontSize";
import { applyChatTextScale, clampChatTextScale, DEFAULT_CHAT_TEXT_SCALE } from "../../chat/textScale";
import { applyEditorTextScale, clampEditorTextScale, DEFAULT_EDITOR_TEXT_SCALE } from "../../editor/textScale";
import { setPreference } from "../preferences";
import { getState, setState } from "../store";
import { swallow } from "../toast";

export const themeIdNow = (): string => getState().preferences?.themeId ?? DEFAULT_THEME_ID;
const customThemesNow = (): Theme[] => getState().preferences?.customThemes ?? [];

export function setThemeId(id: string): void {
    applyTheme(id);
    setPreference("themeId", id);
}

/** Live-apply a draft theme to the whole UI without persisting it — drives the theme editor preview. */
export function previewThemeDraft(theme: Theme): void {
    previewTheme(theme);
}

/** Discard any active preview and re-apply the persisted theme selection. */
export function cancelThemePreview(): void {
    applyTheme(themeIdNow());
}

/** Insert or overwrite a custom theme (matched by id), register it, and make it the active theme. */
export function saveCustomTheme(theme: Theme): void {
    const kept = customThemesNow();
    const customThemes = kept.some((t) => t.id === theme.id) ? kept.map((t) => (t.id === theme.id ? theme : t)) : [...kept, theme];
    registerCustomThemes(customThemes);
    setPreference("customThemes", customThemes);
    setThemeId(theme.id);
}

export function deleteCustomTheme(id: string): void {
    const customThemes = customThemesNow().filter((t) => t.id !== id);
    registerCustomThemes(customThemes);
    setPreference("customThemes", customThemes);
    if (themeIdNow() === id) setThemeId(DEFAULT_THEME_ID);
}

export function setTerminalFontSize(v: number): void {
    const value = clampTerminalFontSize(v);
    applyTerminalFontSize(value);
    setPreference("terminalFontSize", value);
}

export function adjustTerminalFontSize(step: number): void {
    setTerminalFontSize(terminalFontSize() + step);
}

export function resetTerminalFontSize(): void {
    setTerminalFontSize(DEFAULT_TERMINAL_FONT_SIZE);
}

export function setChatTextScale(v: number): void {
    const value = clampChatTextScale(v);
    applyChatTextScale(value);
    setPreference("chatTextScale", value);
}

export function adjustChatTextScale(step: number): void {
    setChatTextScale(chatTextScale() + step);
}

export function resetChatTextScale(): void {
    setChatTextScale(DEFAULT_CHAT_TEXT_SCALE);
}

export function setEditorTextScale(v: number): void {
    const value = clampEditorTextScale(v);
    applyEditorTextScale(value);
    setPreference("editorTextScale", value);
}

export function adjustEditorTextScale(step: number): void {
    setEditorTextScale(editorTextScale() + step);
}

export const terminalFontSize = (): number => getState().preferences?.terminalFontSize ?? DEFAULT_TERMINAL_FONT_SIZE;
export const chatTextScale = (): number => getState().preferences?.chatTextScale ?? DEFAULT_CHAT_TEXT_SCALE;
export const editorTextScale = (): number => getState().preferences?.editorTextScale ?? DEFAULT_EDITOR_TEXT_SCALE;

export function resetEditorTextScale(): void {
    setEditorTextScale(DEFAULT_EDITOR_TEXT_SCALE);
}

export function setWindowOpacity(v: number): void {
    const value = Number.isFinite(v) ? v : 1;
    applyWindowOpacity(value);
    setState({ windowOpacity: value });
}

export function setWindowBlur(v: number): void {
    const value = Number.isFinite(v) ? Math.round(v) : 0;
    void invoke("set_window_blur", { radius: value }).catch(swallow("set_window_blur"));
    setState({ windowBlur: value });
}
