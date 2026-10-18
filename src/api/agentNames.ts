import type { AgentBuddy, AgentType } from "../state/types";
import { coreCall } from "./core";
import { invokeCommand as invoke } from "./invoke";

/** An agent's name as the core keeps it. */
export interface AgentName {
    readonly agentId: string;
    readonly name: string;
    /** The project folder its name is unique in. */
    readonly project: string;
    /** The agent that started it, when another agent did. */
    readonly parent: string | null;
}

/** The name, and when it opens again under a name the person gave it, the character it wore before closing. */
export interface GivenName extends AgentName {
    readonly buddy?: unknown;
}

/** A closed agent the person named, which keeps that name and opens again under it. */
export interface ClosedAgent {
    readonly agentId: string;
    readonly name: string;
    readonly provider: AgentType;
    readonly sessionId: string;
    readonly title: string | null;
    readonly buddy: unknown;
}

/** The launcher and permission mode the app runs an agent with. */
export interface AgentRun {
    readonly launcher: string;
    readonly permissionMode: string;
}

export interface NameRequest {
    readonly agentId: string;
    readonly cwd: string;
    readonly provider: string;
    /** The provider session it runs. */
    readonly resumeId?: string;
    /** How the app draws it and what it works on, for when it opens again after closing. */
    readonly buddy?: AgentBuddy;
    readonly title?: string;
}

export const agentNamesApi = {
    /** The agent's name, given now when it has none. */
    name: (request: NameRequest) => coreCall<GivenName>("nameAgent", request),
    rename: (agentId: string, name: string) => coreCall<AgentName>("renameAgent", { agentId, name }),
    closed: (cwd: string) => coreCall<ClosedAgent[]>("closedAgents", { cwd }),
    /**
     * Every agent the app holds, sleeping ones included, so the core keeps their names theirs and frees the names of the
     * rest, and how each runs, so an agent a terminal agent starts runs on its account and in its mode.
     */
    inApp: (agentIds: readonly string[], runs: Readonly<Record<string, AgentRun>>) =>
        invoke<null>("agent_name_call", { method: "agentsInApp", params: { agentIds, runs } }),
};
