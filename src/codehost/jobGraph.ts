import type { Job, LogLine, Step } from "./api";

/** Jobs from one called workflow, which GitHub names `Caller / Job` and draws in one box. */
export interface JobGroup {
    key: string;
    title: string | null;
    jobs: { job: Job; label: string }[];
}

export type Stage = JobGroup[];

/** A matrix job's values are in parentheses and may hold " / " themselves, so only a split outside them counts. */
export function splitName(name: string): { group: string | null; label: string } {
    let depth = 0;
    for (let at = 0; at < name.length; at += 1) {
        const char = name[at];
        if (char === "(") depth += 1;
        else if (char === ")") depth = Math.max(0, depth - 1);
        else if (depth === 0 && at > 0 && name.startsWith(" / ", at)) return { group: name.slice(0, at), label: name.slice(at + 3) };
    }
    return { group: null, label: name };
}

function time(value: string | null): number | null {
    if (!value) return null;
    const at = Date.parse(value);
    return Number.isNaN(at) ? null : at;
}

interface Unit {
    group: JobGroup;
    start: number | null;
    end: number | null;
    order: number;
}

/**
 * GitHub does not say which job waited for which, so the stages are read off
 * the clock: a group that only started once everything before it had finished
 * was waiting on it. Jobs that have not started yet come last.
 */
export function stagesOf(jobs: readonly Job[]): Stage[] {
    const units = new Map<string, Unit>();
    jobs.forEach((job, order) => {
        const { group, label } = splitName(job.name);
        const key = group ?? `job:${job.id}`;
        const unit = units.get(key) ?? { group: { key, title: group, jobs: [] }, start: null, end: null, order };
        unit.group.jobs.push({ job, label: group ? label : job.name });
        const start = time(job.startedAt);
        const end = time(job.completedAt);
        if (start !== null) unit.start = unit.start === null ? start : Math.min(unit.start, start);
        if (end !== null && job.status === "completed") unit.end = unit.end === null ? end : Math.max(unit.end, end);
        else if (job.status !== "completed") unit.end = Number.POSITIVE_INFINITY;
        units.set(key, unit);
    });

    const started = [...units.values()].filter((unit) => unit.start !== null);
    const waiting = [...units.values()].filter((unit) => unit.start === null);
    started.sort((a, b) => (a.start ?? 0) - (b.start ?? 0) || a.order - b.order);
    waiting.sort((a, b) => a.order - b.order);

    const stages: Stage[] = [];
    let current: Stage = [];
    let currentEnd = Number.NEGATIVE_INFINITY;
    for (const unit of started) {
        if (current.length > 0 && (unit.start ?? 0) >= currentEnd) {
            stages.push(current);
            current = [];
            currentEnd = Number.NEGATIVE_INFINITY;
        }
        current.push(unit.group);
        currentEnd = Math.max(currentEnd, unit.end ?? Number.POSITIVE_INFINITY);
    }
    if (current.length > 0) stages.push(current);
    if (waiting.length > 0) stages.push(waiting.map((unit) => unit.group));
    return stages;
}

export interface GraphJob {
    /** Null for a job the file declares that the run has not made yet. */
    job: Job | null;
    label: string;
}

export interface GraphNode {
    id: string;
    /** A matrix job's name, or the name of the job that calls a workflow; null for a lone job. */
    title: string | null;
    jobs: GraphJob[];
    /** The called workflow's own jobs, laid out the same way. */
    inner: Graph | null;
    /** The ids of the nodes in the same graph that this one waits for. */
    needs: string[];
    /** The job that called this workflow was skipped, so its jobs never will run. */
    skipped: boolean;
}

/** Columns of nodes, each one after every column it waits for. */
export type Graph = GraphNode[][];

/** Each workflow file's jobs by its path, or null when it could not be read. */
export type WorkflowFiles = Record<string, WorkflowJob[] | null>;

const EXPRESSION = /\$\{\{[\s\S]*?\}\}/g;

function displayName(job: WorkflowJob): string {
    return job.name ?? job.key;
}

/** A name with its `${{ … }}` parts taken out, for a job that has not run yet. */
function plainName(name: string): string {
    const plain = name
        .replace(/\s*\(\s*\$\{\{[\s\S]*?\}\}[^)]*\)/g, "")
        .replace(EXPRESSION, "")
        .replace(/\s{2,}/g, " ")
        .trim();
    return plain || name;
}

/** GitHub fills in a name's expressions, or adds a matrix job's values after it in parentheses. */
function matcher(display: string): (name: string) => boolean {
    const parts = display.split(EXPRESSION);
    if (parts.length === 1) return (name) => name === display || (name.startsWith(`${display} (`) && name.endsWith(")"));
    const pattern = new RegExp(`^${parts.map((part) => part.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")).join("[\\s\\S]*?")}$`);
    return (name) => pattern.test(name);
}

function matrixLabel(name: string, title: string): string {
    if (name.startsWith(`${title} (`) && name.endsWith(")")) return name.slice(title.length + 2, -1);
    return name;
}

function columnsOf(nodes: readonly GraphNode[]): Graph {
    const byId = new Map(nodes.map((node) => [node.id, node]));
    const depths = new Map<string, number>();
    const depthOf = (node: GraphNode, seen: Set<string>): number => {
        const known = depths.get(node.id);
        if (known !== undefined) return known;
        if (seen.has(node.id)) return 0;
        seen.add(node.id);
        let depth = 0;
        for (const need of node.needs) {
            const before = byId.get(need);
            if (before) depth = Math.max(depth, depthOf(before, seen) + 1);
        }
        depths.set(node.id, depth);
        return depth;
    };
    const columns: Graph = [];
    for (const node of nodes) (columns[depthOf(node, new Set())] ??= []).push(node);
    return columns.filter(Boolean);
}

function byClock(named: readonly Named[], prefix: string): Graph {
    let before: string[] = [];
    return stagesOfNamed(named).map((stage) => {
        before = column.map((node) => node.id);
        return column;
    });
}

function fromFile(named: readonly Named[], declared: readonly WorkflowJob[], files: WorkflowFiles, prefix: string, skipped: boolean): Graph | null {
    const matches = declared.map((each) => matcher(displayName(each)));
    const own = declared.map((): Named[] => []);
    const called = declared.map((): Named[] => []);
    for (const entry of named) {
        const { group, label } = splitName(entry.name);
        const caller = group === null ? -1 : declared.findIndex((each, index) => each.uses !== null && matches[index]?.(group));
        if (caller >= 0) {
            called[caller]?.push({ job: entry.job, name: label });
            continue;
        }
        const exact = declared.findIndex((each) => displayName(each) === entry.name);
        const at = exact >= 0 ? exact : matches.findIndex((test) => test(entry.name));
        if (at < 0) return null;
        own[at]?.push(entry);
    }

    const keys = new Set(declared.map((each) => each.key));
    const nodes = declared.map((each, index): GraphNode => {
        const id = prefix + each.key;
        const records = own[index] ?? [];
        const calls = called[index] ?? [];
        const needs = each.needs.filter((need) => keys.has(need)).map((need) => prefix + need);
        const title = plainName(displayName(each));
        if (each.uses !== null) {
            const callerSkipped = skipped || records.some((record) => outcomeOf(record.job) === "skipped");
            const path = calledPath(each.uses);
            const file = path ? files[path] : null;
            const inner = (file && fromFile(calls, file, files, `${id}/`, callerSkipped)) || (calls.length > 0 ? byClock(calls, `${id}/`) : null);
            if (inner) return { id, title, jobs: [], inner, needs, skipped: callerSkipped };
        }
        if (records.length > 1) {
            const jobs = records.map((record) => ({ job: record.job, label: matrixLabel(record.name, title) }));
            return { id, title, jobs, inner: null, needs, skipped };
        }
        const record = records[0];
        const jobs = [record ? { job: record.job, label: record.name } : { job: null, label: title }];
        return { id, title: null, jobs, inner: null, needs, skipped };
    });
    return columnsOf(nodes);
}

 * The run's jobs laid out as GitHub draws them, by what each one `needs` in the
 * workflow file at `path`, with a called workflow's jobs inside the job that
 * called it. Without the file, or with a job it does not declare, it falls back
 * to reading the stages off the clock.
 */
export function graphOf(jobs: readonly Job[], files: WorkflowFiles | null, path: string | null): Graph {
    const named = jobs.map((job) => ({ job, name: job.name }));
    const declared = files && path ? files[path] : null;
    return (declared && files && fromFile(named, declared, files, "", false)) || byClock(named, "");
}

/** Every line to draw, from a node to each node that waits for it, inside called workflows too. */
export function edgesOf(graph: Graph): [string, string][] {
    const edges: [string, string][] = [];
    for (const column of graph) {
        for (const node of column) {
            for (const need of node.needs) edges.push([need, node.id]);
            if (node.inner) edges.push(...edgesOf(node.inner));
        }
    }
    return edges;
}

/**
 * Where each step's part of the log begins. A job's log is one stream, and the
 * only thing tying a line to a step is when it was written.
 */
export function stepStarts(lines: readonly LogLine[], steps: readonly Step[]): Map<number, number> {
    const starts = new Map<number, number>();
    const stamps = lines.map((line) => time(line.timestamp));
    let from = 0;
    for (const step of steps) {
        const begins = time(step.startedAt);
        if (begins === null) continue;
        // A step's time is kept to the second and a line's to a fraction of one.
        const floor = Math.floor(begins / 1000) * 1000;
        let index = from;
        while (index < stamps.length && (stamps[index] ?? Number.NEGATIVE_INFINITY) < floor) index += 1;
        if (index >= stamps.length) break;
        starts.set(step.number, index);
        from = index;
    }
    return starts;
}
