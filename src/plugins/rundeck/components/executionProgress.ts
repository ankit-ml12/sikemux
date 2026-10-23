import type { RundeckWorkflowState } from "../api";

const FINISHED_STATES = new Set(["SUCCEEDED", "FAILED", "ABORTED", "NOT_ELIGIBLE", "SKIPPED"]);

export interface ExecutionProgress {
    completed: number;
    total: number;
    percent: number;
}

export function executionProgress(state: RundeckWorkflowState | null | undefined): ExecutionProgress | null {
    if (!state) return null;
    const total = Math.max(state.stepCount ?? 0, state.steps.length);
    if (total === 0) return null;
    const completed = Math.min(total, state.steps.filter((step) => FINISHED_STATES.has((step.executionState ?? "").toUpperCase())).length);
    return { completed, total, percent: Math.round((completed / total) * 100) };
}
