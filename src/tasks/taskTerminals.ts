import * as commands from "../state/commands";
import { confirmDialog } from "../state/dialog";
import { getState } from "../state/store";
import { swallow } from "../state/toast";
import { preserveFocus } from "../harness/service";

const shown = new Set<string>();
const asked = new Set<string>();
const focusing = new Set<string>();
let started = false;

/** The terminal of a run the person just started here comes to the front when it opens. */
export function focusWhenShown(executionId: string): void {
    focusing.add(executionId);
}

    const request = {
        project: run.project,
        cwd: run.cwd ?? run.project,
        terminalKey: run.terminalKey,
        label: run.label ?? run.taskId,
    };
    if (run.agentId && getState().agents[run.agentId]) commands.openDeskTerminal(run.agentId, request);
}

}

/* Runs that were going when the page loaded come back in the background;
   later ones show as they start, wherever they came from. */
function follow(runs: readonly TaskRun[], first: boolean): void {
    for (const run of runs) {
        if (run.ptyId === undefined || shown.has(run.executionId)) continue;
        if (first && !isActive(run)) continue;
        shown.add(run.executionId);
        const focus = focusing.delete(run.executionId);
        try {
        } catch (error) {
            swallow("show task terminal")(error);
        }
    }
    const known = new Set(runs.map((run) => run.executionId));
    for (const id of shown) if (!known.has(id)) shown.delete(id);
}

export function showTaskTerminals(): void {
    if (started) return;
    started = true;
    let first = true;
    taskRuns.subscribe(() => {
        follow(taskRuns.getSnapshot(), first);
        first = false;
    });
}
