import { resource } from "../../plugin-api/resources";
import { gcpApi, type ChartKind, type GcpStatus, type Target } from "./api";

/** Resources are keyed by plain strings; an empty account means gcloud's active one. */
const target = (account: string, project: string): Target => ({ account: account || null, project });

export const gcpStatusR = resource({ kind: "gcp.status", fetch: (): Promise<GcpStatus> => gcpApi.status(), staleAfterMs: 60_000 });
export const gcpProjectsR = resource({
    kind: "gcp.projects",
    fetch: (account: string) => gcpApi.projects(account || null),
    staleAfterMs: 5 * 60_000,
});

export const runServicesR = resource({
    kind: "gcp.run.services",
    fetch: (account: string, project: string) => gcpApi.runServices(target(account, project)),
    staleAfterMs: 30_000,
});
export const runServiceR = resource({
    kind: "gcp.run.service",
    fetch: (account: string, project: string, location: string, service: string) => gcpApi.runService(target(account, project), location, service),
    staleAfterMs: 15_000,
});
export const runJobsR = resource({
    kind: "gcp.run.jobs",
    fetch: (account: string, project: string) => gcpApi.runJobs(target(account, project)),
    staleAfterMs: 30_000,
});
export const runJobR = resource({
    kind: "gcp.run.job",
    fetch: (account: string, project: string, location: string, job: string) => gcpApi.runJob(target(account, project), location, job),
    staleAfterMs: 15_000,
});
export const functionsR = resource({
    kind: "gcp.functions",
    fetch: (account: string, project: string) => gcpApi.functions(target(account, project)),
    staleAfterMs: 30_000,
});
export const instancesR = resource({
    kind: "gcp.instances",
    fetch: (account: string, project: string) => gcpApi.instances(target(account, project)),
    staleAfterMs: 20_000,
});
export const instanceR = resource({
    kind: "gcp.instance",
    fetch: (account: string, project: string, zone: string, instance: string) => gcpApi.instance(target(account, project), zone, instance),
    staleAfterMs: 15_000,
});
export const serialR = resource({
    kind: "gcp.serial",
    fetch: (account: string, project: string, zone: string, instance: string) => gcpApi.instanceSerial(target(account, project), zone, instance),
    staleAfterMs: 10_000,
});
export const clustersR = resource({
    kind: "gcp.clusters",
    fetch: (account: string, project: string) => gcpApi.clusters(target(account, project)),
    staleAfterMs: 30_000,
});
export const workloadsR = resource({
    kind: "gcp.kube.workloads",
    fetch: (account: string, project: string, location: string, cluster: string) => gcpApi.workloads(target(account, project), location, cluster),
    staleAfterMs: 15_000,
});
export const podsR = resource({
    kind: "gcp.kube.pods",
    fetch: (account: string, project: string, location: string, cluster: string, selector: string) =>
        gcpApi.pods(target(account, project), location, cluster, selector || null),
    staleAfterMs: 10_000,
});
export const kubeServicesR = resource({
    kind: "gcp.kube.services",
    fetch: (account: string, project: string, location: string, cluster: string) => gcpApi.kubeServices(target(account, project), location, cluster),
    staleAfterMs: 30_000,
});
export const bucketsR = resource({
    kind: "gcp.buckets",
    fetch: (account: string, project: string) => gcpApi.buckets(target(account, project)),
    staleAfterMs: 60_000,
});
export const folderR = resource({
    kind: "gcp.objects",
    fetch: (account: string, project: string, bucket: string, prefix: string) => gcpApi.objects(target(account, project), bucket, prefix),
    staleAfterMs: 30_000,
});
export const sqlInstancesR = resource({
    kind: "gcp.sql",
    fetch: (account: string, project: string) => gcpApi.sqlInstances(target(account, project)),
    staleAfterMs: 30_000,
});
export const sqlInstanceR = resource({
    kind: "gcp.sql.instance",
    fetch: (account: string, project: string, instance: string) => gcpApi.sqlInstance(target(account, project), instance),
    staleAfterMs: 20_000,
});
export const pubsubR = resource({
    kind: "gcp.pubsub",
    fetch: (account: string, project: string) => gcpApi.pubsub(target(account, project)),
    staleAfterMs: 30_000,
});
export const buildsR = resource({
    kind: "gcp.builds",
    fetch: (account: string, project: string) => gcpApi.builds(target(account, project)),
    staleAfterMs: 15_000,
});
export const buildR = resource({
    kind: "gcp.build",
    fetch: (account: string, project: string, id: string, region: string) => gcpApi.build(target(account, project), id, region || null),
    staleAfterMs: 10_000,
});
export const triggersR = resource({
    kind: "gcp.triggers",
    fetch: (account: string, project: string) => gcpApi.triggers(target(account, project)),
    staleAfterMs: 60_000,
});
export const secretsR = resource({
    kind: "gcp.secrets",
    fetch: (account: string, project: string) => gcpApi.secrets(target(account, project)),
    staleAfterMs: 60_000,
});
export const secretR = resource({
    kind: "gcp.secret",
    fetch: (account: string, project: string, secret: string) => gcpApi.secret(target(account, project), secret),
    staleAfterMs: 30_000,
});
export const metricsR = resource({
    kind: "gcp.metrics",
    fetch: (account: string, project: string, chart: ChartKind, name: string, since: string) =>
        gcpApi.metrics(target(account, project), chart, name, since),
    staleAfterMs: 60_000,
});

export const recentErrorsR = resource({
    kind: "gcp.logs.errors",
    fetch: (account: string, project: string) => gcpApi.logs(target(account, project), { severity: "ERROR", since: "1h", limit: 8 }),
    staleAfterMs: 60_000,
});

export const agentPicksR = resource({ kind: "gcp.agentPicks", fetch: (folder: string | null) => gcpApi.agentPicks(folder), staleAfterMs: 60_000 });

export const errorGroupsR = resource({
    kind: "gcp.errors",
    fetch: (account: string, project: string, since: string) => gcpApi.errorGroups(target(account, project), since),
    staleAfterMs: 60_000,
});
export const errorEventsR = resource({
    kind: "gcp.errors.events",
    fetch: (account: string, project: string, group: string) => gcpApi.errorEvents(target(account, project), group),
    staleAfterMs: 60_000,
});
export const repositoriesR = resource({
    kind: "gcp.repositories",
    fetch: (account: string, project: string) => gcpApi.repositories(target(account, project)),
    staleAfterMs: 60_000,
});
export const repositoryR = resource({
    kind: "gcp.repository",
    fetch: (account: string, project: string, location: string, repository: string) =>
        gcpApi.repository(target(account, project), location, repository),
    staleAfterMs: 60_000,
});
export const schedulerJobsR = resource({
    kind: "gcp.scheduler",
    fetch: (account: string, project: string) => gcpApi.schedulerJobs(target(account, project)),
    staleAfterMs: 30_000,
});
export const iamMembersR = resource({
    kind: "gcp.iam",
    fetch: (account: string, project: string) => gcpApi.iamMembers(target(account, project)),
    staleAfterMs: 5 * 60_000,
});
export const serviceAccountsR = resource({
    kind: "gcp.iam.accounts",
    fetch: (account: string, project: string) => gcpApi.serviceAccounts(target(account, project)),
    staleAfterMs: 5 * 60_000,
});
export const datasetsR = resource({
    kind: "gcp.bq.datasets",
    fetch: (account: string, project: string) => gcpApi.datasets(target(account, project)),
    staleAfterMs: 5 * 60_000,
});
export const tablesR = resource({
    kind: "gcp.bq.tables",
    fetch: (account: string, project: string, dataset: string) => gcpApi.tables(target(account, project), dataset),
    staleAfterMs: 5 * 60_000,
});
export const tableR = resource({
    kind: "gcp.bq.table",
    fetch: (account: string, project: string, dataset: string, table: string) => gcpApi.table(target(account, project), dataset, table),
    staleAfterMs: 5 * 60_000,
});

export const nodesR = resource({
    kind: "gcp.kube.nodes",
    fetch: (account: string, project: string, location: string, cluster: string) => gcpApi.nodes(target(account, project), location, cluster),
    staleAfterMs: 30_000,
});
export const podEventsR = resource({
    kind: "gcp.kube.events",
    fetch: (account: string, project: string, location: string, cluster: string, namespace: string, pod: string) =>
        gcpApi.podEvents(target(account, project), location, cluster, namespace, pod),
    staleAfterMs: 10_000,
});

export const billingR = resource({
    kind: "gcp.billing",
    fetch: (account: string, project: string) => gcpApi.billing(target(account, project)),
    staleAfterMs: 10 * 60_000,
});

export const snapshotsR = resource({
    kind: "gcp.snapshots",
    fetch: (account: string, project: string) => gcpApi.snapshots(target(account, project)),
    staleAfterMs: 60_000,
});
