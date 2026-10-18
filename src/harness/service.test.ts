import { beforeEach, afterEach, describe, expect, it, vi } from "vitest";
import { useStore } from "../state/store";
import { createProjectSession } from "../state/commands";
import { installIpcTransportForTests, MemoryIpcTransport, resetIpcTransportForTests } from "../api/transport";
import { loadProjectConfig } from "../projects/projectConfig";
import { handleHarnessRequest, type HarnessRequest } from "./service";
import { withAgents } from "../test/agents";

vi.mock("@tauri-apps/api/core", async (original) => ({
    ...(await original<object>()),
    Channel: class {
        onmessage = () => {};
    },
}));
vi.mock("../projects/projectConfig", async (original) => ({ ...(await original<object>()), loadProjectConfig: vi.fn() }));
const initial = useStore.getState();
let transport: MemoryIpcTransport;
const config = {
    status: "valid" as const,
    path: "/one/sikemux.json",
    fingerprint: "original",
    config: {
        version: 1 as const,
        actions: [],
        tasks: [{ id: "test", label: "Test", command: "echo test", cwd: ".", env: { PRIVATE: "not-in-inspection" } }],
        preview: { url: "http://localhost:5173", command: "echo test" },
    },
    trust: { requiresApproval: true, executableEntries: 1, reasons: [] },
    trusted: true,
};
function request(method: string, params = {}): HarnessRequest {
    return { id: crypto.randomUUID(), project: "/one", agentId: null, method, params };
}
function addAgents(agents: Parameters<typeof withAgents>[2]) {
    const state = useStore.getState();
    const session = Object.values(state.sessions).find((session) => session.cwd === "/one")!;
    useStore.setState(withAgents(state, session.id, agents));
    return state;
}

beforeEach(() => {
    useStore.setState(initial, true);
    createProjectSession("/one");
    createProjectSession("/two");
    transport = new MemoryIpcTransport();
    installIpcTransportForTests(transport);
    vi.mocked(loadProjectConfig).mockResolvedValue(config);
});
afterEach(() => {
    vi.restoreAllMocks();
    resetIpcTransportForTests();
    useStore.setState(initial, true);
});

describe("harness window service", () => {
    it("gives only the requested project's panes, leaving tasks, ports and runs to the core", async () => {
        const result = await handleHarnessRequest(request("workspace.inspect"));
        expect(result).toMatchObject({ active: false, windows: expect.any(Array) });
        for (const key of ["tasks", "ports", "runs", "cursor"]) expect(result).not.toHaveProperty(key);
        await expect(handleHarnessRequest({ ...request("workspace.inspect"), project: "/missing" })).rejects.toThrow("not open");
        await expect(handleHarnessRequest({ ...request("workspace.inspect"), agentId: "other-agent" })).rejects.toThrow("does not belong");
    });
    it("queues a file open in an unmounted editor without stealing focus", async () => {
        transport.register("harness_resolve_path", () => "/one/file.ts");
        const before = useStore.getState().activeSessionId;
        await handleHarnessRequest(request("ui.open", { kind: "file", path: "file.ts", line: 12 }));
        const state = useStore.getState();
        expect(state.activeSessionId).toBe(before);
        expect(Object.values(state.editorViews).some((view) => view.openTabs.includes("/one/file.ts") && view.activePath === null)).toBe(true);
        expect(Object.values(state.pendingEditorOpens).flat()).toEqual([]);
        await handleHarnessRequest(request("ui.open", { kind: "file", path: "file.ts", line: 12, focus: true }));
        expect(Object.values(useStore.getState().pendingEditorOpens).flat()).toEqual([expect.objectContaining({ path: "/one/file.ts", line: 11 })]);
        await handleHarnessRequest(request("ui.open", { kind: "diff", focus: true }));
        expect(useStore.getState().sessions[useStore.getState().activeSessionId].cwd).toBe("/one");
    });
    it("opens a file an agent asks for on that agent's desk, at the line it names", async () => {
        const state = addAgents([{ id: "fixture-agent", type: "codex", title: "Fixture", startup: "" }]);
        transport.register("harness_resolve_path", () => "/one/file.ts");
        const result = await handleHarnessRequest({ ...request("ui.open", { kind: "file", path: "file.ts", line: 12 }), agentId: "fixture-agent" });
        expect(result).toEqual({ kind: "file", agentId: "fixture-agent", path: "/one/file.ts" });
        expect(useStore.getState().desks["fixture-agent"]).toMatchObject({
            active: "file:/one/file.ts",
            reveal: { path: "/one/file.ts", line: 11, character: 0 },
        });
        expect(useStore.getState().activeSessionId).toBe(state.activeSessionId);
    });
    it("opens the configured preview in the requesting agent's browser", async () => {
        const state = addAgents([{ id: "fixture-agent", type: "codex", title: "Fixture", startup: "" }]);
        const open = vi.fn(() => "fixture-tab");
        transport.register("browser_new_tab", open);
        const result = await handleHarnessRequest({ ...request("ui.open", { kind: "preview" }), agentId: "fixture-agent" });
        expect(result).toEqual({ kind: "preview", tabId: "fixture-tab", url: "http://localhost:5173" });
        expect(open).toHaveBeenCalledWith({ agentId: "fixture-agent", url: "http://localhost:5173" }, expect.anything());
        expect(useStore.getState().activeSessionId).toBe(state.activeSessionId);
    });
    it("rejects preview opens without an agent and methods the core answers itself", async () => {
        await expect(handleHarnessRequest(request("ui.open", { kind: "preview" }))).rejects.toThrow("agent session");
        await expect(handleHarnessRequest(request("task.read", { taskId: "test" }))).rejects.toThrow("Unknown harness method");
    });
});
