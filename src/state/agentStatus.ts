import type { AgentGroup, AgentShown, AgentStatus } from "../api/generated/wire";

export type { AgentGroup, AgentShown, AgentStatus };

/** The one word each surface uses for each group. */
export const AGENT_LABELS: Record<AgentGroup, string> = {
    blocked: "Needs input",
    working: "Working",
    done: "Done",
    idle: "Idle",
};

const RANK: Record<AgentGroup, number> = { blocked: 3, working: 2, done: 1, idle: 0 };

export interface AgentDisplay {
    group: AgentGroup;
    subagents: number;
    tasks: number;
    label: string;
    rank: number;
}

/** How an agent is shown everywhere, from the group the core put it in. Shells left running only show as counts. */
export function agentDisplay(shown: Partial<Pick<AgentShown, "group" | "subagents" | "tasks">> = {}): AgentDisplay {
    const group = shown.group ?? "idle";
    return { group, subagents: shown.subagents ?? 0, tasks: shown.tasks ?? 0, label: AGENT_LABELS[group], rank: RANK[group] };
}

/** The group an agent's status puts it in; one starting up already counts as at work. */
export function groupOfStatus(status: AgentStatus): AgentGroup {
    if (status === "starting") return "working";
    return status === "blocked" || status === "working" || status === "done" ? status : "idle";
}

/** A group of agents shown as the one that wants attention most, counting everything they still have going. */
export function rollupAgentDisplays(displays: readonly AgentDisplay[]): AgentDisplay | undefined {
    let best: AgentDisplay | undefined;
    let subagents = 0;
    let tasks = 0;
    for (const display of displays) {
        if (!best || display.rank > best.rank) best = display;
        subagents += display.subagents;
        tasks += display.tasks;
    }
    return best && { ...best, subagents, tasks };
}

/** Mid-turn or waiting on an answer: stopping it now loses work, so closing it asks first. */
export const isAgentBusy = (shown: AgentShown | undefined): boolean => shown?.group === "working" || shown?.group === "blocked";

export const agentHasBackgroundWork = (shown: AgentShown | undefined): boolean => (shown?.tasks ?? 0) + (shown?.subagents ?? 0) > 0;
