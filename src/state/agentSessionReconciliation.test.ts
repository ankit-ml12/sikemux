import { beforeEach, describe, expect, it, vi } from "vitest";

const renameSession = vi.hoisted(() => vi.fn(() => Promise.resolve()));
vi.mock("../api/agents", async (importOriginal) => {
    const actual = await importOriginal<typeof import("../api/agents")>();
    return { ...actual, agentApi: { ...actual.agentApi, renameSession } };
});
import { renameAgent } from "./commands";
import { getState, setState } from "./store";
import { agentWindowId } from "./selectors";
import { withAgents } from "../test/agents";

const initial = getState();

beforeEach(() => {
    setState(initial, true);
    renameSession.mockClear();
});

describe("naming an agent", () => {
    it("keeps the name the person gave an agent", () => {
        const state = getState();
        const sessionId = state.activeSessionId;
        const agentId = "renamed-agent";
        const slices = withAgents(state, sessionId, [{ id: agentId, type: "claude", title: "claude", createdAt: 100_000, launchState: "live" }]);
        setState({
            ...slices,
            sessions: {
                ...state.sessions,
                [sessionId]: { ...state.sessions[sessionId], kind: "project", cwd: "/repo", activeWindowId: agentWindowId(slices, agentId)! },
            },
        });

        renameAgent(agentId, "  Parser   rewrite  ");
        expect(getState().agents[agentId]).toMatchObject({ title: "Parser rewrite", renamed: true });
        expect(renameSession).not.toHaveBeenCalled();
        expect(getState().windows[agentWindowId(getState(), agentId)!].name).toBe("Parser rewrite");
    });
    it("writes the new name of an open chat to the provider's own session", () => {
        const state = getState();
        const sessionId = state.activeSessionId;
        setState({
            sessions: { ...state.sessions, [sessionId]: { ...state.sessions[sessionId], kind: "project", cwd: "/repo" } },
            ...withAgents(state, sessionId, [
                { id: "claude-chat", type: "claude", title: "Old", resumeId: "claude-session" },
                { id: "codex-chat", type: "codex", title: "Old", resumeId: "codex-session", executablePath: "/bin/codex" },
                { id: "fresh-chat", type: "codex", title: "Old" },
            ]),
        });

        renameAgent("claude-chat", "New name");
        renameAgent("codex-chat", "New name");
        renameAgent("fresh-chat", "New name");

        expect(renameSession).toHaveBeenCalledTimes(2);
        expect(renameSession).toHaveBeenCalledWith("claude", "/repo", "claude-session", "New name", undefined, undefined);
        expect(renameSession).toHaveBeenCalledWith("codex", "/repo", "codex-session", "New name", "/bin/codex", undefined);
        expect(getState().agents["fresh-chat"]).toMatchObject({ title: "New name", renamed: true });
    });
    it("ignores a blank or unchanged name", () => {
        const state = getState();
        const agentId = "unnamed-agent";
        setState(withAgents(state, state.activeSessionId, [{ id: agentId, type: "claude", title: "Provider title" }]));

        renameAgent(agentId, "   ");
        renameAgent(agentId, "Provider title");
        renameAgent(agentId, "<command-name>");
        renameAgent(agentId, "x".repeat(73));
        expect(getState().agents[agentId].title).toBe("Provider title");
        expect(getState().agents[agentId].renamed).toBeUndefined();
    });
});
