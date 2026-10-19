import type { KubeStatus, Pod, Workload } from "./api";

export const ready: KubeStatus = {
    ready: true,
    message: null,
    category: null,
    current: "kind-dev",
    contexts: [
        { name: "gke_acme_prod", cluster: "gke_acme_prod", server: "https://34.1.2.3", namespace: "shop" },
        { name: "kind-dev", cluster: "kind-dev", server: "https://127.0.0.1:6443", namespace: null },
    ],
};

export const noKubectl: KubeStatus = {
    ready: false,
    message: "kubernetes: kubectl isn't installed. Install it (brew install kubernetes-cli), then try again.",
    category: "cli-missing",
    current: null,
    contexts: [],
};

export function workload(name: string, patch: Partial<Workload> = {}): Workload {
    return {
        kind: "Deployment",
        name,
        namespace: "shop",
        ready: 2,
        desired: 2,
        health: "ok",
        note: null,
        images: ["nginx:alpine"],
        created: "2026-10-19T08:00:00Z",
        selector: `app=${name}`,
        schedule: null,
        suspended: null,
        lastRun: null,
        ...patch,
    };
}

export const WEB = workload("web");
export const WORKER = workload("worker", { ready: 0, desired: 1, health: "warning", note: "None of 1 ready", images: ["busybox:1.36"] });
export const NIGHTLY = workload("nightly-report", {
    kind: "CronJob",
    ready: null,
    desired: null,
    schedule: "0 2 * * *",
    suspended: false,
    selector: null,
});

export function pod(name: string, patch: Partial<Pod> = {}): Pod {
    return {
        name,
        namespace: "shop",
        status: "Running",
        phase: "Running",
        ready: 1,
        containers: 1,
        restarts: 0,
        created: "2026-10-19T08:00:00Z",
        node: "kind-dev-control-plane",
        ip: "10.244.0.5",
        owner: { kind: "Deployment", name: "web" },
        ...patch,
    };
}

export const WEB_POD = pod("web-56fd9f5c8b-fgprx");
export const WORKER_POD = pod("worker-546f6f965-9swqn", {
    status: "CrashLoopBackOff",
    ready: 0,
    restarts: 6,
    owner: { kind: "Deployment", name: "worker" },
});
