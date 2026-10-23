import { beforeEach, describe, expect, it } from "vitest";
import { activeAgentId } from "./selectors";
import { revealAgent } from "./commands";
import { getState, setState } from "./store";
import { withAgents } from "../test/agents";

const initial = getState();

/** Two projects, one agent each, so a blocked agent can be off-screen. */
function twoProjects(): void {
    setState(initial, true);
    const first = getState().activeSessionId;
    setState((state) => ({
        sessionOrder: [first, "s2"],
        sessions: { ...state.sessions, s2: { ...state.sessions[first], id: "s2", name: "openjob" } },
    }));
    setState((state) => withAgents(state, first, [{ id: "a1", type: "claude", title: "claude · api", cwd: "/one" }]));
    setState((state) => withAgents(state, "s2", [{ id: "a2", type: "codex", title: "codex · web", cwd: "/two" }]));
}

beforeEach(twoProjects);

describe("an agent waiting in another project", () => {
    it("switches project and opens the agent that was waiting", () => {
        revealAgent("a2");

        expect(getState().activeSessionId).toBe("s2");
        expect(activeAgentId(getState(), getState().sessions.s2)).toBe("a2");
    });
});
