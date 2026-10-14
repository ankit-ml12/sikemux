import { useCallback, useEffect, useState, type MouseEvent, type ReactNode } from "react";
import { gitOverviewR, notify, reportError, swallow, useActiveProjectCwd } from "../../../plugin-api/host";
import { useResourceEnabled } from "../../../plugin-api/resources";
import {
    ContextMenu,
    Dropdown,
    EmptyState,
    IconChevron,
    IconClose,
    IconCopy,
    IconExternal,
    Markdown,
    SkeletonRows,
    type ContextMenuItem,
} from "../../../plugin-api/ui";
import { failureMessage, jiraApi, refreshJira, type JiraIssue, type JiraIssueSummary, type JiraPerson } from "../api";
import { jiraBoardsR, jiraFiltersR, jiraIssueR, jiraProjectsR, jiraSearchR, jiraStatusR } from "../resources";
import { copyIssue, issueMenu, openIssue } from "../issueLinks";
import { jqlOf, mentions, showJiraList, updateJiraView, useJiraView, type JiraList } from "../state";
import { statusCounts, withStatuses } from "../statusFilter";
import { JiraBoard } from "./JiraBoard";
import { JiraSignIn } from "./JiraSignIn";
import { StatusChip } from "./StatusChip";
import { PriorityMark, TypeMark } from "./JiraGlyphs";
import { dueLabel, parentColour } from "../boardFilter";
import { TaskBox, TaskToggleContext, type ToggleTask } from "./TaskBox";
import "../jira.css";

const sameList = (a: JiraList, b: JiraList) => JSON.stringify(a) === JSON.stringify(b);

const MY_WORK: [JiraList, string][] = [
    [{ kind: "mine" }, "Assigned to me"],
    [{ kind: "reported" }, "Reported by me"],
    [{ kind: "watching" }, "Watching"],
    [{ kind: "recent" }, "Recently viewed"],
    [{ kind: "sprint" }, "Current sprint"],
];

const EMPTY: Partial<Record<JiraList["kind"], string>> = {
    mine: "Nothing is assigned to you that is open or was done in the last two weeks.",
    reported: "Nothing you reported is open or was done in the last two weeks.",
    watching: "You are not watching any open issue.",
    recent: "No issue viewed lately.",
    unassigned: "Every open issue here has someone on it.",
};

export function JiraPane({ paneId, active }: { paneId: string; active: boolean }) {
    const status = useResourceEnabled(active, jiraStatusR);
    if (status.data === undefined && status.status !== "error") return <SkeletonRows rows={5} label="Connecting to Jira" />;
    if (!status.data?.configured || status.data.authFailed) return <JiraSignIn status={status.data} onSignedIn={refreshJira} />;
    return <JiraWorkspace paneId={paneId} active={active} sites={status.data.sites.map((site) => site.host)} />;
}

function JiraWorkspace({ paneId, active, sites }: { paneId: string; active: boolean; sites: string[] }) {
    const view = useJiraView(paneId);
    const [jql, setJql] = useState(view.list.kind === "jql" ? view.list.jql : "");
    const site = sites.includes(view.site) ? view.site : (sites[0] ?? "");
    const filters = useResourceEnabled(active, jiraFiltersR, site);
    const projects = useResourceEnabled(active, jiraProjectsR, site);
    const boards = useResourceEnabled(active, jiraBoardsR, site);
    const [boardFilter, setBoardFilter] = useState("");
    const listProject = view.list.kind === "project" || view.list.kind === "unassigned" ? view.list : null;
    const [picked, setPicked] = useState<{ key: string; name: string } | null>(null);
    const project = listProject ?? picked ?? projects.data?.[0] ?? null;
    const show = (list: JiraList) => showJiraList(paneId, list);
    const words = boardFilter.trim().toLowerCase();
    const shownBoards = (boards.data ?? []).filter(
        (board) => !words || board.name.toLowerCase().includes(words) || (board.project ?? "").toLowerCase().includes(words),
    );
    const listButton = (list: JiraList, label: string, title?: string) => (
        <button
            key={label}
            type="button"
            className={`jira-list${sameList(view.list, list) ? " active" : ""}`}
            title={title}
            onClick={() => show(list)}>
            {label}
        </button>
    );

    return (
        <div className={`jira-pane${view.list.kind === "board" ? " board" : ""}`}>
            <nav className="jira-sidebar" aria-label="Jira lists">
                {sites.length > 1 && (
                    <Dropdown
                        label="Site"
                        value={site}
                        options={sites.map((host) => ({ value: host, label: host }))}
                        onChange={(host) =>
                            updateJiraView(paneId, { site: host, issue: null, ...(view.list.kind === "filter" && { list: { kind: "mine" } }) })
                        }
                    />
                )}
                <div className="jira-lists">{MY_WORK.map(([list, label]) => listButton(list, label))}</div>
                {(projects.data?.length ?? 0) > 0 && project && (
                    <div className="jira-lists">
                        <div className="jira-heading">Project</div>
                        <Dropdown
                            label="Project"
                            value={project.key}
                            options={(projects.data ?? []).map((each) => ({ value: each.key, label: each.name, detail: each.key }))}
                            search="Find a project"
                            onChange={(key) => {
                                const chosen = projects.data?.find((each) => each.key === key);
                                if (!chosen) return;
                                setPicked(chosen);
                                show({ kind: listProject?.kind ?? "project", key: chosen.key, name: chosen.name });
                            }}
                        />
                        {listButton({ kind: "project", key: project.key, name: project.name }, "All open", `Open issues in ${project.name}`)}
                        {listButton(
                            { kind: "unassigned", key: project.key, name: project.name },
                            "Unassigned",
                            `Open issues in ${project.name} with nobody on them`,
                        )}
                    </div>
                )}
                {(boards.data?.length ?? 0) > 0 && (
                    <div className="jira-lists">
                        <div className="jira-heading">Boards</div>
                        {(boards.data?.length ?? 0) > 8 && (
                            <input
                                className="jira-board-filter"
                                value={boardFilter}
                                onChange={(event) => setBoardFilter(event.target.value)}
                                placeholder="Find a board"
                                aria-label="Find a board"
                                spellCheck={false}
                            />
                        )}
                        {shownBoards.map((board) =>
                            listButton(
                                { kind: "board", id: board.id, name: board.name },
                                board.name,
                                board.project ? `${board.name} · ${board.project}` : board.name,
                            ),
                        )}
                    </div>
                )}
                {(filters.data?.length ?? 0) > 0 && (
                    <div className="jira-lists">
                        <div className="jira-heading">Starred filters</div>
                        {filters.data?.map((filter) => {
                            const list: JiraList = { kind: "filter", ...filter };
                            return (
                                <button
                                    key={filter.id}
                                    type="button"
                                    className={`jira-list${sameList(view.list, list) ? " active" : ""}`}
                                    title={filter.jql}
                                    onClick={() => show(list)}>
                                    {filter.name}
                                </button>
                            );
                        })}
                    </div>
                )}
                <label className="jira-jql">
                    <span className="jira-heading">Search</span>
                    <input
                        value={jql}
                        onChange={(event) => setJql(event.target.value)}
                        onKeyDown={(event) => {
                            if (event.key === "Enter" && jql.trim()) show({ kind: "jql", jql: jql.trim() });
                        }}
                        placeholder="Words, or JQL like project = ABC"
                        spellCheck={false}
                    />
                </label>
                <button
                    type="button"
                    className="jira-signout"
                    onClick={() => void jiraApi.signOut(site).then(refreshJira).catch(reportError("sign out of Jira"))}>
                    Sign out of {site}
                </button>
            </nav>
            {view.list.kind === "board" ? (
                <JiraBoard
                    key={view.list.id}
                    active={active}
                    boardId={view.list.id}
                    site={site}
                    selected={view.issue}
                    onSelect={(key) => updateJiraView(paneId, { issue: key === view.issue ? null : key })}
                />
            ) : (
                <IssueList
                    key={jqlOf(view.list)}
                    active={active}
                    empty={EMPTY[view.list.kind] ?? "No issues here."}
                    jql={jqlOf(view.list)}
                    site={site}
                    selected={view.issue}
                    onSelect={(key) => updateJiraView(paneId, { issue: key })}
                />
            )}
            {view.list.kind === "board" ? (
                view.issue && (
                    <IssueOverlay onClose={() => updateJiraView(paneId, { issue: null })}>
                        <IssueDetail active={active} issueKey={view.issue} site={site} onClose={() => updateJiraView(paneId, { issue: null })} />
                    </IssueOverlay>
                )
            ) : view.issue ? (
                <IssueDetail active={active} issueKey={view.issue} site={site} onClose={() => updateJiraView(paneId, { issue: null })} />
            ) : (
                <div className="jira-detail">
                    <EmptyState message="Pick an issue to read it, comment, move it along or assign it." />
                </div>
            )}
        </div>
    );
}

function IssueList({
    active,
    empty,
    jql,
    site,
    selected,
    onSelect,
}: {
    active: boolean;
    empty: string;
    jql: string;
    site: string;
    selected: string | null;
    onSelect: (key: string) => void;
}) {
    const found = useResourceEnabled(active, jiraSearchR, jql, site);
    const [picked, setPicked] = useState<ReadonlySet<string>>(new Set());
    if (found.status === "error")
        return (
            <div className="jira-list-pane">
                <EmptyState message={found.error ?? "Jira could not run that search."} tone="error" />
            </div>
        );
    if (!found.data)
        return (
            <div className="jira-list-pane">
                <SkeletonRows rows={8} label="Loading issues" />
            </div>
        );
    if (found.data.issues.length === 0)
        return (
            <div className="jira-list-pane">
                <EmptyState message={empty} />
            </div>
        );
    const counts = statusCounts(found.data.issues);
    const shown = new Set([...picked].filter((status) => counts.some((each) => each.status === status)));
    const toggle = (status: string) => {
        const next = new Set(shown);
        if (!next.delete(status)) next.add(status);
        setPicked(next);
    };
    return (
        <div className="jira-list-pane">
            {counts.length > 1 && (
                <div className="jira-status-filter" role="group" aria-label="Filter by status">
                    <button type="button" className="jira-status-pick" aria-pressed={shown.size === 0} onClick={() => setPicked(new Set())}>
                        All <span className="jira-status-count">{found.data.issues.length}</span>
                    </button>
                    {counts.map(({ status, category, count }) => (
                        <button
                            key={status}
                            type="button"
                            className={`jira-status-pick jira-status-${category || "new"}`}
                            aria-pressed={shown.has(status)}
                            onClick={() => toggle(status)}>
                            {status} <span className="jira-status-count">{count}</span>
                        </button>
                    ))}
                </div>
            )}
            <div className="jira-issues" role="list">
                {withStatuses(found.data.issues, shown).map((issue) => (
                    <IssueRow key={issue.key} issue={issue} selected={issue.key === selected} onSelect={() => onSelect(issue.key)} />
                ))}
            </div>
        </div>
    );
}

function IssueRow({ issue, selected, onSelect }: { issue: JiraIssueSummary; selected: boolean; onSelect: () => void }) {
    const [menu, setMenu] = useMenu();
    return (
        <>
            <button
                type="button"
                role="listitem"
                className={`jira-row${selected ? " active" : ""}`}
                onClick={onSelect}
                onContextMenu={(event) => setMenu(event, issueMenu(issue))}>
                <span className="jira-row-top">
                    <span className="jira-key">{issue.key}</span>
                    {issue.priority && <span className="jira-meta">{issue.priority}</span>}
                    <StatusChip status={issue.status} category={issue.statusCategory} />
                </span>
                <span className="jira-row-summary">{issue.summary}</span>
                <span className="jira-meta">
                    {issue.assignee?.name ?? "Unassigned"}
                    {issue.sprint && ` · ${issue.sprint}`}
                </span>
            </button>
            {menu}
        </>
    );
}

/** A right-click menu: the element to render, and the handler that opens it at the pointer. */
function useMenu(): [ReactNode, (event: MouseEvent, items: ContextMenuItem[]) => void] {
    const [open, setOpen] = useState<{ x: number; y: number; items: ContextMenuItem[] } | null>(null);
    const element = open && <ContextMenu x={open.x} y={open.y} items={open.items} onClose={() => setOpen(null)} />;
    return [
        element,
        (event, items) => {
            event.preventDefault();
            setOpen({ x: event.clientX, y: event.clientY, items });
        },
    ];
}

function IssueDetail({ active, issueKey, site, onClose }: { active: boolean; issueKey: string; site: string; onClose: () => void }) {
    const found = useResourceEnabled(active, jiraIssueR, issueKey, site);
    if (found.status === "error")
        return (
            <div className="jira-detail">
                <EmptyState message={found.error ?? `Jira could not open ${issueKey}.`} tone="error" />
            </div>
        );
    if (!found.data)
        return (
            <div className="jira-detail">
                <SkeletonRows rows={6} label={`Loading ${issueKey}`} />
            </div>
        );
    return <IssueBody key={found.data.key} issue={found.data} site={site} refresh={() => void found.refresh()} onClose={onClose} />;
}

function IssueBody({ issue, site, refresh, onClose }: { issue: JiraIssue; site: string; refresh: () => void; onClose: () => void }) {
    const [menu, setMenu] = useMenu();
    const [comment, setComment] = useState("");
    const [busy, setBusy] = useState(false);
    const [assigning, setAssigning] = useState(false);
    const [people, setPeople] = useState<JiraPerson[]>([]);
    const changed = () => {
        refresh();
        refreshJira();
    };
    const toggleTask = useCallback<ToggleTask>(
        async (index, text, done) => {
            try {
                await jiraApi.setTask(issue.key, index, text, done, site || undefined);
                refresh();
            } catch (error) {
                notify("error", `Update the task: ${failureMessage(error)}`);
                throw error;
            }
        },
        [issue.key, site, refresh],
    );
    const run = async (label: string, work: () => Promise<unknown>) => {
        setBusy(true);
        try {
            await work();
            changed();
        } catch (error) {
            notify("error", `${label}: ${failureMessage(error)}`);
        } finally {
            setBusy(false);
        }
    };

    const assignee = (
        <span className="jira-detail-value">
            {issue.assignee ? <Person name={issue.assignee.name} /> : <span className="jira-meta">Unassigned</span>}
            <span className="jira-detail-actions">
                <button
                    type="button"
                    className="jira-link-button"
                    disabled={busy}
                    onClick={() => void run("Assign", () => jiraApi.assign(issue.key, { me: true }, site || undefined))}>
                    Assign to me
                </button>
                <button
                    type="button"
                    className="jira-link-button"
                    disabled={busy || !issue.assignee}
                    onClick={() => void run("Unassign", () => jiraApi.assign(issue.key, { accountId: null }, site || undefined))}>
                    Unassign
                </button>
                <button type="button" className="jira-link-button" disabled={busy} onClick={() => setAssigning((open) => !open)}>
                    Assign…
                </button>
            </span>
        </span>
    );

    return (
        <article className="jira-detail jira-issue" aria-label={`${issue.key} ${issue.summary}`}>
            <header className="jira-detail-head" onContextMenu={(event) => setMenu(event, issueMenu(issue))}>
                <span className="jira-detail-key">
                    {issue.parent && (
                        <>
                            <TypeMark type={issue.parent.issueType} size={13} />
                            <span className="jira-key" title={issue.parent.summary}>
                                {issue.parent.key}
                            </span>
                            <span className="jira-meta">/</span>
                        </>
                    )}
                    <TypeMark type={issue.issueType} size={13} />
                    <button type="button" className="jira-key link" title="Open in browser" onClick={() => openIssue(issue)}>
                        {issue.key}
                    </button>
                    <span className="jira-grow" />
                    <button
                        type="button"
                        className="jira-icon-button"
                        title="Copy link"
                        aria-label="Copy link"
                        onClick={() => copyIssue(issue.url, "link")}>
                        <IconCopy size={12} />
                    </button>
                    <button
                        type="button"
                        className="jira-icon-button"
                        title="Open in browser"
                        aria-label="Open in browser"
                        onClick={() => openIssue(issue)}>
                        <IconExternal size={12} />
                    </button>
                    <button type="button" className="jira-icon-button" title="Close" aria-label="Close the issue" onClick={onClose}>
                        <IconClose size={12} />
                    </button>
                </span>
                {menu}
            </header>
            <h2 className="jira-issue-title">{issue.summary}</h2>
            <div className="jira-issue-grid">
                <div className="jira-issue-main">
                    <Fold title="Description">
                        <section className="jira-description">
                            {issue.description ? (
                                <TaskToggleContext.Provider value={toggleTask}>
                                    <Markdown taskBox={TaskBox}>{issue.description}</Markdown>
                                </TaskToggleContext.Provider>
                            ) : (
                                <p className="jira-meta">No description.</p>
                            )}
                        </section>
                    </Fold>
                    <section className="jira-comments" aria-label="Comments">
                        <div className="jira-heading">
                            Comments{issue.commentCount > issue.comments.length ? ` (latest ${issue.comments.length} of ${issue.commentCount})` : ""}
                        </div>
                        {issue.comments.map((entry) => (
                            <div key={entry.id} className="jira-comment">
                                <div className="jira-meta">
                                    {entry.author} · {new Date(entry.created).toLocaleString()}
                                </div>
                                <Markdown>{entry.body}</Markdown>
                            </div>
                        ))}
                        <textarea
                            value={comment}
                            onChange={(event) => setComment(event.target.value)}
                            placeholder="Add a comment in markdown"
                            rows={3}
                        />
                        <button
                            type="button"
                            className="jira-chip primary"
                            disabled={busy || !comment.trim()}
                            onClick={() =>
                                void run("Comment", async () => {
                                    await jiraApi.comment(issue.key, comment, site || undefined);
                                    setComment("");
                                })
                            }>
                            Comment
                        </button>
                    </section>
                </div>
                <aside className="jira-issue-side">
                    <div className="jira-facts">
                        <Dropdown
                            label="Status"
                            value=""
                            title="Move this issue along its workflow"
                            options={issue.transitions.map((transition) => ({
                                value: transition.id,
                                label: transition.name,
                                detail: `→ ${transition.to}`,
                            }))}
                            onChange={(id) => void run("Move issue", () => jiraApi.transition(issue.key, id, site || undefined))}
                            disabled={busy || issue.transitions.length === 0}
                            trailing={<StatusChip status={issue.status} category={issue.statusCategory} />}
                        />
                    </div>
                    <Fold title="Details" boxed>
                        <dl className="jira-details">
                            <Detail label="Assignee">{assignee}</Detail>
                            {assigning && (
                                <div className="jira-assign">
                                    <input
                                        autoFocus
                                        placeholder="Find a person"
                                        onChange={(event) =>
                                            void jiraApi
                                                .assignable(issue.key, event.target.value, site || undefined)
                                                .then(setPeople)
                                                .catch(swallow("find people"))
                                        }
                                    />
                                    {people.map((person) => (
                                        <button
                                            key={person.accountId}
                                            type="button"
                                            className="jira-chip"
                                            onClick={() => {
                                                setAssigning(false);
                                                void run("Assign", () =>
                                                    jiraApi.assign(issue.key, { accountId: person.accountId }, site || undefined),
                                                );
                                            }}>
                                            {person.name}
                                        </button>
                                    ))}
                                </div>
                            )}
                            <Detail label="Reporter">{issue.reporter ? <Person name={issue.reporter.name} /> : null}</Detail>
                            <Detail label="Labels">
                                {issue.labels.length > 0 ? (
                                    <span className="jira-labels">
                                        {issue.labels.map((label) => (
                                            <span key={label} className="jira-label">
                                                {label}
                                            </span>
                                        ))}
                                    </span>
                                ) : null}
                            </Detail>
                            <Detail label="Due date">{issue.due ? dueLabel(issue.due) : null}</Detail>
                            <Detail label="Start date">{issue.start ? dueLabel(issue.start) : null}</Detail>
                            <Detail label="Priority">
                                {issue.priority ? (
                                    <span className="jira-detail-value">
                                        <PriorityMark priority={issue.priority} />
                                        {issue.priority}
                                    </span>
                                ) : null}
                            </Detail>
                            <Detail label="Parent">
                                {issue.parent ? (
                                    <span className="jira-lozenge" title={issue.parent.summary}>
                                        <span className="jira-swatch" style={{ background: parentColour(issue.parent.key) }} />
                                        <span className="jira-lozenge-text">
                                            {issue.parent.key} {issue.parent.summary}
                                        </span>
                                    </span>
                                ) : null}
                            </Detail>
                            <Detail label="Sprint">{issue.sprint}</Detail>
                            <Detail label="Fix versions">{issue.versions.length > 0 ? issue.versions.join(", ") : null}</Detail>
                        </dl>
                    </Fold>
                    <Fold title="Development" boxed closed>
                        <ProjectCommits issueKey={issue.key} />
                    </Fold>
                    <Fold title="More fields" boxed closed hint="Original estimate, Time tracking, Components">
                        <dl className="jira-details">
                            <Detail label="Original estimate">{issue.originalEstimate}</Detail>
                            <Detail label="Time spent">{issue.timeSpent}</Detail>
                            <Detail label="Components">{issue.components.length > 0 ? issue.components.join(", ") : null}</Detail>
                            <Detail label="Issue type">{issue.issueType}</Detail>
                            <Detail label="Project">{issue.project}</Detail>
                        </dl>
                    </Fold>
                    <div className="jira-issue-dates">
                        {issue.created && <span>Created {ago(issue.created)}</span>}
                        {issue.updated && <span>Updated {ago(issue.updated)}</span>}
                        {issue.resolved && <span>Resolved {ago(issue.resolved)}</span>}
                    </div>
                </aside>
            </div>
        </article>
    );
}

/** An issue opened from a board, in a window over the board as Jira opens it. */
function IssueOverlay({ onClose, children }: { onClose: () => void; children: ReactNode }) {
    useEffect(() => {
        const escape = (event: KeyboardEvent) => {
            if (event.key === "Escape" && !event.defaultPrevented) onClose();
        };
        window.addEventListener("keydown", escape);
        return () => window.removeEventListener("keydown", escape);
    }, [onClose]);
    return (
        <div
            className="jira-issue-overlay"
            onPointerDown={(event) => {
                if (event.target === event.currentTarget) onClose();
            }}>
            <div className="jira-issue-window">{children}</div>
        </div>
    );
}

/** A part of the issue that opens and closes under its title, as Jira's do. */
function Fold({
    title,
    hint,
    boxed = false,
    closed = false,
    children,
}: {
    title: string;
    hint?: string;
    boxed?: boolean;
    closed?: boolean;
    children: ReactNode;
}) {
    const [open, setOpen] = useState(!closed);
    return (
        <section className={`jira-fold${boxed ? " boxed" : ""}`}>
            <button type="button" className="jira-fold-head" aria-expanded={open} onClick={() => setOpen(!open)}>
                <IconChevron size={10} />
                <span className="jira-fold-title">{title}</span>
                {!open && hint && <span className="jira-meta">{hint}</span>}
            </button>
            {open && <div className="jira-fold-body">{children}</div>}
        </section>
    );
}

function Detail({ label, children }: { label: string; children: ReactNode }) {
    return (
        <>
            <dt>{label}</dt>
            <dd>{children ?? <span className="jira-meta">None</span>}</dd>
        </>
    );
}

function Person({ name }: { name: string }) {
    const letters = name
        .split(/\s+/)
        .filter(Boolean)
        .slice(0, 2)
        .map((part) => part[0]?.toUpperCase() ?? "")
        .join("");
    return (
        <span className="jira-detail-value">
            <span className="jira-avatar" aria-hidden>
                {letters}
            </span>
            {name}
        </span>
    );
}

/** A time as Jira's issue footer says it, such as 5 days ago. */
function ago(at: string, now = Date.now()): string {
    const then = Date.parse(at);
    if (Number.isNaN(then)) return at;
    const seconds = Math.round((then - now) / 1000);
    const words = new Intl.RelativeTimeFormat("en", { numeric: "auto" });
    const steps: [Intl.RelativeTimeFormatUnit, number][] = [
        ["year", 31_536_000],
        ["month", 2_592_000],
        ["day", 86_400],
        ["hour", 3_600],
        ["minute", 60],
    ];
    for (const [unit, size] of steps) {
        if (Math.abs(seconds) >= size) return words.format(Math.round(seconds / size), unit);
    }
    return "just now";
}

/** The open project's recent commits that name the issue in their message. */
function ProjectCommits({ issueKey }: { issueKey: string }) {
    const cwd = useActiveProjectCwd();
    const repo = useResourceEnabled(!!cwd, gitOverviewR, cwd ?? "");
    const commits = (repo.data?.log ?? []).filter((commit) => mentions(commit.subject, issueKey));
    if (commits.length === 0) return null;
    return (
        <section className="jira-commits" aria-label="Commits in this project">
            <div className="jira-heading">Commits in this project</div>
            {commits.map((commit) => (
                <div key={commit.full_hash} className="jira-commit">
                    <span className="jira-key">{commit.hash}</span>
                    <span>{commit.subject}</span>
                    <span className="jira-meta">{commit.author}</span>
                </div>
            ))}
        </section>
    );
}
