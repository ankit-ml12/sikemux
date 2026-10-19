import { resource } from "../../plugin-api/resources";
import { sentryApi, type IssueDetail, type IssueQuery, type Member, type Organization, type Project, type Release, type SentryStatus } from "./api";

export const sentryStatusR = resource({
    kind: "sentry.status",
    fetch: (): Promise<SentryStatus> => sentryApi.status(),
    staleAfterMs: 60_000,
});

export const sentryOrganizationsR = resource({
    kind: "sentry.organizations",
    fetch: (): Promise<Organization[]> => sentryApi.organizations(),
    staleAfterMs: 300_000,
});

/** Keyed by organisation, so switching back shows what was read there. */
export const sentryProjectsR = resource({
    kind: "sentry.projects",
    fetch: (_org: string): Promise<Project[]> => sentryApi.projects(),
    staleAfterMs: 300_000,
});

export const sentryMembersR = resource({
    kind: "sentry.members",
    fetch: (_org: string): Promise<Member[]> => sentryApi.members(),
    staleAfterMs: 300_000,
});

export const sentryEnvironmentsR = resource({
    kind: "sentry.environments",
    fetch: (_org: string, project: string | null): Promise<string[]> => sentryApi.environments(project),
    staleAfterMs: 300_000,
});

export const sentryIssuesR = resource({
    kind: "sentry.issues",
    fetch: (_org: string, query: IssueQuery) => sentryApi.issues(query),
    staleAfterMs: 30_000,
    keyFn: ([org, query]) => `${org as string}|${JSON.stringify(query)}`,
});

export const sentryIssueR = resource({
    kind: "sentry.issue",
    fetch: (_org: string, id: string): Promise<IssueDetail> => sentryApi.issue(id),
    staleAfterMs: 30_000,
});

export const sentryReleasesR = resource({
    kind: "sentry.releases",
    fetch: (_org: string, project: Project): Promise<Release[]> => sentryApi.releases(project),
    staleAfterMs: 60_000,
    keyFn: ([org, project]) => `${org as string}|${(project as Project).id}`,
});
