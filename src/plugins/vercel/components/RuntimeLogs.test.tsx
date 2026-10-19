import { act, cleanup, fireEvent, render, screen, within } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { PluginStreamHandlers } from "../../../plugin-api/backend";
import type { RuntimeLogLine, VercelDeployment } from "../api";

const streams = vi.hoisted(() => ({ handlers: [] as PluginStreamHandlers<{ lines: RuntimeLogLine[] }>[], stopped: 0 }));
const api = vi.hoisted(() => ({
    followRuntimeLogs: vi.fn((_team: string, _project: string, _id: string, handlers: PluginStreamHandlers<{ lines: RuntimeLogLine[] }>) => {
        streams.handlers.push(handlers);
        return { stop: () => (streams.stopped += 1) };
    }),
}));
vi.mock("../api", async (importOriginal) => ({ ...(await importOriginal<object>()), vercelApi: api }));

import { keeps, RuntimeLogs } from "./RuntimeLogs";

const line = (patch: Partial<RuntimeLogLine>): RuntimeLogLine => ({
    at: 1_792_300_000_000,
    level: "info",
    source: "serverless",
    message: "ok",
    method: "GET",
    path: "/",
    status: 200,
    domain: "shop.acme.dev",
    ...patch,
});
const crash = line({ level: "error", message: "TypeError: cart is undefined", method: "POST", path: "/api/checkout", status: 500 });
const missing = line({ level: "warning", path: "/favicon.png", status: 404, source: "static" });
const deployment = { id: "dpl_live", project: "storefront", projectId: "prj_store" } as VercelDeployment;

beforeEach(() => {
    streams.handlers = [];
    streams.stopped = 0;
    api.followRuntimeLogs.mockClear();
});
afterEach(cleanup);

describe("runtime logs", () => {
    it("keep the level, status, source and words asked for", () => {
        const all = { level: "all" as const, status: "all" as const, source: "", path: "" };
        expect(keeps(crash, { ...all, level: "error" })).toBe(true);
        expect(keeps(missing, { ...all, level: "error" })).toBe(false);
        expect(keeps(missing, { ...all, level: "warning" })).toBe(true);
        expect(keeps(missing, { ...all, status: "4xx" })).toBe(true);
        expect(keeps(crash, { ...all, status: "4xx" })).toBe(false);
        expect(keeps(crash, { ...all, status: "5xx" })).toBe(true);
        expect(keeps(missing, { ...all, source: "serverless" })).toBe(false);
        expect(keeps(crash, { ...all, path: "checkout" })).toBe(true);
        expect(keeps(crash, { ...all, path: "typeerror" })).toBe(true);
    });

    it("listen only when asked, show what arrives, and stop", () => {
        render(<RuntimeLogs team="team_acme" deployment={deployment} />);
        expect(screen.getByText(/keeps no runtime logs from before you listen/)).toBeInTheDocument();
        expect(api.followRuntimeLogs).not.toHaveBeenCalled();
        fireEvent.click(screen.getByRole("button", { name: "Listen" }));
        expect(api.followRuntimeLogs).toHaveBeenCalledWith("team_acme", "prj_store", "dpl_live", expect.anything());
        act(() => streams.handlers[0]!.onItem({ lines: [crash, missing] }));
        const log = screen.getByRole("log", { name: "Runtime log" });
        expect(within(log).getByText("TypeError: cart is undefined")).toBeInTheDocument();
        fireEvent.click(within(screen.getByRole("group", { name: "Status" })).getByRole("button", { name: "5xx" }));
        expect(within(log).queryByText("/favicon.png")).not.toBeInTheDocument();
        expect(screen.getByRole("status")).toHaveTextContent("1 of 2");
        fireEvent.click(screen.getByRole("button", { name: "Stop" }));
        expect(streams.stopped).toBe(1);
    });
});
