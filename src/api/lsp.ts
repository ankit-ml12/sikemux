import { coreCall } from "./core";
import type { CoreChange } from "./coreChanged";
import { swallow } from "../state/toast";
import { fileUriToPath } from "../lib/paths";
import { languageServerOf } from "../languages";
import { getIpcTransport, type IpcUnsubscribe } from "./transport";

export interface LspPos {
    line: number;
    character: number;
}

export interface LspRange {
    start: LspPos;
    end: LspPos;
}

export interface LspLocation {
    uri: string;
    range: LspRange;
}

export type LspLocationKind = "definition" | "declaration" | "typeDefinition" | "implementation" | "references";

export interface LspTextChange {
    range?: LspRange;
    rangeLength?: number;
    text: string;
}

export type LspDiagnosticSeverity = "error" | "warning" | "information" | "hint";

export interface LspDiagnostic {
    readonly range: LspRange;
    readonly severity: LspDiagnosticSeverity | null;
    readonly code: string | null;
    readonly source: string | null;
    readonly message: string;
}

export type LspDocumentVersion = number | null;

export interface LspDiagnosticsPayload {
    readonly project: string;
    readonly language: string;
    readonly path: string;
    /** `null` means the server omitted a version; consumers decide freshness. */
    readonly version: LspDocumentVersion;
    readonly diagnostics: readonly LspDiagnostic[];
}

export interface LspDocumentSymbol {
    readonly name: string;
    readonly detail: string | null;
    readonly kind: number;
    readonly range: LspRange;
    readonly selectionRange: LspRange;
    readonly children: readonly LspDocumentSymbol[];
}

export type LspDiagnosticsListener = (payload: LspDiagnosticsPayload) => void;

export interface LspDiagnosticsSubscriptionOptions {
    readonly signal?: AbortSignal;
}

/** Which servers last published diagnostics for each file, so one that clears them is told too. */
const publishedBy = new Map<string, Set<string>>();
const serverKey = (payload: Pick<LspDiagnosticsPayload, "project" | "language">) => `${payload.project}\0${payload.language}`;

/** Reads a file's diagnostics again after the core says they moved, clearing any server's that are gone. */
async function readDiagnostics(path: string, listener: LspDiagnosticsListener): Promise<void> {
    const sets = await coreCall<LspDiagnosticsPayload[]>("lsp.diagnostics", { path });
    const now = new Set(sets.map(serverKey));
    for (const key of publishedBy.get(path) ?? []) {
        if (now.has(key)) continue;
        const [project, language] = key.split("\0");
        listener({ project, language, path, version: null, diagnostics: [] });
    }
    if (now.size) publishedBy.set(path, now);
    else publishedBy.delete(path);
    for (const set of sets) listener(set);
}

let servedLanguages: ReadonlySet<string> = new Set();
let servedLanguagesAsked: Promise<void> | null = null;

export function noteServedLanguages(languages: readonly string[]): void {
    servedLanguages = new Set(languages);
}

/** Asks the native side once which languages a server can start for, including any a `SIKEMUX_LSP_<LANG>` variable adds. */
export function loadServedLanguages(): Promise<void> {
    servedLanguagesAsked ??= coreCall<string[]>("lsp.languages").then(noteServedLanguages, () => {
        servedLanguagesAsked = null;
    });
    return servedLanguagesAsked;
}

/** The language server a file is opened with, when one can start for its language. */
export function languageFromPath(path: string): string | null {
    const language = languageServerOf(path);
    return language && servedLanguages.has(language) ? language : null;
}

export const uriToPath = (uri: string): string => fileUriToPath(uri) ?? uri;

export const lsp = {
    start: (project: string, language: string) => coreCall<void>("lsp.start", { project, language }),
    stop: (project: string) => coreCall<void>("lsp.stop", { project }),
    install: (language: string) => coreCall<string>("lsp.install", { language }),
    open: (project: string, language: string, path: string, content: string, languageId: string = language) =>
        coreCall<void>("lsp.open", { project, language, path, content, languageId }),
    change: (project: string, language: string, path: string, content: string, version: number) =>
        coreCall<void>("lsp.change", { project, language, path, content, version }),
    changeIncremental: (project: string, language: string, path: string, changes: LspTextChange[], version: number) =>
        coreCall<void>("lsp.changeIncremental", { project, language, path, changes, version }),
    save: (project: string, language: string, path: string, content?: string | null) =>
        coreCall<void>("lsp.save", { project, language, path, content: content ?? null }),
    close: (project: string, language: string, path: string) => coreCall<void>("lsp.close", { project, language, path }),
    locations: (project: string, language: string, path: string, line: number, character: number, kind: LspLocationKind) =>
        coreCall<LspLocation[]>("lsp.locations", {
            project,
            language,
            path,
            line,
            character,
            kind,
        }),
    definition: (project: string, language: string, path: string, line: number, character: number) =>
        lsp.locations(project, language, path, line, character, "definition"),
    declaration: (project: string, language: string, path: string, line: number, character: number) =>
        lsp.locations(project, language, path, line, character, "declaration"),
    typeDefinition: (project: string, language: string, path: string, line: number, character: number) =>
        lsp.locations(project, language, path, line, character, "typeDefinition"),
    implementation: (project: string, language: string, path: string, line: number, character: number) =>
        lsp.locations(project, language, path, line, character, "implementation"),
    references: (project: string, language: string, path: string, line: number, character: number) =>
        lsp.locations(project, language, path, line, character, "references"),
    documentSymbols: (project: string, language: string, path: string) => coreCall<LspDocumentSymbol[]>("lsp.symbols", { project, language, path }),
    subscribeDiagnostics: (listener: LspDiagnosticsListener, options: LspDiagnosticsSubscriptionOptions = {}): Promise<IpcUnsubscribe> =>
        getIpcTransport().subscribe<CoreChange>(
            "core_changed",
            ({ payload }) => {
                if (payload.topic !== "lsp") return;
                for (const path of payload.paths ?? []) void readDiagnostics(path, listener).catch(swallow("read a file's diagnostics"));
            },
            options,
        ),
};
