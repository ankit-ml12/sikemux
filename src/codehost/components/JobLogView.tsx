import { memo, useCallback, useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";
import { useVirtualizer } from "@tanstack/react-virtual";
import { useResourceEnabled } from "../../plugin-api/resources";
import { copyText, notify, swallow } from "../../plugin-api/host";
import { EmptyState, IconCollapseAll, IconExpandAll, SkeletonRows } from "../../plugin-api/ui";
import { CARD_HEADER_HEIGHT, CardStream, type CardStreamHandle } from "../../ui/CardStream";
import { failureMessage, type Job, type LogLine, type RepoRef, type Step } from "../api";
import { stepStarts } from "../jobGraph";
import { jobLogR } from "../resources";
import { elapsedMs, outcomeOf } from "../runStatus";
import { useEvery } from "../../plugin-api/hooks";
import { formatDuration } from "../../lib/format";
import { RunMark } from "./ActionsIcon";

/** Runner logs mark their sections with `##[...]`, which is noise on screen. */
const MARKUP = /^##\[(?:group|endgroup|section|command)\]/u;
const ENDGROUP = /^##\[endgroup\]/u;

function clean(text: string): string {
    return text.replace(MARKUP, "");
}

const LIVE_REFRESH_MS = 5_000;
const LINE_HEIGHT = 18;
/** How close to the bottom the reader must be for a running job's new lines to keep the view at the bottom. */
const FOLLOW_SLACK = 48;

function findMatches(lines: readonly LogLine[], needle: string): number[] {
    if (!needle) return [];
    const found: number[] = [];
    lines.forEach((line, index) => {
        if (line.text.toLowerCase().includes(needle)) found.push(index);
    });
    return found;
}

const needleOf = (query: string) => query.trim().toLowerCase();

/** One step's stretch of the job's log: lines `from` up to `to`. */
interface Section {
    key: string;
    step: Step | null;
    from: number;
    to: number;
}

/** Lines written before the first step began belong to it; a step that never ran holds no lines. */
function sectionsOf(lines: readonly LogLine[], steps: readonly Step[], starts: ReadonlyMap<number, number>): Section[] {
    if (steps.length === 0) return [{ key: "log", step: null, from: 0, to: lines.length }];
    let cursor = 0;
    let first = true;
    return steps.map((step, index) => {
        const key = `step:${step.number}`;
        const begins = starts.get(step.number);
        if (begins === undefined) return { key, step, from: cursor, to: cursor };
        const next = steps.slice(index + 1).find((later) => starts.has(later.number));
        const to = next ? (starts.get(next.number) ?? lines.length) : lines.length;
        const from = first ? 0 : begins;
        first = false;
        cursor = to;
        return { key, step, from, to };
    });
}

interface Jump {
    key: string;
    line: number;
}

interface Marks {
    matched: ReadonlySet<number>;
    current: number;
}

interface StepLinesProps {
    lines: readonly LogLine[];
    section: Section;
    start: number;
    scroller: () => HTMLElement | null;
    marks: Marks;
    jump: Jump | null;
    onJumped: () => void;
}

/** An open step's lines, drawn only where the list is scrolled to, however long the step ran. */
const StepLines = memo(function StepLines({ lines, section, start, scroller, marks, jump, onJumped }: StepLinesProps) {
    const rows = useRef<HTMLDivElement>(null);
    const [margin, setMargin] = useState(start + CARD_HEADER_HEIGHT + BODY_PADDING);
    useLayoutEffect(() => {
        if (rows.current) setMargin(start + rows.current.offsetTop);
    }, [start]);
    const { from, to } = section;
    const virtualizer = useVirtualizer({
        count: to - from,
        getScrollElement: scroller,
        estimateSize: () => LINE_HEIGHT,
        overscan: 24,
        scrollMargin: margin,
        getItemKey: (index) => lines[from + index]?.number ?? index,
        initialRect: { width: 1000, height: 800 },
    });

    const mine = jump?.key === section.key ? jump : null;
    useEffect(() => {
        if (!mine) return;
        const frame = window.requestAnimationFrame(() => {
            onJumped();
        });
        return () => window.cancelAnimationFrame(frame);

    if (to === from) return <div className="gha-step-log gha-dim gha-step-empty">This step wrote nothing.</div>;
    return (
        <div className="gha-step-log">
            <div ref={rows} className="gha-step-rows" style={{ height: virtualizer.getTotalSize() }}>
                {virtualizer.getVirtualItems().map((row) => {
                    const index = from + row.index;
                    const line = lines[index];
                    if (!line) return null;
                    const group = MARKUP.test(line.text) && !ENDGROUP.test(line.text) ? " group" : "";
                    const hit = index === marks.current ? " hit current" : marks.matched.has(index) ? " hit" : "";
                    return (
                        <div
                            key={row.key}
                            ref={virtualizer.measureElement}
                            data-index={row.index}
                            className={`gha-log-line${group}${hit}`}
                            style={{ transform: `translateY(${row.start - virtualizer.options.scrollMargin}px)` }}>
                            <span className="gha-log-number gha-mono">{row.index + 1}</span>
                            <span className="gha-log-text gha-mono">{clean(line.text) || " "}</span>
                        </div>
                    );
                })}
            </div>
        </div>
    );
});

interface Props {
    repo: RepoRef;
    job: Job;
    now: number;
    active: boolean;
}

export function JobLogView({ repo, job, now, active }: Props) {
    const running = job.status !== "completed";
    const log = useResourceEnabled(active, jobLogR, repo, job.id);
    const [query, setQuery] = useState("");
    const [match, setMatch] = useState(0);
    const [jump, setJump] = useState<Jump | null>(null);
    const [opened, setOpened] = useState<Set<string>>(() => new Set());
    const stream = useRef<CardStreamHandle>(null);
    const following = useRef(true);

    useEvery(active && running, LIVE_REFRESH_MS, () => void log.refresh());

    // The last lines a job writes, usually the error, land after the last
    // read made while it was running, so a finished job is read once more.
    const refreshLog = useRef(log.refresh);
    refreshLog.current = log.refresh;
    const owesFinalRead = useRef(running);
    useEffect(() => {
        if (running) {
            owesFinalRead.current = true;
            return;
        }
        if (!owesFinalRead.current || !active) return;
        owesFinalRead.current = false;
        void refreshLog.current();
    }, [running, active]);

    const lines = useMemo(() => log.data?.lines ?? [], [log.data]);
    const starts = useMemo(() => stepStarts(lines, job.steps), [lines, job.steps]);
    const sections = useMemo(() => sectionsOf(lines, job.steps, starts), [lines, job.steps, starts]);
    const keys = useMemo(() => sections.map((section) => section.key), [sections]);
    const byKey = useMemo(() => new Map(sections.map((section) => [section.key, section])), [sections]);
    const collapsed = useMemo(() => new Set(keys.filter((key) => !opened.has(key))), [keys, opened]);
    const needle = needleOf(query);
    const matches = useMemo(() => findMatches(lines, needle), [lines, needle]);
    const matched = useMemo(() => new Set(matches), [matches]);

    const shownStep = useRef<{ number: number } | null>(null);
    useEffect(() => {
        if (!running || !following.current || needle) return;
        const frame = window.requestAnimationFrame(() => {
            const list = stream.current?.scroller();
            if (list) list.scrollTop = list.scrollHeight;
        });
        return () => window.cancelAnimationFrame(frame);
    }, [running, lines.length, opened, needle]);

    const toggle = useCallback((key: string) => {
        setOpened((was) => {
            const next = new Set(was);
            next.has(key) ? next.delete(key) : next.add(key);
            return next;
        });
    }, []);
    const jumped = useCallback(() => setJump(null), []);
    const scroller = useCallback(() => stream.current?.scroller() ?? null, []);

    if (log.status === "loading" && !log.data) return <SkeletonRows rows={12} label="Loading log" />;
    if (log.error) {
        return (
            <EmptyState
                title="Could not read the log"
                message={failureMessage(log.error)}
                tone="error"
                action={{ label: "Try again", onClick: () => void log.refresh() }}
            />
        );
    }
    if (log.data?.expired) {
        return <EmptyState title="The log is gone" message="GitHub keeps job logs for a limited time, and this one has aged out." />;
    }
    if (lines.length === 0) {
        return <EmptyState message={running ? "This job has not written anything yet." : "This job wrote no log."} />;
    }

    const copyAll = () =>
        void copyText(lines.map((line) => clean(line.text)).join("\n"))
            .then(() => notify("success", `Copied ${lines.length} lines`))
            .catch(swallow("copy the log"));

    const goTo = (next: number) => {
        if (matches.length === 0) return;
        const wrapped = (next + matches.length) % matches.length;
        setMatch(wrapped);
        show(matches[wrapped] ?? 0);
    };

    return (
        <div className="gha-log">
            <div className="gha-log-head">
                <input
                    className="gha-log-search"
                    type="search"
                    placeholder="Search the log"
                    value={query}
                    spellCheck={false}
                    onChange={(event) => {
                        const first = findMatches(lines, needleOf(event.target.value))[0];
                        setQuery(event.target.value);
                        setMatch(0);
                        if (first !== undefined) show(first);
                    }}
                    onKeyDown={(event) => {
                        if (event.key !== "Enter") return;
                        event.preventDefault();
                        goTo(event.shiftKey ? match - 1 : match + 1);
                    }}
                />
                {needle && <span className="gha-dim gha-mono">{matches.length === 0 ? "no matches" : `${match + 1} of ${matches.length}`}</span>}
                {matches.length > 1 && (
                    <>
                        <button type="button" className="gha-link" onClick={() => goTo(match - 1)}>
                            Previous
                        </button>
                        <button type="button" className="gha-link" onClick={() => goTo(match + 1)}>
                            Next
                        </button>
                    </>
                )}
                <span className="gha-log-spacer" />
                <span className="gha-dim">
                    {lines.length} line{lines.length === 1 ? "" : "s"}
                    {running && " so far"}
                </span>
                <button type="button" className="merge-review-action" onClick={() => setOpened(new Set(keys))} disabled={collapsed.size === 0}>
                    <IconExpandAll size={12} />
                    Expand all
                </button>
                <button type="button" className="merge-review-action" onClick={() => setOpened(new Set())} disabled={collapsed.size === keys.length}>
                    <IconCollapseAll size={12} />
                    Collapse all
                </button>
                <button type="button" className="gha-link" onClick={copyAll}>
                    Copy
                </button>
                <button type="button" className="gha-link" onClick={() => void log.refresh()}>
                    Refresh
                </button>
            </div>
            <div className="merge-review gha-log-steps">
                <CardStream
                    handle={stream}
                    keys={keys}
                    collapsed={collapsed}
                    onToggle={toggle}
                    estimate={(key) => {
                        const section = byKey.get(key);
                        return CARD_HEADER_HEIGHT + BODY_PADDING * 2 + Math.max(1, (section?.to ?? 0) - (section?.from ?? 0)) * LINE_HEIGHT;
                    }}
                    toggleLabel={(key, open) => `${open ? "Collapse" : "Expand"} ${byKey.get(key)?.step?.name ?? "the log"}`}
                    onScroll={(list) => {
                        following.current = list.scrollHeight - list.scrollTop - list.clientHeight < FOLLOW_SLACK;
                    }}
                    header={(key) => {
                        const step = byKey.get(key)?.step ?? null;
                        if (!step) return <span className="acc-name gha-step-title">{job.name}</span>;
                        const outcome = outcomeOf(step);
                        const took = elapsedMs(step.startedAt, step.completedAt, now);
                        return (
                            <>
                                <RunMark outcome={outcome} size={14} />
                                <span className="acc-name gha-step-title" data-outcome={outcome}>
                                    {step.name}
                                </span>
                                <span className="acc-grow" />
                                {took !== null && outcome !== "skipped" && <span className="merge-file-status gha-step-took">{formatDuration(took)}</span>}
                            </>
                        );
                    }}
                    body={(key, start) => {
                        const section = byKey.get(key);
                        if (!section) return null;
                        return (
                            <StepLines
                                lines={lines}
                                section={section}
                                start={start}
                                scroller={scroller}
                                marks={marks}
                                jump={jump}
                                onJumped={jumped}
                            />
                        );
                    }}
                />
            </div>
        </div>
    );
}
