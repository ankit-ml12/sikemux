import type { AgentInfo } from "../api/agents";
import { agentTypes, agentLabel, featuredAgentTypes, installCommand } from "./catalog";

/** Every agent Sikemux can set up: those it found, and those it can install that it did not. */
export function withMissingAgents(found: readonly AgentInfo[]): AgentInfo[] {
    const missing = agentTypes()
        .filter((type) => installCommand(type) && !found.some((agent) => agent.type === type))
        .map((type): AgentInfo => ({
            type,
            label: agentLabel(type),
            command: type,
            available: false,
            defaultModel: null,
            defaultEffort: null,
            status: { state: "missing" },
        }));
    return [...found, ...missing];
}

/** Ready agents first, then the featured ones, then the rest in the catalog's order. */
export function setupOrder(agents: readonly AgentInfo[]): AgentInfo[] {
    const rank = (agent: AgentInfo) => {
        const ready = agent.available !== false && agent.status?.state !== "signedOut" ? 0 : 1;
        const featured = featuredAgentTypes().includes(agent.type) ? 0 : 1;
        return [ready, featured, agentTypes().indexOf(agent.type)];
    };
    return [...agents].sort((a, b) => {
        const [left, right] = [rank(a), rank(b)];
        return left[0] - right[0] || left[1] - right[1] || left[2] - right[2];
    });
}
