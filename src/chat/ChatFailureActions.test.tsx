import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { Agent } from "../state/types";

const { signInAgent } = vi.hoisted(() => ({ signInAgent: vi.fn() }));
vi.mock("../agents/AgentInstall", () => ({ signInAgent }));

import { ChatFailureActions } from "./ChatFailureActions";

const agent = (type: Agent["type"], executablePath?: string): Agent => ({ id: `${type}-1`, type, title: type, startup: "", executablePath });

afterEach(() => {
    cleanup();
    vi.clearAllMocks();
});

describe("a chat that failed for want of a sign-in", () => {
    it("runs the agent's own sign-in for an agent without accounts", () => {
        render(
            <ChatFailureActions agent={agent("grok", "/Users/me/.grok/bin/grok")} profile={undefined} failure={{ kind: "signIn", account: null }} />,
        );
        fireEvent.click(screen.getByRole("button", { name: "Sign in to Grok" }));
        expect(signInAgent).toHaveBeenCalledWith({ type: "grok", command: "/Users/me/.grok/bin/grok", configPath: null });
    });

    it("offers nothing for other failures, or an agent with no sign-in to run", () => {
        const { container, rerender } = render(
            <ChatFailureActions agent={agent("opencode")} profile={undefined} failure={{ kind: "limit", account: null }} />,
        );
        expect(container).toBeEmptyDOMElement();
        rerender(<ChatFailureActions agent={agent("muse")} profile={undefined} failure={{ kind: "signIn", account: null }} />);
        expect(container).toBeEmptyDOMElement();
    });
});
