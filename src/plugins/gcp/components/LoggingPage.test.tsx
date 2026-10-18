import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const api = vi.hoisted(() => ({}) as ReturnType<typeof import("../testing").mockApi>);
vi.mock("../api", async (importOriginal) => {
    const { mockApi } = await import("../testing");
    Object.assign(api, mockApi());
    return { ...(await importOriginal<object>()), gcpApi: api };
});

import { invalidate } from "../../../plugin-api/resources";
import { answerEmpty } from "../testing";
import { LoggingPage } from "./LoggingPage";

const target = { account: null, project: "acme-prod" };
const entry = (id: string, text: string, severity = "INFO") => ({
    id,
    at: "2024-10-18T11:00:00Z",
    severity,
    text,
    log: "run.googleapis.com/stderr",
    resource: "cloud_run_revision",
    labels: null,
    http: null,
    json: { order: 42 },
    trace: null,
});

beforeEach(() => {
    Object.values(api).forEach((mock) => mock.mockReset());
    answerEmpty(api);
});

afterEach(() => {
    cleanup();
    invalidate((kind) => kind.startsWith("gcp."));
});

describe("Logs Explorer", () => {
    it("lists entries oldest first and opens one to its payload", async () => {
        api.logs.mockResolvedValue([entry("2", "second"), entry("1", "first", "ERROR")]);
        render(<LoggingPage paneId="logs-1" active target={target} />);
        const rows = within(await screen.findByRole("log", { name: "Log entries" })).getAllByRole("button");
        expect(rows[0]).toHaveTextContent("first");
        expect(rows[1]).toHaveTextContent("second");
        fireEvent.click(screen.getByRole("button", { name: /first/ }));
        expect(await screen.findByText(/"order": 42/)).toBeInTheDocument();
    });

    it("runs a query for one resource with a raw filter beside it", async () => {
        render(<LoggingPage paneId="logs-2" active target={target} />);
        fireEvent.click(screen.getByRole("button", { name: "Resource" }));
        fireEvent.click(await screen.findByRole("option", { name: "Cloud Run service" }));
        fireEvent.change(screen.getByRole("textbox", { name: "Resource name" }), { target: { value: "api" } });
        fireEvent.change(screen.getByRole("textbox", { name: "Logging query" }), { target: { value: "httpRequest.status>=500" } });
        fireEvent.click(screen.getByRole("button", { name: "Run query" }));
        await waitFor(() =>
            expect(api.logs).toHaveBeenLastCalledWith(
                target,
                expect.objectContaining({ resource: "runService", name: "api", filter: "httpRequest.status>=500", since: "1h" }),
            ),
        );
    });

    it("streams new entries while live and stops when asked", async () => {
        const stop = vi.fn();
        let push: (entries: unknown[]) => void = () => {};
        api.tailLogs.mockImplementation((_target, _query, handlers: { onEntries: (entries: unknown[]) => void }) => {
            push = handlers.onEntries;
            return { stop };
        });
        render(<LoggingPage paneId="logs-3" active target={target} />);
        fireEvent.click(await screen.findByRole("button", { name: "Stream logs" }));
        act(() => push([entry("9", "just now")]));
        expect(await screen.findByText("just now")).toBeInTheDocument();
        fireEvent.click(screen.getByRole("button", { name: "Stop streaming" }));
        expect(stop).toHaveBeenCalled();
    });
});
