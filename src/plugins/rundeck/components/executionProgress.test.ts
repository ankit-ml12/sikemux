import { describe, expect, it } from "vitest";
import type { RundeckStep, RundeckWorkflowState } from "../api";
import { executionProgress } from "./executionProgress";

function step(executionState: string): RundeckStep {
    return { id: null, stepctx: null, executionState, startTime: null, endTime: null, nodeStep: null };
}

function state(steps: RundeckStep[], stepCount = steps.length): RundeckWorkflowState {
    return { executionState: "RUNNING", steps, stepCount, completed: false };
}

describe("executionProgress", () => {
    it("counts finished workflow steps", () => {
        expect(executionProgress(state([step("SUCCEEDED"), step("RUNNING"), step("NOT_STARTED"), step("NOT_ELIGIBLE")]))).toEqual({
            completed: 2,
            total: 4,
            percent: 50,
        });
    });

    it("uses the reported step count when steps have not all arrived", () => {
        expect(executionProgress(state([step("SUCCEEDED")], 5))).toEqual({ completed: 1, total: 5, percent: 20 });
    });
});
