import { useEffect, useState, useSyncExternalStore } from "react";
import { coreCall } from "../api/core";
import { onCoreChanged } from "../api/coreChanged";

export type TaskSourceKind = "project" | "personal" | "detected" | "rerun" | "command";

/** A task the project offers, as the core lists it. */
export interface ProjectTask {
    readonly id: string;
    readonly label: string;
    readonly command: string;
    /** Relative to the project, `.` for its root. */
    readonly cwd: string;
    readonly source: TaskSourceKind;
    readonly java?: { readonly required: number; readonly runsOn: number | null; readonly current: number | null };
}

export interface ProjectTasks {
    readonly project: string;
    readonly configStatus: "absent" | "invalid" | "valid";
    readonly trusted: boolean;
    readonly tasks: readonly ProjectTask[];
}

/** What the core asks the person before it runs commands someone else wrote. */
export interface TrustAsk {
    readonly title: string;
    readonly body: string;
    readonly commands: { label: string; command: string }[];
    readonly confirmLabel: string;
}

export type RunStatus = "awaiting-trust" | "starting" | "running" | "completed" | "failed" | "stopping" | "stopped";

/** One run of a task, whoever started it. */
export interface TaskRun {
    readonly executionId: string;
    readonly taskId: string;
    readonly project: string;
    readonly status: RunStatus;
    readonly label?: string;
    readonly ptyId?: number;
    readonly exitCode?: number;
    readonly signal?: string;
    readonly error?: string;
    readonly previewUrl?: string;
    readonly cwd?: string;
    readonly startedAt?: number;
    /** The agent that started it; none when the person did. */
    readonly agentId: string | null;
    /** Shared by every run of the task, so a rerun shows in the same terminal. */
    readonly terminalKey: string;
    readonly tests?: TestResults;
}

export const ACTIVE_STATUSES: readonly RunStatus[] = ["awaiting-trust", "starting", "running", "stopping"];

export function isActive(run: TaskRun): boolean {
    return ACTIVE_STATUSES.includes(run.status);
}

export const tasksApi = {
    list: (project: string) => coreCall<ProjectTasks>("tasks", { project }),
    /** Answers the checks that stopped the task, or the run it started. */
    start: (project: string, taskId: string, how: { force?: boolean; fix?: boolean } = {}) =>
        coreCall<{ executionId: string } | { checks: Check[] }>("taskStart", { project, taskId, ...how }),
    stop: (executionId: string) => coreCall<void>("taskStop", { executionId }),
    rerunFailed: (executionId: string) => coreCall<{ taskId: string; executionId: string }>("taskRerunFailed", { executionId }),
    add: (project: string, task: { label: string; command: string; cwd: string; team: boolean }) =>
        coreCall<{ taskId: string }>("addTask", { project, ...task }),
};

let runs: readonly TaskRun[] = [];
let watching = false;
const listeners = new Set<() => void>();

function readRuns(): void {
    void coreCall<TaskRun[]>("taskRuns")
        .catch(() => {});
}

function subscribeRuns(listener: () => void): () => void {
    listeners.add(listener);
    if (!watching) {
        watching = true;
        onCoreChanged("runs", readRuns);
        readRuns();
    }
    return () => listeners.delete(listener);
}

/** Every run the core knows, as the core last said. */
export const taskRuns = { subscribe: subscribeRuns, getSnapshot: (): readonly TaskRun[] => runs };

export function useTaskRuns(): readonly TaskRun[] {
    return useSyncExternalStore(taskRuns.subscribe, taskRuns.getSnapshot, taskRuns.getSnapshot);
}

/** The newest run with a process behind `terminalKey`, which its terminal shows. */
export function runShownIn(all: readonly TaskRun[], terminalKey: string): TaskRun | null {
    for (let index = all.length - 1; index >= 0; index -= 1) {
        const run = all[index];
        if (run.terminalKey === terminalKey && run.ptyId !== undefined) return run;
    }
    return null;
}

/** What `project` can run, read again whenever the core says it changed. */
export function useProjectTasks(project: string | null, refreshKey: unknown = null): ProjectTasks | null {
    const [found, setFound] = useState<ProjectTasks | null>(null);
    const [version, setVersion] = useState(0);
    useEffect(() => {
        if (!project) return;
        return onCoreChanged("tasks", (change) => {
            if (change.project === null || change.project === project) setVersion((current) => current + 1);
        });
    }, [project]);
    useEffect(() => {
        if (!project) return;
        let cancelled = false;
        void tasksApi
            .list(project)
            .then((next) => {
                if (!cancelled) setFound(next);
            })
            .catch(() => {});
        return () => {
            cancelled = true;
        };
    }, [project, version, refreshKey]);
    return found?.project === project ? found : null;
}
