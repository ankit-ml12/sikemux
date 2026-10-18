import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { VercelDeployment, VercelLog, VercelProject, VercelStatus } from "../api";

const api = vi.hoisted(() => ({
    status: vi.fn(),
    signIn: vi.fn(),
    signOut: vi.fn(),
    setTeam: vi.fn(),
    projects: vi.fn(),
    deployments: vi.fn(),
    deployment: vi.fn(),
    logs: vi.fn(),
    domains: vi.fn(),
    env: vi.fn(),
    redeploy: vi.fn(),
    promote: vi.fn(),
    cancel: vi.fn(),
}));
vi.mock("../api", async (importOriginal) => ({ ...(await importOriginal<object>()), vercelApi: api }));

import { acceptDialog, useDialogs } from "../../../state/dialog";
import { invalidate } from "../../../plugin-api/resources";
import { deploymentForAgent } from "../state";
import { VercelPane } from "./VercelPane";

const signedIn: VercelStatus = {
    configured: true,
    ok: true,
    authFailed: false,
    message: null,
    account: { id: "u1", username: "ankit", name: "Ankit", email: "a@example.com" },
    teams: [{ id: "team_1", slug: "acme", name: "Acme" }],
    team: "team_1",
    personal: true,
};

const project: VercelProject = {
    id: "prj_1",
    name: "web",
    framework: "nextjs",
    updatedAt: Date.now() - 60_000,
    repo: "acme/web",
    gitProvider: "github",
    productionBranch: "main",
    production: { id: "dpl_live", url: "https://web.acme.dev", state: "READY", createdAt: Date.now() - 3_600_000 },
};

const failed: VercelDeployment = {
    id: "dpl_bad",
    project: "web",
    projectId: "prj_1",
    url: "https://web-bad.vercel.app",
    state: "ERROR",
    target: "preview",
    createdAt: Date.now() - 120_000,
    readyAt: null,
    branch: "fix-nav",
    commit: "abc1234def",
    message: "Fix the nav",
    author: "Ankit",
    inspectorUrl: "https://vercel.com/acme/web/bad",
    aliases: null,
    error: 'Command "npm run build" exited with 1',
};

const ready: VercelDeployment = {
    ...failed,
    id: "dpl_ok",
    state: "READY",
    message: "Tidy the footer",
    error: null,
    url: "https://web-ok.vercel.app",
};

const log: VercelLog = {
    deployment: failed,
    lines: [
        { at: 1, kind: "command", text: "npm run build" },
        { at: 2, kind: "stderr", text: "Type error: nav is not defined" },
    ],
    skipped: 0,
};

let pane = 0;
const renderPane = () => render(<VercelPane paneId={`vercel-${++pane}`} active />);

afterEach(() => {
    cleanup();
    invalidate((kind) => kind.startsWith("vercel."));
});

beforeEach(() => {
    Object.values(api).forEach((mock) => mock.mockReset());
    api.status.mockResolvedValue(signedIn);
    api.projects.mockResolvedValue([project]);
    api.deployments.mockResolvedValue([failed, ready]);
    api.logs.mockImplementation((_team: string, id: string) => Promise.resolve(id === "dpl_ok" ? { ...log, deployment: ready } : log));
    api.domains.mockResolvedValue([{ name: "web.acme.dev", verified: true, redirect: null, gitBranch: null }]);
    api.env.mockResolvedValue([
        { id: "e1", key: "DATABASE_URL", target: ["production", "preview"], type: "encrypted", gitBranch: null, updatedAt: 1 },
    ]);
    api.promote.mockResolvedValue(ready);
    api.redeploy.mockResolvedValue(failed);
    api.setTeam.mockResolvedValue(undefined);
});

describe("the Vercel pane", () => {
    it("asks for an access token when no account is signed in", async () => {
        api.status.mockResolvedValue({ ...signedIn, configured: false, account: null, teams: [], team: null, personal: true });
        renderPane();
        expect(await screen.findByRole("heading", { name: "Connect Vercel" })).toBeInTheDocument();
        fireEvent.change(screen.getByPlaceholderText("Paste a Vercel token"), { target: { value: "tok_123" } });
        api.signIn.mockResolvedValue(signedIn);
        fireEvent.click(screen.getByRole("button", { name: "Sign in" }));
        await waitFor(() => expect(api.signIn).toHaveBeenCalledWith("tok_123"));
    });

    it("lists the team's projects and opens one on its deployments", async () => {
        renderPane();
        fireEvent.click(await screen.findByRole("button", { name: /web\.acme\.dev/ }));
        expect(api.projects).toHaveBeenCalledWith("team_1");
        expect(await screen.findByText("Fix the nav")).toBeInTheDocument();
        expect(api.deployments).toHaveBeenCalledWith("team_1", "web");
        expect(screen.getByText("Tidy the footer")).toBeInTheDocument();
        expect(screen.getAllByText("fix-nav · abc1234 · Ankit")).toHaveLength(2);
    });

    it("shows a failed deployment's error and build log, and hands both to an agent", async () => {
        renderPane();
        fireEvent.click(await screen.findByRole("button", { name: /web\.acme\.dev/ }));
        fireEvent.click(await screen.findByRole("button", { name: /Fix the nav/ }));
        expect(await screen.findByText("Type error: nav is not defined")).toBeInTheDocument();
        expect(screen.getByText('Command "npm run build" exited with 1')).toBeInTheDocument();
        expect(screen.queryByRole("button", { name: "Promote to production" })).not.toBeInTheDocument();

        const text = deploymentForAgent(failed, log);
        expect(text).toContain("Vercel deployment dpl_bad of web — https://web-bad.vercel.app");
        expect(text).toContain('Error: Command "npm run build" exited with 1');
        expect(text).toContain("Type error: nav is not defined");
        expect(text).toContain("vercel_logs or vercel_deployment with id dpl_bad");
    });

    it("promotes a ready deployment once the person confirms", async () => {
        renderPane();
        fireEvent.click(await screen.findByRole("button", { name: /web\.acme\.dev/ }));
        fireEvent.click(await screen.findByRole("button", { name: /Tidy the footer/ }));
        fireEvent.click(await screen.findByRole("button", { name: "Promote to production" }));
        await waitFor(() => expect(useDialogs.getState().dialog).toMatchObject({ kind: "confirm", title: "Promote to production?" }));
        expect(api.promote).not.toHaveBeenCalled();
        act(() => acceptDialog(useDialogs.getState().dialog!.id));
        await waitFor(() => expect(api.promote).toHaveBeenCalledWith("team_1", "dpl_ok"));
    });

    it("lists domains, and environment variable names without values", async () => {
        renderPane();
        fireEvent.click(await screen.findByRole("button", { name: /web\.acme\.dev/ }));
        fireEvent.click(await screen.findByRole("button", { name: "Domains" }));
        expect(await screen.findByText("Verified")).toBeInTheDocument();
        fireEvent.click(screen.getByRole("button", { name: "Environment" }));
        expect(await screen.findByText("DATABASE_URL")).toBeInTheDocument();
        expect(screen.getByText("production, preview")).toBeInTheDocument();
        expect(screen.getByText("Names only. Values stay in Vercel.")).toBeInTheDocument();
    });
});
