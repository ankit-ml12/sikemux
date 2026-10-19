import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { NIGHTLY, noKubectl, ready, WEB, WEB_POD, WORKER, WORKER_POD } from "../testing";

const api = vi.hoisted(() => ({
    status: vi.fn(),
    namespaces: vi.fn(),
    workloads: vi.fn(),
    pods: vi.fn(),
    events: vi.fn(),
}));
vi.mock("../api", async (importOriginal) => ({ ...(await importOriginal<object>()), kubeApi: api }));

import { invalidate } from "../../../plugin-api/resources";
import { KubernetesPane } from "./KubernetesPane";

let pane = 0;
const renderPane = () => render(<KubernetesPane paneId={`k8s-${++pane}`} active />);

afterEach(() => {
    cleanup();
    invalidate((kind) => kind.startsWith("kubernetes."));
});
beforeEach(() => {
    Object.values(api).forEach((mock) => mock.mockReset());
    api.status.mockResolvedValue(ready);
    api.namespaces.mockResolvedValue([
        { name: "default", phase: "Active" },
        { name: "shop", phase: "Active" },
    ]);
    api.workloads.mockResolvedValue([WORKER, WEB, NIGHTLY]);
    api.pods.mockResolvedValue([]);
    api.events.mockResolvedValue([]);
});

describe("the Kubernetes pane", () => {
    it("says what is missing before there is a cluster to show", async () => {
        api.status.mockResolvedValue(noKubectl);
        renderPane();
        expect(await screen.findByText("kubectl isn't installed")).toBeInTheDocument();
        expect(screen.getByText(/brew install kubernetes-cli/)).toBeInTheDocument();
    });

    it("lists the current cluster's workloads, flagging the ones in trouble", async () => {
        renderPane();
        const table = await screen.findByRole("table", { name: "Workloads" });
        expect(within(table).getAllByRole("row")).toHaveLength(4);
        expect(within(table).getByText("None of 1 ready")).toBeInTheDocument();
        expect(within(table).getByText(/0 2 \* \* \*/)).toBeInTheDocument();
        expect(api.workloads).toHaveBeenCalledWith({ context: "kind-dev", namespace: "default" }, false);
        const nav = screen.getByRole("navigation", { name: "Kubernetes" });
        expect(within(nav).getByRole("button", { name: /Workloads/ })).toHaveTextContent("1");
        fireEvent.click(within(screen.getByRole("group", { name: "Show" })).getByRole("button", { name: /^Needs attention/ }));
        expect(within(table).getAllByRole("row")).toHaveLength(2);
    });

    it("reads every namespace at once when asked", async () => {
        renderPane();
        await screen.findByRole("table", { name: "Workloads" });
        fireEvent.click(screen.getByRole("button", { name: /Namespace/ }));
        fireEvent.click(await screen.findByRole("option", { name: /All namespaces/ }));
        await waitFor(() => expect(api.workloads).toHaveBeenLastCalledWith({ context: "kind-dev", namespace: undefined }, true));
    });

    it("opens a workload's own pods, worst first, and lets the narrowing go", async () => {
        api.pods.mockResolvedValue([WORKER_POD, WEB_POD]);
        renderPane();
        const workloads = await screen.findByRole("table", { name: "Workloads" });
        fireEvent.click(within(workloads).getByRole("row", { name: "Deployment worker" }));
        const pods = await screen.findByRole("table", { name: "Pods" });
        expect(api.pods).toHaveBeenLastCalledWith({ context: "kind-dev", namespace: "default" }, false, "app=worker");
        expect(within(pods).getByText("CrashLoopBackOff")).toBeInTheDocument();
        expect(within(pods).getByText("6")).toHaveClass("k8s-restarts");
        fireEvent.click(screen.getByRole("button", { name: /app=worker/ }));
        await waitFor(() => expect(api.pods).toHaveBeenLastCalledWith({ context: "kind-dev", namespace: "default" }, false, null));
    });

    it("finds pods by name, owner or node", async () => {
        api.pods.mockResolvedValue([WORKER_POD, WEB_POD]);
        renderPane();
        fireEvent.click(within(await screen.findByRole("navigation", { name: "Kubernetes" })).getByRole("button", { name: /Pods/ }));
        const pods = await screen.findByRole("table", { name: "Pods" });
        fireEvent.change(screen.getByRole("textbox", { name: "Find" }), { target: { value: "web" } });
        expect(within(pods).getAllByRole("row")).toHaveLength(2);
        fireEvent.click(within(screen.getByRole("group", { name: "Show" })).getByRole("button", { name: /^Needs attention/ }));
        expect(screen.getByText("No pods match.")).toBeInTheDocument();
    });
});
