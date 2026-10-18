import type { ReactNode } from "react";
import { notify, openUrl, reportError, swallow } from "../../plugin-api/host";
import { invalidate, useResourceEnabled } from "../../plugin-api/resources";
import { EmptyState, IconExternal, IconInfo, IconPlus, SkeletonRows, StaleNotice, Tooltip } from "../../plugin-api/ui";
import { GitColumns } from "../../git/GitColumns";
import { hostApi, failureMessage, type Issue, type RepoRef } from "../api";
import { issueCountsR, issueR, issuesR } from "../resources";
import { compose, setListState, showItem, updateView } from "../state";
import { Comments, Labels, StateMark, Who } from "./Bits";
import { CommentThread, Face } from "./CommentThread";
import { useHost } from "../registry";
import { useBusy } from "./hooks";
import { useNow } from "../../plugin-api/hooks";
import { NewIssueForm } from "./NewIssueForm";
import { IconAgent } from "../../ui/Icons";
import { workOnIssue } from "../workOnIssue";
import { formatAgo } from "../../lib/format";

const LIST_STATES = ["open", "closed", "all"];

/** Starts a chat in the project with this issue in its input. Only offered where the issue's repository is the project's own. */
function WorkOnThis({ repo, number, cwd, compact = false }: { repo: RepoRef; number: number; cwd: string; compact?: boolean }) {
    const [busy, runBusy] = useBusy();
    const start = () => runBusy(() => workOnIssue(repo, number, cwd));
    if (compact)
        return (
            <Tooltip label="Work on this">
                <button type="button" className="gha-icon-btn gha-work-on" aria-label={`Work on #${number}`} disabled={busy} onClick={start}>
                    <IconAgent size={13} />
                </button>
            </Tooltip>
        );
    return (
        <button type="button" className="gha-btn" disabled={busy} onClick={start}>
            <IconAgent size={12} /> Work on this
        </button>
    );
}

/** An issue as the full list shows it, read like the pull request list: who opened it, its state and title, then its number, who and when, with who it is assigned to, its comments and starting work at the end. */
function IssueListRow({ issue, now, onOpen, work }: { issue: Issue; now: number; onOpen: () => void; work: ReactNode }) {
    const opened = formatAgo(issue.createdAt, now);
    const updated = formatAgo(issue.updatedAt, now);
    return (
        <div className="issue-row">
            <button type="button" className="pr-row pr-headline" onClick={onOpen}>
                <span className="pr-face">
                    <Face login={issue.author} url={issue.avatarUrl} />
                </span>
                <span className="pr-headline-top">
                    <StateMark kind="issue" state={issue.state} reason={issue.stateReason} size={14} />
                    <span className="pr-headline-title">{issue.title}</span>
                </span>
                <span className="pr-headline-sub">
                    <span className="pr-headline-who">
                        #{issue.number} · {issue.author ?? "someone"} opened {opened}
                        {updated !== opened && <> · updated {updated}</>}
                    </span>
                    <Labels labels={issue.labels} />
                </span>
                <span className="pr-row-side">
                    {issue.assignees.length > 0 && (
                        <span className="pr-row-reviewers" title={`Assigned to ${issue.assignees.join(", ")}`}>
                            {issue.assignees.map((login) => (
                                <Face key={login} login={login} url={null} />
                            ))}
                        </span>
                    )}
                    <Comments count={issue.comments} />
                </span>
            </button>
            {work}
        </div>
    );
}

function IssueRow({ issue, now, selected, onOpen, work }: { issue: Issue; now: number; selected: boolean; onOpen: () => void; work: ReactNode }) {
    return (
        <div className="gha-item-line">
            <button type="button" className="gha-item-row" data-on={selected ? "1" : "0"} onClick={onOpen}>
                <StateMark kind="issue" state={issue.state} reason={issue.stateReason} />
                <span className="gha-item-head">
                    <span className="gha-item-title">{issue.title}</span>
                    <Labels labels={issue.labels} />
                </span>
                <Comments count={issue.comments} />
                <span className="gha-item-sub">
                    <span className="gha-item-number">#{issue.number}</span>
                    {issue.author && <Who login={issue.author} avatarUrl={issue.avatarUrl} />}
                    {issue.assignees.length > 0 && <span>→ {issue.assignees.join(", ")}</span>}
                </span>
                <span className="gha-item-when">{formatAgo(issue.updatedAt, now)}</span>
            </button>
            {work}
        </div>
    );
}

export function IssueDetail({ repo, number, active, cwd = null }: { repo: RepoRef; number: number; active: boolean; cwd?: string | null }) {
    const host = useHost();
    const issue = useResourceEnabled(active, issueR, repo, number);
    const now = useNow(false);
    const [busy, runBusy] = useBusy();
    if (issue.status === "loading" && !issue.data) return <SkeletonRows rows={6} label="Loading issue" />;
    if (!issue.data) {
        return <EmptyState title="Could not read it" message={failureMessage(issue.error)} tone="error" />;
    }
    const found = issue.data;
    const closing = found.state === "open";

    const setState = () =>
        runBusy(() =>
            hostApi(repo.provider)
                .setIssueState(repo, found.number, closing ? "closed" : "open")
                .then(() => {
                    notify("success", closing ? `Closed #${found.number}` : `Reopened #${found.number}`);
                    invalidate((kind) => kind === "host.issue" || kind === "host.issues");
                })
                .catch(reportError(closing ? "Could not close it" : "Could not reopen it")),
        );

    return (
        <div className="pr-right">
            <div className="git-detail">
                <div className="pr-headline">
                    <span className="pr-face">
                        <Face login={found.author} url={found.avatarUrl} />
                    </span>
                    <span className="pr-headline-top">
                        <StateMark kind="issue" state={found.state} reason={found.stateReason} size={14} />
                        <h2 className="git-detail-title pr-headline-title">{found.title}</h2>
                        <span className="issue-head-actions">
                            {cwd && <WorkOnThis repo={repo} number={found.number} cwd={cwd} />}
                            <Tooltip label={`Open on ${host.name}`}>
                                <button
                                    type="button"
                                    className="gha-icon-btn"
                                    aria-label={`Open on ${host.name}`}
                                    onClick={() => void openUrl(found.url).catch(swallow(`open ${host.name}`))}>
                                    <IconExternal size={12} />
                                </button>
                            </Tooltip>
                        </span>
                    </span>
                    <span className="pr-headline-sub">
                        <span className="pr-headline-who">
                            #{found.number} · {found.author ?? "someone"} opened {formatAgo(found.createdAt, now)}
                            {found.assignees.length > 0 && <> · assigned to {found.assignees.join(", ")}</>}
                        </span>
                        <Labels labels={found.labels} />
                    </span>
                </div>
            </div>
            <div className="pr-conversation">
                <CommentThread
                    repo={repo}
                    number={found.number}
                    of="issue"
                    active={active}
                    now={now}
                    opening={{
                        key: "opening",
                        author: found.author,
                        avatarUrl: found.avatarUrl,
                        association: null,
                        at: found.createdAt,
                        body: found.body,
                        review: null,
                    }}
                    extraActions={
                        <button type="button" className="gha-btn" disabled={busy} onClick={setState}>
                            {closing ? "Close issue" : "Reopen issue"}
                        </button>
                    }
                />
            </div>
        </div>
    );
}

interface Props {
    paneId: string;
    repo: RepoRef;
    listState: string;
    item: number | null;
    composing: boolean;
    page: number;
    /** The project folder, when the issues are its own repository's, so an agent can be started on one there. */
    cwd: string | null;
    active: boolean;
}

export function IssuesView({ paneId, repo, listState, item, composing, page, cwd, active }: Props) {
    const issues = useResourceEnabled(active && !composing, issuesR, repo, listState, page);
    const counts = useResourceEnabled(active && !composing && item === null, issueCountsR, repo).data;
    const now = useNow(false);

    if (composing) return <NewIssueForm repo={repo} onCreated={(number) => showItem(paneId, number)} onCancel={() => compose(paneId, null)} />;

    const rows = issues.data?.issues ?? [];
    const total = issues.data?.total ?? rows.length;
    const nextPage = issues.data?.nextPage ?? null;
    const list =
        issues.status === "loading" && !issues.data ? (
            <SkeletonRows rows={8} label="Loading issues" />
        ) : issues.error && !issues.data ? (
            <EmptyState
                title="Could not read issues"
                message={failureMessage(issues.error)}
                tone="error"
                action={{ label: "Try again", onClick: () => void issues.refresh() }}
            />
        ) : (
            <div className="gha-list pr-list">
                <StaleNotice handle={issues} />
                <div className="gha-list-head">
                    <div className="gha-chips gha-seg" role="group" aria-label="Which to list">
                        {LIST_STATES.map((state) => (
                            <button
                                key={state}
                                type="button"
                                className="gha-chip"
                                data-on={listState === state ? "1" : "0"}
                                onClick={() => setListState(paneId, "issues", state)}>
                                {state === "all" ? "All" : state === "open" ? "Open" : "Closed"}
                                {counts && state !== "all" && (
                                    <span className="gha-chip-count">{counts[state as "open" | "closed"].toLocaleString()}</span>
                                )}
                            </button>
                        ))}
                    </div>
                    <span className="gha-page-spacer" />
                    <span className="gha-dim">{total}</span>
                    <Tooltip label="New issue">
                        <button type="button" className="gha-icon-btn" aria-label="New issue" onClick={() => compose(paneId, "issue")}>
                            <IconPlus size={13} />
                        </button>
                    </Tooltip>
                </div>
                {rows.length === 0 ? (
                    <EmptyState icon={<IconInfo size={20} />} message={`No ${listState === "all" ? "" : listState} issues.`} />
                ) : item === null ? (
                    <div className="pr-rows">
                        {rows.map((issue) => (
                            <IssueListRow
                                key={issue.number}
                                issue={issue}
                                now={now}
                                onOpen={() => showItem(paneId, issue.number)}
                                work={cwd && issue.state === "open" && <WorkOnThis repo={repo} number={issue.number} cwd={cwd} compact />}
                            />
                        ))}
                    </div>
                ) : (
                    rows.map((issue) => (
                        <IssueRow
                            key={issue.number}
                            issue={issue}
                            now={now}
                            selected={issue.number === item}
                            onOpen={() => showItem(paneId, issue.number)}
                            work={cwd && issue.state === "open" && <WorkOnThis repo={repo} number={issue.number} cwd={cwd} compact />}
                        />
                    ))
                )}
                {(page > 1 || nextPage) && (
                    <div className="gha-pager">
                        <button type="button" className="gha-btn" disabled={page <= 1} onClick={() => updateView(paneId, { page: page - 1 })}>
                            Newer
                        </button>
                        <span className="gha-dim">Page {page}</span>
                        <button type="button" className="gha-btn" disabled={!nextPage} onClick={() => updateView(paneId, { page: page + 1 })}>
                            Older
                        </button>
                    </div>
                )}
            </div>
        );

    if (item === null) return <div className="gha-full-page">{list}</div>;
    const right = (
        <div className="issue-page">
            <IssueDetail repo={repo} number={item} active={active} cwd={cwd} />
        </div>
    );
    return <GitColumns paneId={paneId} left={list} right={<div className="git-right-review">{right}</div>} />;
}
