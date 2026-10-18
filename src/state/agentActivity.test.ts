import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { installIpcTransportForTests, MemoryIpcTransport, resetIpcTransportForTests } from "../api/transport";
import { closeAgent, selectAgent, sleepAgent } from "./commands";
import { getState, setState } from "./store";
import { withAgents } from "../test/agents";

const initial = getState();

beforeEach(() => {
    resetIpcTransportForTests();
    setState(initial, true);
});
afterEach(resetIpcTransportForTests);

function installAgent() {
    const state = getState();
    const sid = state.activeSessionId;
    const agent = { id: "agent-1", type: "claude" as const, title: "Claude", startup: "claude", ptyId: 7 };
    setState({
        sessions: { ...state.sessions, [sid]: { ...state.sessions[sid], kind: "project" } },
        ...withAgents(state, sid, [agent]),
    });
    return agent.id;
}

describe("agent activity", () => {
    it("sleeps only resumable terminal agents and resumes a sleeping one on selection", () => {
        const id = installAgent();
        expect(sleepAgent(id)).toBe(false);

        setState((state) => ({ agents: { ...state.agents, [id]: { ...state.agents[id], resumeId: "session-1" } } }));
        expect(sleepAgent(id)).toBe(true);
        expect(getState().agents[id].launchState).toBe("dormant");

        selectAgent(id);
        expect(getState().agents[id].launchState).toBe("live");
    });

    it("closes browser targets when an agent is removed", async () => {
        const id = installAgent();
        const transport = new MemoryIpcTransport();
        const closed: unknown[] = [];
        transport.register("browser_close_agent", (args) => closed.push(args));
        installIpcTransportForTests(transport);

        closeAgent(id);
        await Promise.resolve();

        expect(getState().agents[id]).toBeUndefined();
        expect(closed).toEqual([{ agentId: id }]);
    });
});
