import { describe, expect, it } from "vitest";
import { normalizePermissionMode } from "./agentLaunch";

describe("agent launch policy", () => {
    it("presents only Normal and YOLO", () => {
        expect(normalizePermissionMode("opencode", "read-only")).toBe("workspace-write");
        expect(normalizePermissionMode("opencode", "bypass")).toBe("workspace-write");
        expect(normalizePermissionMode("claude", "bypass")).toBe("bypass");
    });
});
