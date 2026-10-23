import { beforeEach, describe, expect, it } from "vitest";
import * as cmd from "../commands";
import { getState, setState } from "../store";
import type { TaskTerminalRequest } from "./taskTerminal";

const initial = getState();

beforeEach(() => {
    setState(initial, true);
});

function presentation(overrides: Partial<TaskTerminalRequest> = {}): TaskTerminalRequest {
    return {
        terminalKey: "task:test:/work/demo",
        label: "Test",
        project: "/work/demo",
        cwd: "/work/demo",
        ...overrides,
    };
}

describe("task terminal presentation", () => {
    it("creates a transient secret-free window in the owning project", () => {
        cmd.createProjectSession("/work/demo");
        const paneId = cmd.openTaskTerminal(presentation());
        const state = getState();
        const session = state.sessions[state.activeSessionId];
        const window = state.windows[session.activeWindowId];

        expect(window).toMatchObject({ name: "Test", role: "term", transient: true, activePaneId: paneId });
        expect(window.root).toMatchObject({
            type: "pane",
            id: paneId,
            cwd: "/work/demo",
            title: "Test",
            externalPty: true,
            taskTerminalKey: "task:test:/work/demo",
        });
        expect(JSON.stringify(window)).not.toContain("pnpm test");
        expect(JSON.stringify(window)).not.toContain("NODE_ENV");
    });

    it("reuses the same task pane while updating safe presentation metadata", () => {
        cmd.createProjectSession("/work/demo");
        const first = cmd.openTaskTerminal(presentation());
        const second = cmd.openTaskTerminal(presentation({ label: "Test again", cwd: "/work/demo/package" }));
        const state = getState();
        const taskWindows = Object.values(state.windows).filter((window) => window.transient);

        expect(second).toBe(first);
        expect(taskWindows).toHaveLength(1);
        expect(taskWindows[0]).toMatchObject({ name: "Test again", root: { id: first, cwd: "/work/demo/package", title: "Test again" } });
    });

    it("rejects closed projects and escaped directories", () => {
        expect(() => cmd.openTaskTerminal(presentation())).toThrow("no longer open");
        cmd.createProjectSession("/work/demo");
        expect(() => cmd.openTaskTerminal(presentation({ cwd: "/work/outside" }))).toThrow("stay within");
    });
});
