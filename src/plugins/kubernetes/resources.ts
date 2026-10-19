import { resource } from "../../plugin-api/resources";
import { kubeApi, type KubeEvent, type KubeStatus, type Namespace, type Pod, type PodDetail, type Scope, type Workload } from "./api";

/** One key per cluster and namespace, so switching back shows what was read there. */
const keyOf = ([scope, all]: unknown[]) => `${(scope as Scope).context ?? ""}|${all ? "*" : ((scope as Scope).namespace ?? "")}`;

export const kubeStatusR = resource({
    kind: "kubernetes.status",
    fetch: (): Promise<KubeStatus> => kubeApi.status(),
    staleAfterMs: 30_000,
});

export const kubeNamespacesR = resource({
    kind: "kubernetes.namespaces",
    fetch: (context: string): Promise<Namespace[]> => kubeApi.namespaces({ context }),
    staleAfterMs: 60_000,
});

export const kubePodsR = resource({
    kind: "kubernetes.pods",
    fetch: (scope: Scope, all: boolean): Promise<Pod[]> => kubeApi.pods(scope, all),
    staleAfterMs: 5_000,
    keyFn: keyOf,
});

export const kubeWorkloadsR = resource({
    kind: "kubernetes.workloads",
    fetch: (scope: Scope, all: boolean): Promise<Workload[]> => kubeApi.workloads(scope, all),
    staleAfterMs: 5_000,
    keyFn: keyOf,
});

export const kubeEventsR = resource({
    kind: "kubernetes.events",
    fetch: (scope: Scope, all: boolean): Promise<KubeEvent[]> => kubeApi.events(scope, all),
    staleAfterMs: 10_000,
    keyFn: keyOf,
});

export const kubePodR = resource({
    kind: "kubernetes.pod",
    fetch: (scope: Scope, name: string): Promise<PodDetail | null> => kubeApi.pod(scope, name),
    staleAfterMs: 5_000,
    keyFn: ([scope, name]) => `${(scope as Scope).context ?? ""}|${(scope as Scope).namespace ?? ""}|${name as string}`,
});
