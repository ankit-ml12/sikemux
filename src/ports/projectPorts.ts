import { collectPanes } from "../state/layout";
import { agentIdsOf, activeAgentId, terminalLabel } from "../state/selectors";
import type { StoreState } from "../state/store";
import { runShownIn, type TaskRun } from "../tasks/coreTasks";

type PortsState = Pick<StoreState, "sessions" | "windows" | "windowsBySession" | "agents" | "agentStatus" | "desks" | "terminalTitles">;

/** Who listens on a port, as the core matched it to its own sessions. */
export type CorePortOwner =
    | { kind: "terminal"; sessionId: number; paneId?: string }
    | { kind: "task"; sessionId: number; executionId: string; taskId: string; label: string; agentId?: string }
    | { kind: "agent"; agentId: string; label: string; sessionId?: number };

/** A port as the core's `ports` call answers it. */
export interface CorePort {
    port: number;
    address: string;
    pid: number;
    process: string;
    project?: string;
    owner: CorePortOwner;
    url: string;
    preview: boolean;
}

export type PortReveal =
    | { kind: "agent"; agentId: string }
    | { kind: "pane"; sessionId: string; windowId: string; paneId: string }
    | { kind: "desk-terminal"; agentId: string; id: string };

export interface ProjectPort {
    port: number;
    address: string;
    process: string;
    url: string;
    preview: boolean;
    owner: {
        kind: "terminal" | "task" | "agent";
        label: string;
        reveal: PortReveal | null;
        terminalKey?: string;
        /** Set when a task run listens here, so it can be stopped. */
        executionId?: string;
    };
}

function sessionPanes(state: PortsState, sessionId: string) {
    return (state.windowsBySession[sessionId] ?? []).flatMap((windowId) => {
        const win = state.windows[windowId];
        return win ? collectPanes(win.root).map((pane) => ({ windowId, pane })) : [];
    });
}

/** The running agent in front, or else the running one that worked last. Never one that is asleep. */
export function deskAgentFor(state: PortsState, sessionId: string): string | null {
    const live = agentIdsOf(state, sessionId).filter((id) => state.agents[id] && state.agents[id].launchState !== "dormant");
    const front = activeAgentId(state, state.sessions[sessionId]);
    if (front && live.includes(front)) return front;
    const lastWorked = (id: string) => state.agentStatus[id]?.activeAt ?? 0;
    return live.sort((left, right) => lastWorked(right) - lastWorked(left))[0] ?? null;
}

/** Where a task's output is showing, for opening it. */
export function taskTerminalReveal(state: PortsState, sessionId: string, terminalKey: string): PortReveal | null {
    for (const [agentId, desk] of Object.entries(state.desks))
        for (const terminal of desk.terminals)
            if (terminal.terminalKey === terminalKey) return { kind: "desk-terminal", agentId, id: terminal.id };
    const shown = sessionPanes(state, sessionId).find(({ pane }) => pane.taskTerminalKey === terminalKey);
    return shown ? { kind: "pane", sessionId, windowId: shown.windowId, paneId: shown.pane.id } : null;
}

function ownerOf(state: PortsState, sessionId: string, runs: readonly TaskRun[], owner: CorePortOwner): ProjectPort["owner"] {
    if (owner.kind === "agent") return { kind: "agent", label: owner.label, reveal: state.agents[owner.agentId] ? { kind: "agent", agentId: owner.agentId } : null };
    if (owner.kind === "task") {
        const terminalKey = runs.find((run) => run.executionId === owner.executionId)?.terminalKey;
        const ran = { terminalKey, executionId: owner.executionId };
        if (owner.agentId && state.agents[owner.agentId])
            return { kind: "agent", label: state.agents[owner.agentId].title || "agent", reveal: { kind: "agent", agentId: owner.agentId }, ...ran };
        const reveal = terminalKey && runShownIn(runs, terminalKey) ? taskTerminalReveal(state, sessionId, terminalKey) : null;
        return { kind: "task", label: owner.label, reveal, ...ran };
    }
    const shown = owner.paneId ? sessionPanes(state, sessionId).find(({ pane }) => pane.id === owner.paneId) : undefined;
    if (!shown) return { kind: "terminal", label: "terminal", reveal: null };
    const label = terminalLabel(state.windows[shown.windowId], shown.pane, state.terminalTitles);
    return { kind: "terminal", label, reveal: { kind: "pane", sessionId, windowId: shown.windowId, paneId: shown.pane.id } };
}

/** The core's ports for the project, labelled and pointed at what this window shows of their owners. */
export function projectPorts(state: PortsState, sessionId: string, ports: readonly CorePort[], runs: readonly TaskRun[]): ProjectPort[] {
    return ports.map((port) => ({
        port: port.port,
        address: port.address,
        process: port.process,
        url: port.url,
        preview: port.preview,
        owner: ownerOf(state, sessionId, runs, port.owner),
    }));
}
