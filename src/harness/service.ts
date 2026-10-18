import { invokeCommand } from "../api/invoke";
import { browserApi } from "../api/browser";
import { pagesApi } from "../api/pages";
import { chatShown } from "../chat/chatViews";
import { loadProjectConfig } from "../projects/projectConfig";
import { collectPanes } from "../state/layout";
import { agentIdsOf, projectSessionId } from "../state/selectors";
import { useStore, setState } from "../state/store";
import * as commands from "../state/commands";
import { runShownIn, taskRuns } from "../tasks/coreTasks";
import { openTerminalWorker } from "../agents/terminalWorker";
import { appConsole } from "./appConsole";

/**
 * A tool call the core hands to the window because it needs the window: the
 * panes it shows and what opens in them. The core answers everything else.
 */
export interface HarnessRequest {
    id: string;
    project: string;
    agentId: string | null;
    method: string;
    params: Record<string, unknown>;
}

/** Runs `operation` and puts the person's focus back where it was. */
export function preserveFocus<T>(operation: () => T): T {
    const before = useStore.getState();
    try {
        return operation();
    } finally {
        const current = useStore.getState();
        const sessions = { ...current.sessions };
        for (const [id, session] of Object.entries(sessions)) {
            const previous = before.sessions[id];
            if (previous) sessions[id] = { ...session, activeWindowId: previous.activeWindowId };
        }
        setState({
            activeSessionId: before.activeSessionId,
            zoomedPaneId: before.zoomedPaneId,
            pickerOpen: before.pickerOpen,
            settingsOpen: before.settingsOpen,
            sessions,
        });
    }
}

function text(params: Record<string, unknown>, key: string, required = true): string | undefined {
    const value = params[key];
    if (value === undefined && !required) return undefined;
    if (typeof value !== "string" || !value.trim() || value.length > 4096) throw new Error(`${key} must be nonempty text of at most 4096 characters`);
    return value;
}

function integer(params: Record<string, unknown>, key: string, fallback: number, max: number, min = 0): number {
    const value = params[key] ?? fallback;
    if (typeof value !== "number" || !Number.isSafeInteger(value) || value < min || value > max)
        throw new Error(`${key} must be an integer between ${min} and ${max}`);
    return value;
}

function projectSession(request: HarnessRequest) {
    const state = useStore.getState();
    const sessionId = projectSessionId(state, request.project);
    const session = sessionId ? state.sessions[sessionId] : undefined;
    if (!session) throw new Error("Project is not open in Sikemux");
    if (request.agentId && !agentIdsOf(state, session.id).includes(request.agentId)) throw new Error("Agent does not belong to this project");
    return session;
}

/* A chat draws the page from the call's answer; an agent in its terminal has
   no chat, so the page opens on its desk instead. */
async function showPage(request: HarnessRequest) {
    const { params, agentId } = request;
    const path = text(params, "path")!;
    if (!path.startsWith("/")) throw new Error("path must be absolute");
    const height = params.height === undefined ? undefined : integer(params, "height", 0, 4000, 1);
    const page = await pagesApi.publish(agentId, path, text(params, "title")!, height);
    if (agentId && !chatShown(agentId)) {
        await pagesApi.open(agentId, page.id);
        commands.showDeskBrowser(agentId);
        return { page, note: "The page is open on your desk, beside your terminal, where the person can see it." };
    }
    return {
        page,
        note: "The person sees the page in your reply, above the text you write next. Do not describe or restate it; write only what it does not already say.",
    };
}

export async function handleHarnessRequest(request: HarnessRequest, signal?: AbortSignal): Promise<unknown> {
    if (signal?.aborted) throw signal.reason;
    if (request.method === "agent.terminal") return openTerminalWorker(request.params);
    const session = projectSession(request);
    const { params, project } = request;
    switch (request.method) {
        case "workspace.inspect": {
            const state = useStore.getState();
            return {
                sessionId: session.id,
                active: state.activeSessionId === session.id,
                activeWindowId: session.activeWindowId,
                windows: (state.windowsBySession[session.id] ?? [])
                    .map((id) => state.windows[id])
                    .filter(Boolean)
                    .map((window) => ({
                        id: window.id,
                        name: window.name,
                        role: window.role,
                        panes: collectPanes(window.root).map((pane) => ({
                            id: pane.id,
                            kind: pane.kind,
                            cwd: pane.cwd,
                            activePath: state.editorViews[pane.id]?.activePath,
                            openTabs: state.editorViews[pane.id]?.openTabs,
                        })),
                    })),
            };
        }
        case "ui.open": {
            const kind = text(params, "kind")!;
            if (params.focus !== undefined && typeof params.focus !== "boolean") throw new Error("focus must be a boolean");
            const focus = params.focus === true;
            if (kind === "preview") {
                if (!request.agentId) throw new Error("Opening a preview requires a Sikemux agent session");
                const config = await loadProjectConfig(project);
                const url = config.status === "valid" ? config.config.preview?.url : undefined;
                if (!url) throw new Error("No preview URL is configured in sikemux.json");
                const tabId = await browserApi.newTab(request.agentId, url);
                commands.showDeskBrowser(request.agentId);
                if (focus) {
                    commands.selectSession(session.id);
                    commands.selectAgent(request.agentId);
                }
                return { kind, tabId, url };
            }
            if (kind === "file" && request.agentId) {
                const agentId = request.agentId;
                const path = await invokeCommand<string>("harness_resolve_path", { project, path: text(params, "path")! });
                const line = integer(params, "line", 1, 10_000_000, 1) - 1;
                commands.openFileOnDesk(agentId, path, line, 0, { focus: false });
                if (focus) {
                    commands.selectSession(session.id);
                    commands.selectAgent(agentId);
                }
                return { kind, agentId, path };
            }
            const executionId = kind === "terminal" ? text(params, "executionId")! : undefined;
            const onDesk = executionId ? commands.deskTerminalFor(executionId) : null;
            if (onDesk) {
                commands.showDeskTerminal(onDesk.agentId, onDesk.id);
                if (focus) {
                    commands.selectSession(session.id);
                    commands.selectAgent(onDesk.agentId);
                }
                return { kind, agentId: onDesk.agentId };
            }
            const path = kind === "file" ? await invokeCommand<string>("harness_resolve_path", { project, path: text(params, "path")! }) : undefined;
            const line = integer(params, "line", 1, 10_000_000, 1) - 1;
            const open = () => {
                commands.selectSession(session.id);
                if (kind === "file" && !focus) {
                    commands.openEditorPane();
                    const state = useStore.getState();
                    const window = state.windows[state.sessions[session.id].activeWindowId];
                    const pane = collectPanes(window.root).find((pane) => pane.kind === "editor");
                    if (!pane) throw new Error("Project has no editor pane");
                    commands.openEditorTab(pane.id, path!, false);
                } else if (kind === "file") {
                    const failures = commands.routeCliOpenRequest({
                        id: request.id,
                        cwd: project,
                        wait: false,
                        targets: [{ id: request.id, kind: "file", path: path!, projectRoot: project, line }],
                    });
                    if (failures.some((result) => result.error)) throw new Error(failures.find((result) => result.error)!.error!);
                } else if (kind === "diff") commands.openDiffPane();
                else if (kind === "terminal") {
                    const window = (useStore.getState().windowsBySession[session.id] ?? [])
                        .map((id) => useStore.getState().windows[id])
                        .find((window) =>
                            collectPanes(window.root).some(
                                (pane) => pane.taskTerminalKey && runShownIn(taskRuns.getSnapshot(), pane.taskTerminalKey)?.executionId === executionId,
                            ),
                        );
                    if (!window) throw new Error("Task terminal is no longer open");
                    commands.selectWindowId(window.id);
                } else throw new Error("kind must be file, diff, terminal, or preview");
                return { kind, windowId: useStore.getState().sessions[session.id].activeWindowId, path };
            };
            return focus ? open() : preserveFocus(open);
        }
        case "page.show":
            return showPage(request);
        case "app.console":
            return appConsole.read(params);
        default:
            throw new Error("Unknown harness method");
    }
}
