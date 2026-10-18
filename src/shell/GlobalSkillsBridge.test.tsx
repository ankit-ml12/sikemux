import { act, cleanup, render } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { installIpcTransportForTests, MemoryIpcTransport, resetIpcTransportForTests } from "../api/transport";
import { useToasts } from "../state/toast";
import { GlobalSkillsBridge } from "./GlobalSkillsBridge";

let transport: MemoryIpcTransport;

beforeEach(() => {
    transport = new MemoryIpcTransport();
    installIpcTransportForTests(transport);
    useToasts.setState({ toasts: [] });
});

afterEach(() => {
    cleanup();
    resetIpcTransportForTests();
});

describe("GlobalSkillsBridge", () => {
    it("offers to move a skill an agent saved in its own folder, and moves it when asked", async () => {
        const adopt = vi.fn(() => ({ name: "notes", description: "", path: "/data/skills/notes" }));
        const added = [{ agent: "Claude", name: "notes", path: "/home/.claude/skills/notes" }];
        transport.register("core_call", (args) => {
            const { method, params } = args as { method: string; params: unknown };
            if (method === "adoptSkill") return adopt(params);
            if (method === "skillsAdded") return added;
            return null;
        });
        render(<GlobalSkillsBridge />);
        await act(async () => {});

        await act(async () => transport.emit("core_changed", { topic: "skills", project: null, paths: null }));
        await act(async () => transport.emit("core_changed", { topic: "skills", project: null, paths: null }));
        expect(useToasts.getState().toasts).toHaveLength(1);
        const [toast] = useToasts.getState().toasts;
        expect(toast.text).toContain("Claude saved the skill notes in its own folder");
        expect(toast.action?.label).toBe("Move into Sikemux");

        await act(async () => toast.action?.run(toast.id));
        expect(adopt).toHaveBeenCalledWith({ path: "/home/.claude/skills/notes" });
        expect(useToasts.getState().toasts.some((each) => each.text.startsWith("notes is in Sikemux now"))).toBe(true);
    });
});
