import type { IBuffer } from "@xterm/xterm";

export interface TerminalSearchOptions {
    caseSensitive: boolean;
    regex: boolean;
    wholeWord: boolean;
}

/** Produce readable text from the active xterm buffer while preserving soft-wrapped lines. */
export function terminalBufferText(buffer: Pick<IBuffer, "length" | "getLine">): string {
    const out: string[] = [];
    let current = "";
    let started = false;
    for (let row = 0; row < buffer.length; row++) {
        const line = buffer.getLine(row);
        if (!line) continue;
        const text = line.translateToString(true);
        if (!line.isWrapped) {
            if (started) out.push(current);
            current = text;
            started = true;
        } else {
            current += text;
            started = true;
        }
    }
    if (started) out.push(current);
    while (out.length > 0 && out[out.length - 1] === "") out.pop();
    return out.join("\n");
}

export function isTerminalFindShortcut(event: Pick<KeyboardEvent, "code" | "metaKey" | "ctrlKey" | "altKey" | "shiftKey">, macos: boolean): boolean {
    return event.code === "KeyF" && !event.altKey && !event.shiftKey && (macos ? event.metaKey && !event.ctrlKey : event.ctrlKey && !event.metaKey);
}
