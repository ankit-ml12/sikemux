import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { installIpcTransportForTests, MemoryIpcTransport, resetIpcTransportForTests } from "../api/transport";
import { recordAgentTurns } from "./activityRecorder";
import { setAgentStatus } from "./commands";
import { getState, setState } from "./store";
import { withAgents } from "../test/agents";

const initial = getState();
let calls: [string, unknown][] = [];
let stop = () => {};

beforeEach(() => {
    resetIpcTransportForTests();
    setState(initial, true);
    calls = [];
    const transport = new MemoryIpcTransport();
    for (const command of ["activity_turn_started", "activity_turn_ended", "browser_close_agent"]) {
        transport.register(command, (args) => void calls.push([command, args]));
    }
    installIpcTransportForTests(transport);
    const state = getState();
    const sid = state.activeSessionId;
    setState({
        sessions: { ...state.sessions, [sid]: { ...state.sessions[sid], kind: "project", cwd: "/work/app" } },
        ...withAgents(state, sid, [{ id: "agent-1", type: "codex", title: "Codex", startup: "codex", resumeId: "s-1" }]),
    });
    stop = recordAgentTurns();
});

afterEach(() => {
    stop();
    resetIpcTransportForTests();
});

const noteAgentActivity = (agentId: string, state: "working" | "complete") =>
    setAgentStatus([{ agentId, status: state === "working" ? "working" : "done", group: state === "working" ? "working" : "done", unread: false, subagents: 0, tasks: 0, activeAt: null }]);

const recorded = () => calls.filter(([command]) => command.startsWith("activity_"));

describe("agent turn recording", () => {
    it("opens a turn when the agent starts working and closes it when it settles", async () => {
        noteAgentActivity("agent-1", "working");
        noteAgentActivity("agent-1", "working");
        noteAgentActivity("agent-1", "complete");
        await Promise.resolve();
        expect(recorded()).toEqual([
            ["activity_turn_started", { agentId: "agent-1", cwd: "/work/app" }],
            ["activity_turn_ended", { agentId: "agent-1", agent: "codex", cwd: "/work/app", sessionId: "s-1", configPath: null }],
        ]);
    });
});
