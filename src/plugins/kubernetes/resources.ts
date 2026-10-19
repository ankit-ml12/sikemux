import { resource } from "../../plugin-api/resources";
import {
    kubeApi,
    type Ingress,
    type KubeEvent,
    type KubeStatus,
    type Namespace,
    type Pod,
    type PodDetail,
    type Scope,
    type Service,
    type Store,
    type Workload,
} from "./api";

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
    fetch: (scope: Scope, all: boolean, selector: string | null): Promise<Pod[]> => kubeApi.pods(scope, all, selector),
    staleAfterMs: 5_000,
    keyFn: (args) => `${keyOf(args)}|${(args[2] as string | null) ?? ""}`,
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

export const kubeNetworkR = resource({
    kind: "kubernetes.network",
    fetch: (scope: Scope, all: boolean): Promise<{ services: Service[]; ingresses: Ingress[] }> => kubeApi.network(scope, all),
    staleAfterMs: 15_000,
    keyFn: keyOf,
});

export const kubeConfigR = resource({
    kind: "kubernetes.config",
    fetch: (scope: Scope, all: boolean): Promise<{ configMaps: Store[]; secrets: Store[] }> => kubeApi.config(scope, all),
    staleAfterMs: 30_000,
    keyFn: keyOf,
});
