import { coreCall } from "../api/core";
import { onCoreChanged } from "../api/coreChanged";
import type { PluginFailure } from "../api/plugins";
import { isSignedOutFailure } from "../plugin-api/backend";
import { resourceKind } from "../plugin-api/identity";
import { invalidate, resource } from "../state/resources";
import type { IssueList, PullList, ReleaseList, RunList, TrackedItem } from "./types";

type Answer<T> = { value: T } | { failed: PluginFailure & { plugin: string } };

/** One of the core's lists of the project at `project`; a failure rejects as its plugin's would, so a sign-in gone reads as one. */
async function list<T>(method: string, params: Record<string, unknown>): Promise<T> {
    follow();
    const answer = await coreCall<Answer<T>>(method, params);
    if ("value" in answer) return answer.value;
    const { failed } = answer;
    if (isSignedOutFailure(failed)) invalidate((kind) => kind === resourceKind(failed.plugin, "status"));
    throw failed;
}

const LISTS = new Set(["host.pullList", "host.runList", "host.issueList", "host.releaseList", "host.trackedList"]);
let following = false;

/** The core says when lists moved, so they are read again then and at no other time; it answers the unchanged ones from what it keeps. */
function follow(): void {
    if (following) return;
    following = true;
    onCoreChanged("codeHost", () => invalidate((kind) => LISTS.has(kind)));
}

export interface RunFilter {
    workflowId?: string;
    branch?: string;
    status?: string;
    headSha?: string;
    page?: number;
    perPage?: number;
}

export const pullListR = resource({
    kind: "host.pullList",
    fetch: (project: string, state: string): Promise<PullList> => list("codeHostPulls", { project, state }),
});

export const runListR = resource({
    kind: "host.runList",
    fetch: (project: string, filter: RunFilter): Promise<RunList> => list("codeHostRuns", { project, ...filter }),
});

export const issueListR = resource({
    kind: "host.issueList",
    fetch: (project: string, state: string, page: number): Promise<IssueList> => list("codeHostIssues", { project, state, page }),
});

export const releaseListR = resource({
    kind: "host.releaseList",
    fetch: (project: string): Promise<ReleaseList> => list("codeHostReleases", { project }),
});

/** The open issues and pull requests a message can be about, newest first. */
export const trackedListR = resource({
    kind: "host.trackedList",
    fetch: (project: string): Promise<TrackedItem[]> => list("codeHostTracked", { project }),
});
