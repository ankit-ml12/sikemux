import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { useEffect } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { PtyDirectCommand } from "../state/types/domain";

const mocks = vi.hoisted(() => ({
    exited: null as null | ((event: { payload: { id: number; code: number | null } }) => void),
    refreshStatuses: vi.fn(async () => {}),
    fetchResource: vi.fn(),
    invalidate: vi.fn(),
}));

vi.mock("../terminal/TerminalPane", () => ({
    TerminalPane: ({ directCommand, onPtySession }: { directCommand: PtyDirectCommand; onPtySession: (id: number) => void }) => {
        useEffect(() => onPtySession(7), [onPtySession]);
        return <pre data-testid="terminal">{directCommand.args.join(" ")}</pre>;
    },
}));
vi.mock("../api/transport", () => ({
    getIpcTransport: () => ({
        subscribe: async (_event: string, listener: typeof mocks.exited) => {
            mocks.exited = listener;
            return () => {};
        },
    }),
}));
vi.mock("../api/agents", () => ({ agentApi: { refreshStatuses: mocks.refreshStatuses } }));
vi.mock("../state/resources", () => ({ fetchResource: mocks.fetchResource, invalidate: mocks.invalidate }));
vi.mock("../state/resources.defs", () => ({ agentCatalogR: { kind: "agents.catalog" } }));

import { AgentInstallHost, installAgent } from "./AgentInstall";

const exit = (id: number, code: number | null) => act(() => mocks.exited!({ payload: { id, code } }));

beforeEach(() => {
    mocks.fetchResource.mockResolvedValue([{ type: "hermes", label: "Hermes", available: true, status: { state: "ready", account: null } }]);
});

afterEach(() => {
    cleanup();
    vi.clearAllMocks();
});

describe("installing an agent", () => {
    it("runs the agent's own installer in a terminal and finds the agent once it succeeds", async () => {
        render(<AgentInstallHost />);
        act(() => installAgent("hermes"));

        expect(await screen.findByTestId("terminal")).toHaveTextContent("-c curl -fsSL https://hermes-agent.nousresearch.com/install.sh | bash");
        expect(screen.getByRole("dialog", { name: "Install Hermes" })).toBeInTheDocument();
        expect(screen.getByRole("button", { name: "Cancel" })).toBeInTheDocument();

        exit(8, 0);
        expect(mocks.refreshStatuses).not.toHaveBeenCalled();
        exit(7, 0);

        expect(await screen.findByRole("status")).toHaveTextContent("Hermes is installed: Ready.");
        expect(mocks.refreshStatuses).toHaveBeenCalledOnce();
        expect(mocks.invalidate).toHaveBeenCalled();
        fireEvent.click(screen.getByRole("button", { name: "Close" }));
        expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    });

    it("says the installer failed and offers to run it again", async () => {
        render(<AgentInstallHost />);
        act(() => installAgent("grok"));
        await screen.findByTestId("terminal");

        exit(7, 1);
        expect(screen.getByRole("status")).toHaveTextContent("The installer stopped with exit code 1.");
        expect(mocks.refreshStatuses).not.toHaveBeenCalled();

        fireEvent.click(screen.getByRole("button", { name: "Try again" }));
        await waitFor(() => expect(screen.getByRole("status")).toHaveTextContent("Running Grok's own installer…"));
        fireEvent.click(screen.getByRole("button", { name: "Cancel" }));
    });

    it("says so when the installer succeeded but the agent still cannot be found", async () => {
        mocks.fetchResource.mockResolvedValue([]);
        render(<AgentInstallHost />);
        act(() => installAgent("omp"));
        await screen.findByTestId("terminal");

        exit(7, 0);
        expect(await screen.findByText(/can't find OMP yet/)).toBeInTheDocument();
        fireEvent.click(screen.getByRole("button", { name: "Close" }));
    });

    it("has nothing to run for an agent without an installer", () => {
        render(<AgentInstallHost />);
        act(() => installAgent("muse"));
        expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    });
});
