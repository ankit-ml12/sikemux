import { memo, useLayoutEffect, useMemo, useState } from "react";
import { useResourceEnabled } from "../../plugin-api/resources";
import type { Job, RepoRef, Run } from "../api";
import { edgesOf, graphOf, type Graph, type GraphJob, type GraphNode } from "../jobGraph";
import { useHost } from "../registry";
import { workflowJobsR } from "../resources";
import { elapsedMs, jobsSummary, OUTCOME_LABEL, outcomeOf, type Outcome } from "../runStatus";
import { coarse } from "./hooks";
import { formatDuration } from "../../lib/format";

interface Props {
    repo: RepoRef;
    run: Run;
    jobs: Job[];
    now: number;
    active: boolean;
    openJob: string | null;
    onOpen: (jobId: string) => void;
    fileShown: boolean;
    /** Left out when the host cannot show the file a run came from. */
    onToggleFile: (() => void) | null;
}

interface Shared {
    now: number;
    openJob: string | null;
    onOpen: (jobId: string) => void;
}

type Mark = Outcome | "idle";

function fileName(path: string | null): string | null {
    if (!path) return null;
    return path.split("/").pop() ?? path;
}

function JobMark({ mark }: { mark: Mark }) {
    const label = mark === "idle" ? "Not started" : OUTCOME_LABEL[mark];
    const pending = mark === "running" || mark === "queued" || mark === "blocked";
    const filled = mark === "success" || mark === "failure" || mark === "cancelled";
    return (
        <span className="gha-graph-mark" data-outcome={mark} role="img" aria-label={label} title={label}>
            {pending ? (
                <span className="pr-ci-pending" data-outcome={mark} />
            ) : (
                <svg
                    width={14}
                    height={14}
                    viewBox="0 0 16 16"
                    fill="none"
                    strokeWidth={1.6}
                    strokeLinecap="round"
                    strokeLinejoin="round"
                    aria-hidden="true">
                    {filled ? (
                        <circle cx="8" cy="8" r="7.2" fill="currentColor" />
                    ) : (
                        <circle cx="8" cy="8" r="6.4" stroke="currentColor" strokeWidth={1.3} />
                    )}
                    {mark === "success" && <path d="m5.2 8.2 1.9 1.9 3.7-3.9" className="gha-graph-mark-inner" />}
                    {mark === "failure" && <path d="m5.8 5.8 4.4 4.4m0-4.4-4.4 4.4" className="gha-graph-mark-inner" />}
                    {mark === "cancelled" && <path d="M5.6 10.4 10.4 5.6" className="gha-graph-mark-inner" />}
                    {mark === "skipped" && <path d="M5.5 10.5 10.5 5.5" stroke="currentColor" strokeWidth={1.3} />}
                </svg>
            )}
        </span>
    );
}

function JobBox({ entry, node, shared }: { entry: GraphJob; node: GraphNode; shared: Shared }) {
    const { job, label } = entry;
    if (!job)
        return (
            <div className="gha-graph-job" data-idle="1" data-node={node.title ? undefined : node.id} title={label}>
                <JobMark mark={node.skipped ? "skipped" : "idle"} />
                <span className="gha-graph-job-name">{label}</span>
            </div>
        );
    const outcome = outcomeOf(job);
    const took = elapsedMs(job.startedAt, job.completedAt, job.completedAt ? coarse(shared.now) : shared.now);
    return (
        <button
            type="button"
            className="gha-graph-job"
            data-node={node.title ? undefined : node.id}
            data-on={shared.openJob === job.id ? "1" : "0"}
            title={job.name}
            onClick={() => shared.onOpen(job.id)}>
            <JobMark mark={outcome} />
            <span className="gha-graph-job-name">{label}</span>
            {took !== null && outcome !== "skipped" && <span className="gha-graph-job-took">{formatDuration(took)}</span>}
        </button>
    );
}

function NodeBox({ node, shared }: { node: GraphNode; shared: Shared }) {
    if (!node.title) return node.jobs[0] ? <JobBox entry={node.jobs[0]} node={node} shared={shared} /> : null;
    return (
        <div className="gha-graph-group" data-node={node.id} data-called={node.inner ? "1" : "0"}>
            <div className="gha-graph-group-title" title={node.title}>
                {node.title}
            </div>
            {node.inner ? (
                <Columns graph={node.inner} shared={shared} />
            ) : (
                node.jobs.map((entry) => <JobBox key={entry.job?.id ?? entry.label} entry={entry} node={node} shared={shared} />)
            )}
        </div>
    );
}

function Columns({ graph, shared }: { graph: Graph; shared: Shared }) {
    return (
        <div className="gha-graph-columns">
            {graph.map((column) => (
                <div className="gha-graph-stage" key={column.map((node) => node.id).join("|")}>
                    {column.map((node) => (
                        <NodeBox key={node.id} node={node} shared={shared} />
                    ))}
                </div>
            ))}
        </div>
    );
}

interface Drawn {
    width: number;
    height: number;
    lines: string[];
    dots: [number, number][];
}

function offsetWithin(element: HTMLElement, root: HTMLElement): { x: number; y: number } {
    let x = 0;
    let y = 0;
    let at: HTMLElement | null = element;
    while (at && at !== root) {
        x += at.offsetLeft;
        y += at.offsetTop;
        at = at.offsetParent as HTMLElement | null;
    }
    return { x, y };
}

/** Lines meet a box at the middle of its first row, so a tall box joins where its name is. */
function measure(canvas: HTMLElement, edges: readonly [string, string][]): Drawn {
    const boxes = new Map<string, HTMLElement>();
    for (const box of canvas.querySelectorAll<HTMLElement>("[data-node]")) if (box.dataset.node) boxes.set(box.dataset.node, box);
    const lines: string[] = [];
    const dots = new Map<string, [number, number]>();
    for (const [from, to] of edges) {
        const start = boxes.get(from);
        const end = boxes.get(to);
        if (!start || !end) continue;
        const a = offsetWithin(start, canvas);
        const b = offsetWithin(end, canvas);
        const x1 = a.x + start.offsetWidth;
        const y1 = a.y + Math.min(start.offsetHeight / 2, 14);
        const x2 = b.x;
        const y2 = b.y + Math.min(end.offsetHeight / 2, 14);
        const bend = Math.max(12, (x2 - x1) / 2);
        lines.push(`M${x1} ${y1}C${x1 + bend} ${y1} ${x2 - bend} ${y2} ${x2} ${y2}`);
        dots.set(`${x1},${y1}`, [x1, y1]);
        dots.set(`${x2},${y2}`, [x2, y2]);
    }
    return { width: canvas.offsetWidth, height: canvas.offsetHeight, lines, dots: [...dots.values()] };
}

function Edges({ canvas, edges }: { canvas: HTMLDivElement | null; edges: readonly [string, string][] }) {
    const [drawn, setDrawn] = useState<Drawn | null>(null);
    useLayoutEffect(() => {
        const root = canvas;
        if (!root) return;
        let last = "";
        const draw = () => {
            const next = measure(root, edges);
            const key = `${next.width}x${next.height}:${next.lines.join("")}`;
            if (key === last) return;
            last = key;
            setDrawn(next);
        };
        draw();
        const observer = new ResizeObserver(draw);
        observer.observe(root);
        for (const box of root.querySelectorAll("[data-node]")) observer.observe(box);
        return () => observer.disconnect();
    }, [canvas, edges]);
    if (!drawn) return null;
    return (
        <svg className="gha-graph-edges" width={drawn.width} height={drawn.height} aria-hidden="true">
            {drawn.lines.map((line) => (
                <path key={line} d={line} />
            ))}
            {drawn.dots.map(([x, y]) => (
                <circle key={`${x},${y}`} cx={x} cy={y} r={2.5} />
            ))}
        </svg>
    );
}

export const JobGraph = memo(function JobGraph({ repo, run, jobs, now, active, openJob, onOpen, fileShown, onToggleFile }: Props) {
    const host = useHost();
    const readable = !!host.api.workflowFileAt && !!run.path && run.path.startsWith(".github/");
    const files = useResourceEnabled(active && readable, workflowJobsR, repo, run.path ?? "", run.sha);
    const settled = !readable || !!files.data || !!files.error;
    const graph = useMemo(() => graphOf(jobs, files.data ?? null, run.path), [jobs, files.data, run.path]);
    const edges = useMemo(() => edgesOf(graph), [graph]);
    const [canvas, setCanvas] = useState<HTMLDivElement | null>(null);
    const shared = useMemo(() => ({ now, openJob, onOpen }), [now, openJob, onOpen]);
    const name = fileName(run.path);
    const summary = jobsSummary(jobs);

    return (
        <section className="gha-graph">
            <div className="gha-graph-head">
                <span className="gha-graph-file">{name ?? run.name}</span>
                {summary.total > 0 && (
                    <span className="gha-dim">
                        {summary.done} of {summary.total} job{summary.total === 1 ? "" : "s"} done
                        {summary.failed > 0 && <span className="gha-failed-count"> · {summary.failed} failed</span>}
                    </span>
                )}
                <span className="gha-graph-spacer" />
                {onToggleFile && run.path && !run.path.startsWith("dynamic/") && (
                    <button type="button" className="gha-link" onClick={onToggleFile}>
                        {fileShown ? "Hide workflow file" : "Workflow file"}
                    </button>
                )}
            </div>
            {!settled ? (
                <div className="gha-graph-stages" />
            ) : graph.length === 0 ? (
                <div className="gha-dim gha-graph-empty">No jobs yet</div>
            ) : (
                <div className="gha-graph-stages">
                    <div className="gha-graph-canvas" ref={setCanvas}>
                        <Edges canvas={canvas} edges={edges} />
                        <Columns graph={graph} shared={shared} />
                    </div>
                </div>
            )}
        </section>
    );
});
