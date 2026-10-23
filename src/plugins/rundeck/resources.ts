import { peekResource, useResource } from "../../plugin-api/resources";
import { defineService } from "../../plugin-api/service";
import { rundeckApi, type PlanRequest, type RundeckJob } from "./api";
import { RUNDECK_PLUGIN } from "./kinds";
import type { RundeckActivityPage, RundeckJobPage, RundeckJobs, RundeckTree } from "./types.generated";
import { DEFAULT_BRANCH_OPTIONS, DEFAULT_PROD_ENVS } from "./shape";

export const rundeck = defineService(RUNDECK_PLUGIN, rundeckApi.status);

export const rndProjectsR = rundeck.resource("projects", () => rundeckApi.projects(), 5 * 60_000);

export const rndJobsR = rundeck.resource("jobs", (project: string) => rundeckApi.jobs(project), 60_000);

export const rndJobIndexR = rundeck.resource("jobIndex", () => rundeckApi.jobIndex(), 5 * 60_000);

export const rndJobDetailR = rundeck.resource("jobDetail", (jobId: string) => rundeckApi.jobDetail(jobId), 60_000);

export const rndOptionValuesR = rundeck.resource("optionValues", (url: string) => rundeckApi.optionValues(url), 60_000);

/** The projects' folders of jobs, kept by the core for the pane's tree. */
export const rndTreeV = rundeck.view<[], RundeckTree>("treeView", () => ({}));

/** A folder's jobs with how each last ran, kept by the core; `group` empty for the project's root. */
export const rndJobsV = rundeck.view<[project: string, group: string], RundeckJobs>("jobsView", (project, group) => ({
    project,
    group: group || undefined,
}));

/** A job with its recent runs, kept by the core and read every few seconds while one goes on. */
export const rndJobV = rundeck.view<[jobId: string, project: string], RundeckJobPage>("jobView", (jobId, project) => ({ jobId, project }));

/** A project's newest `max` runs of every job, kept by the core. */
export const rndActivityV = rundeck.view<[project: string, max: number], RundeckActivityPage>("activityView", (project, max) => ({
    project,
    max,
}));

export const rndJobCellsR = rundeck.resource("jobCells", (jobs: RundeckJob[]) => rundeckApi.jobCells(jobs), 30_000);

export const rndExecutionsR = rundeck.resource(
    "executions",
    (jobId: string, project: string, max: number) => rundeckApi.executions(jobId, project, max),
    15_000,
);

export const rndPlanR = rundeck.resource("plan", (request: PlanRequest) => rundeckApi.plan(request), 10_000);

export const rndListsR = rundeck.resource("lists", () => rundeckApi.lists());

/** Job options that hold the git branch, as the plugin keeps them. */
export function useBranchOptions(): string[] {
    return useResource(rndListsR).data?.branchOptions ?? DEFAULT_BRANCH_OPTIONS;
}

/** Words that mark a job's top folder or project as production, as the plugin keeps them. */
export function useProdEnvs(): string[] {
    return useResource(rndListsR).data?.prodEnvs ?? DEFAULT_PROD_ENVS;
}

export const branchOptionsNow = (): string[] => peekResource(rndListsR)?.branchOptions ?? DEFAULT_BRANCH_OPTIONS;
