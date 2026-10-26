import { describe, expect, it } from "vitest";
import { activityText } from "./chatStatus";
import type { PermissionAsk, ChatMessage, ChatRows } from "./types";

const request: PermissionAsk = {
    requestId: "r",
    sessionId: "s",
    toolCall: { toolCallId: "t", title: "Run", row: { action: "other", target: "Run" } },
    options: [],
};
const message: ChatMessage = { id: "m1", role: "user", parts: [] };

describe("activityText", () => {
    const state = (overrides: Partial<ChatRows>) =>
        ({ permissions: [], running: false, messages: [], connection: "ready", ...overrides }) as ChatRows;

    it("stays quiet while a permission card is showing", () => {
        expect(activityText(state({ permissions: [request], running: true }), "Reading…")).toBeNull();
    });

    it("names the running tool, or thinks without one", () => {
        expect(activityText(state({ running: true }), "Reading…")).toBe("Reading…");
        expect(activityText(state({ running: true }), null)).toBe("Thinking…");
    });

    it("says the agent is reconnecting, with its attempt when known, ahead of the running tool", () => {
        expect(activityText(state({ running: true, retrying: { attempt: 3, connection: true } }), "Reading…")).toBe("Reconnecting… (attempt 3)");
        expect(activityText(state({ running: true, retrying: { connection: true } }), null)).toBe("Reconnecting…");
        expect(activityText(state({ running: true, retrying: { attempt: 2, connection: false } }), null)).toBe("Retrying… (attempt 2)");
    });

    it("shows the connection step under an existing transcript", () => {
        expect(activityText(state({ messages: [message], connection: "starting" }), null)).toBe("Starting agent…");
        expect(activityText(state({ messages: [message] }), null)).toBeNull();
    });

    it("leaves an empty transcript to the connection card", () => {
        expect(activityText(state({ connection: "starting" }), null)).toBeNull();
    });
});
