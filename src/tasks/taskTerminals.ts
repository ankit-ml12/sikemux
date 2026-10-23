import * as commands from "../state/commands";
import { confirmDialog } from "../state/dialog";
import { getState } from "../state/store";
import { swallow } from "../state/toast";
import { preserveFocus } from "../harness/service";
import { onCoreChanged } from "../api/coreChanged";
import { isActive, taskRuns, tasksApi, type PendingTrust, type TaskRun } from "./coreTasks";

const shown = new Set<string>();
const asked = new Set<string>();
const focusing = new Set<string>();
let started = false;

/** The terminal of a run the person just started here comes to the front when it opens. */
export function focusWhenShown(executionId: string): void {
    focusing.add(executionId);
}

/* An agent's task goes on that agent's desk. One the person started, or whose
   agent has since closed, gets a tab in the workspace, which takes focus only
   when the person started it here. */
function show(run: TaskRun, adopted: boolean, focus: boolean): void {
    const request = {
        project: run.project,
        cwd: run.cwd ?? run.project,
        terminalKey: run.terminalKey,
        label: run.label ?? run.taskId,
        background: adopted,
    };
    if (run.agentId && getState().agents[run.agentId]) commands.openDeskTerminal(run.agentId, request);
    else if (focus) commands.openTaskTerminal(request);
    else preserveFocus(() => commands.openTaskTerminal({ ...request, background: true }));
}

function ask(question: PendingTrust): void {
    if (asked.has(question.id)) return;
    asked.add(question.id);
    void confirmDialog(question)
        .then((trust) => tasksApi.answerTrust(question.id, trust))
        .catch(swallow("answer trust"));
}

function readAsks(): void {
    void tasksApi
        .trustAsks()
        .then((questions) => {
            for (const question of questions) ask(question);
            const waiting = new Set(questions.map((question) => question.id));
            for (const id of asked) if (!waiting.has(id)) asked.delete(id);
        })
        .catch(swallow("read trust questions"));
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
            show(run, first, focus);
        } catch (error) {
            swallow("show task terminal")(error);
        }
    }
    const known = new Set(runs.map((run) => run.executionId));
    for (const id of shown) if (!known.has(id)) shown.delete(id);
}

/** Shows every task's terminal from the core's runs, and puts the core's trust questions to the person, from now on. */
export function showTaskTerminals(): void {
    if (started) return;
    started = true;
    onCoreChanged("trust", readAsks);
    readAsks();
    let first = true;
    taskRuns.subscribe(() => {
        follow(taskRuns.getSnapshot(), first);
        first = false;
    });
}
