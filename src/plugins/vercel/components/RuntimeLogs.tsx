import { useEffect, useMemo, useState } from "react";
import { copyWithNotice } from "../../../plugin-api/host";
import { Seg } from "../../../plugin-api/ui";
import { errorText } from "../../../plugin-api/format";
import { vercelApi, type RuntimeLogLine, type VercelDeployment } from "../api";

/** The most lines kept while listening; older ones fall off the top. */
const MAX_LINES = 2000;

export type LevelFilter = "all" | "warning" | "error";
export type StatusFilter = "all" | "4xx" | "5xx";

export interface RuntimeFilter {
    level: LevelFilter;
    status: StatusFilter;
    source: string;
    path: string;
}

export function keeps(line: RuntimeLogLine, filter: RuntimeFilter): boolean {
    if (filter.level === "error" && line.level !== "error") return false;
    if (filter.level === "warning" && line.level !== "error" && line.level !== "warning") return false;
    if (filter.status === "5xx" && !(line.status !== null && line.status >= 500)) return false;
    if (filter.status === "4xx" && !(line.status !== null && line.status >= 400 && line.status < 500)) return false;
    if (filter.source && line.source !== filter.source) return false;
    const words = filter.path.trim().toLowerCase();
    if (words && !`${line.path ?? ""} ${line.message}`.toLowerCase().includes(words)) return false;
    return true;
}

const LEVELS = [
    { value: "all" as const, label: "All" },
    { value: "warning" as const, label: "Warnings" },
    { value: "error" as const, label: "Errors" },
];

const STATUSES = [
    { value: "all" as const, label: "Any status" },
    { value: "4xx" as const, label: "4xx" },
    { value: "5xx" as const, label: "5xx" },
];

/**
 * What the deployment's functions, middleware and edge write while answering
 * requests, as it happens. Vercel keeps none from before, so nothing shows until
 * requests come in while listening.
 */
export function RuntimeLogs({ team, deployment }: { team: string; deployment: VercelDeployment }) {
    const [listening, setListening] = useState(false);
    const [lines, setLines] = useState<RuntimeLogLine[]>([]);
    const [error, setError] = useState<string | null>(null);
    const [filter, setFilter] = useState<RuntimeFilter>({ level: "all", status: "all", source: "", path: "" });
    const project = deployment.projectId ?? deployment.project;

    useEffect(() => {
        if (!listening) return;
        setError(null);
        const stream = vercelApi.followRuntimeLogs(team, project, deployment.id, {
            onItem: (batch) => setLines((before) => before.concat(batch.lines).slice(-MAX_LINES)),
            onEnd: () => setListening(false),
            onError: (failure) => {
                setError(errorText(failure));
                setListening(false);
            },
        });
        return () => stream.stop();
    }, [listening, team, project, deployment.id]);

    const sources = useMemo(() => [...new Set(lines.map((line) => line.source).filter(Boolean))].sort(), [lines]);
    const shown = lines.filter((line) => keeps(line, filter));

    return (
        <section className="vercel-box vercel-logs vercel-runtime" aria-label="Runtime Logs">
            <header className="vercel-box-head">
                <span className="vercel-box-title">Runtime Logs</span>
                <span className="vercel-actions">
                    <span className="vercel-muted" role="status">
                        {listening
                            ? lines.length === 0
                                ? "Listening for requests…"
                                : `${shown.length} of ${lines.length}`
                            : lines.length > 0
                              ? `${lines.length} lines`
                              : ""}
                    </span>
                    <button
                        type="button"
                        className="vercel-button"
                        disabled={shown.length === 0}
                        onClick={() =>
                            copyWithNotice(
                                shown
                                    .map(
                                        (line) =>
                                            `${new Date(line.at).toISOString()} ${line.level} ${line.status ?? ""} ${line.path ?? ""} ${line.message}`,
                                    )
                                    .join("\n"),
                                "Runtime logs",
                            )
                        }>
                        Copy
                    </button>
                    <button type="button" className={`vercel-button${listening ? "" : " primary"}`} onClick={() => setListening((was) => !was)}>
                        {listening ? "Stop" : "Listen"}
                    </button>
                </span>
            </header>
            <div className="vercel-runtime-filters">
                <Seg label="Level" options={LEVELS} value={filter.level} onChange={(level) => setFilter({ ...filter, level })} />
                <Seg label="Status" options={STATUSES} value={filter.status} onChange={(status) => setFilter({ ...filter, status })} />
                {sources.length > 1 ? (
                    <select
                        className="vercel-find"
                        aria-label="Source"
                        value={filter.source}
                        onChange={(event) => setFilter({ ...filter, source: event.target.value })}>
                        <option value="">Every source</option>
                        {sources.map((source) => (
                            <option key={source} value={source}>
                                {source}
                            </option>
                        ))}
                    </select>
                ) : null}
                <input
                    className="vercel-find"
                    aria-label="Path or words"
                    placeholder="Path or words…"
                    value={filter.path}
                    onChange={(event) => setFilter({ ...filter, path: event.target.value })}
                    spellCheck={false}
                />
            </div>
            {error ? <p className="vercel-note">{error}</p> : null}
            <div className="vercel-log" role="log" aria-label="Runtime log">
                {shown.length === 0 ? (
                    <p className="vercel-muted vercel-runtime-empty">
                        {listening
                            ? "Open the site or call the API; each request's logs show here as it happens."
                            : lines.length > 0
                              ? "No lines match."
                              : "Vercel keeps no runtime logs from before you listen. Press Listen, then make the request."}
                    </p>
                ) : (
                    shown.map((line, index) => (
                        <div key={`${line.at}:${index}`} className={`vercel-log-line runtime ${line.level}`}>
                            <span className="vercel-log-time">{new Date(line.at).toLocaleTimeString(undefined, { hour12: false })}</span>
                            {line.status !== null ? (
                                <span className={`vercel-log-status s${Math.floor(line.status / 100)}`}>{line.status}</span>
                            ) : null}
                            {line.path ? (
                                <span className="vercel-log-path">
                                    {line.method ? `${line.method} ` : ""}
                                    {line.path}
                                </span>
                            ) : null}
                            <span className="vercel-log-text">{line.message}</span>
                        </div>
                    ))
                )}
            </div>
        </section>
    );
}
