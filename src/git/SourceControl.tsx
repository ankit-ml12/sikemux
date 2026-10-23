import { useMemo, useState, type KeyboardEvent, type ReactNode } from "react";
import { git, hasUnstaged, isStaged, type GitFile, type GitStatus } from "../api/git";
import { PRIMARY_SHORTCUT } from "../lib/platform";
import { basename, dirname, joinPath } from "../lib/paths";
import { TreeContextMenu, type CtxItem } from "../rail/FileTree";
import * as cmd from "../state/commands";
import { confirmDialog } from "../state/dialog";
import { commitGitDraft, generateGitDraft, runRepositoryGit, setGitDraft, useGitWorkbench } from "../state/gitWorkbench";
import { invalidate, useCachedResourceEnabled } from "../state/resources";
import { gitDiscoveredReposR, gitStatusR } from "../state/resources.defs";
import { getState, useStore } from "../state/store";
import { DEFAULT_GIT_VIEW } from "../state/types";
import { FileIcon } from "../ui/FileIcon";
import {
    IconChevron,
    IconDiscard,
    IconDownload,
    IconFile,
    IconGit,
    IconMinus,
    IconPanel,
    IconPlus,
    IconRefresh,
    IconSparkle,
    IconStar,
    IconUpload,
} from "../ui/Icons";
import { SkeletonRows } from "../ui/Skeleton";
import { Tooltip } from "../ui/Tooltip";
import { gitStatusDecoration } from "./gitFileStatus";
import { repoAction, reposIn, sectionOpen, sourceControlRoots, type RepoAction, type RepoSummary } from "./sourceControlRepos";

type Side = "staged" | "unstaged";

const NO_FOLDS: Readonly<Record<string, boolean>> = {};

/** Reads a repository again everywhere it shows: its section, its full view, and the summary of the folder it sits in. */
function refreshRepo(repo: string): void {
    invalidate(
        (kind, args) =>
            ((kind === "git.status" || kind === "git.overview" || kind === "git.changeOwners") && args[0] === repo) || kind === "git.discoveredRepos",
    );
}

function runIn(repo: string, label: string, action: () => Promise<unknown>): Promise<boolean> {
    return runRepositoryGit(repo, label, action).finally(() => refreshRepo(repo));
}

function setRepoFold(paneId: string, repo: string, open: boolean): void {
    const folds = getState().gitViews[paneId]?.repoFolds ?? NO_FOLDS;
    cmd.setGitView(paneId, { repoFolds: { ...folds, [repo]: open } });
}

/** Opens a repository's full view, with one of its changed files picked when one is given. */
function openFullView(paneId: string, repo: string, files: readonly GitFile[], picked?: { file: GitFile; side: Side }): void {
    const staged = files.filter(isStaged);
    const unstaged = files.filter(hasUnstaged);
    let index = 0;
    if (picked) {
        const stagedRows = staged.length ? staged.length + 1 : 0;
        index =
            picked.side === "staged"
                ? 1 + staged.findIndex((file) => file.path === picked.file.path)
                : stagedRows + 1 + unstaged.findIndex((file) => file.path === picked.file.path);
    }
    cmd.setGitView(paneId, { sourceControl: false, repo, listing: false, panel: "files", selected: { ...DEFAULT_GIT_VIEW.selected, files: index } });
}

function ActButton({ label, onClick, disabled, children }: { label: string; onClick: () => void; disabled?: boolean; children: ReactNode }) {
    return (
        <Tooltip label={label}>
            <button
                type="button"
                className="git-row-act"
                aria-label={label}
                disabled={disabled}
                onClick={(event) => {
                    event.stopPropagation();
                    onClick();
                }}>
                {children}
            </button>
        </Tooltip>
    );
}

/** Every repository's changes in one list, a section each, as a code editor's source control shows several at once. */
export function SourceControl({ paneId, cwd, active }: { paneId: string; cwd: string; active: boolean }) {
    const scope = useStore((s) => s.gitViews[paneId]?.scope ?? "project");
    const folds = useStore((s) => s.gitViews[paneId]?.repoFolds ?? NO_FOLDS);
    const sessions = useStore((s) => s.sessions);
    const sessionOrder = useStore((s) => s.sessionOrder);
    const roots = useMemo(() => sourceControlRoots({ sessions, sessionOrder }, cwd, scope), [sessions, sessionOrder, cwd, scope]);

    return (
        <div className="git-pane git-source-control">
            <div className="git-toolbar">
                <span className="git-tb-status">
                    <IconGit size={13} className="git-tb-icon" />
                    <span className="git-tb-branch">Source Control</span>
                </span>
                <span className="git-tb-grow" />
                <div className="git-sc-scope" role="radiogroup" aria-label="Repositories from">
                    {(["project", "all"] as const).map((value) => (
                        <button
                            key={value}
                            type="button"
                            role="radio"
                            aria-checked={scope === value}
                            className={scope === value ? "on" : undefined}
                            onClick={() => cmd.setGitView(paneId, { scope: value })}>
                            {value === "project" ? "This project" : "All projects"}
                        </button>
                    ))}
                </div>
            </div>
            <div className="git-sc-list">
                {roots.map((root) => (
                    <ProjectRepos
                        key={root}
                        paneId={paneId}
                        root={root}
                        heading={scope === "all" ? basename(root) : null}
                        folds={folds}
                        active={active}
                    />
                ))}
            </div>
        </div>
    );
}

function ProjectRepos({
    paneId,
    root,
    heading,
    folds,
    active,
}: {
    paneId: string;
    root: string;
    heading: string | null;
    folds: Readonly<Record<string, boolean>>;
    active: boolean;
}) {
    const rootStatus = useCachedResourceEnabled(active, gitStatusR, root);
    const discovered = useCachedResourceEnabled(active, gitDiscoveredReposR, root);
    const repos = useMemo(() => reposIn(root, rootStatus.data ?? null, discovered.data ?? []), [root, rootStatus.data, discovered.data]);
    const loading = (rootStatus.status === "loading" && !rootStatus.data) || (discovered.status === "loading" && !discovered.data);

    return (
        <section className="git-sc-project" aria-label={heading ?? basename(root)}>
            {heading && <div className="git-sc-project-name">{heading}</div>}
            {loading && repos.length === 0 ? (
                <SkeletonRows rows={2} />
            ) : repos.length === 0 ? (
                <p className="git-sc-empty">No repository in {basename(root)}, or in the folders directly inside it.</p>
            ) : (
                repos.map((repo) => (
                    <RepoSection key={repo.path} paneId={paneId} root={root} repo={repo} open={sectionOpen(folds, repo)} active={active} />
                ))
            )}
        </section>
    );
}

function RepoSection({ paneId, root, repo, open, active }: { paneId: string; root: string; repo: RepoSummary; open: boolean; active: boolean }) {
    const byDefault = useStore((s) => s.defaultGitRepos[root] === repo.path);
    const status = useCachedResourceEnabled(active && open, gitStatusR, repo.path);
    const live = status.data;
    const operation = useGitWorkbench((state) => state.operations[repo.path]);
    const busy = !!operation?.busy;
    const branch = live?.branch ?? repo.branch;
    const ahead = live?.ahead ?? repo.ahead;
    const behind = live?.behind ?? repo.behind;
    const changes = live?.files.length ?? repo.changes;
    const pushLabel = live && live.branch && !live.upstream ? "Publish branch" : "Push";

    return (
        <div className={`git-sc-repo${open ? " open" : ""}`}>
            <div
                className="git-sc-head"
                role="button"
                tabIndex={0}
                aria-expanded={open}
                aria-label={repo.name}
                onClick={() => setRepoFold(paneId, repo.path, !open)}
                onKeyDown={(event) => {
                    if (event.key === "Enter" || event.key === " ") {
                        event.preventDefault();
                        setRepoFold(paneId, repo.path, !open);
                    }
                }}>
                <IconChevron size={9} className="git-sc-chev" />
                <IconGit size={13} className={`git-repo-icon${changes > 0 ? " dirty" : ""}`} />
                <span className="git-sc-name">{repo.name}</span>
                <span className="git-sc-branch">{branch || "no commits"}</span>
                {ahead > 0 && <span className="git-repo-sync">{ahead}↑</span>}
                {behind > 0 && <span className="git-repo-sync">{behind}↓</span>}
                <span className="git-tb-grow" />
                <span className="git-sc-acts">
                    <ActButton label="Fetch" disabled={busy} onClick={() => void runIn(repo.path, "Fetch", () => git.fetch(repo.path))}>
                        <IconRefresh size={12} />
                    </ActButton>
                    <ActButton label="Pull" disabled={busy} onClick={() => void runIn(repo.path, "Pull", () => git.pull(repo.path))}>
                        <IconDownload size={12} />
                    </ActButton>
                    <ActButton label={pushLabel} disabled={busy} onClick={() => void runIn(repo.path, pushLabel, () => git.push(repo.path))}>
                        <IconUpload size={12} />
                    </ActButton>
                    {repo.path !== root && (
                        <ActButton
                            label={byDefault ? "Opened by default in this project; click to stop" : "Open this one by default in this project"}
                            onClick={() => cmd.setDefaultGitRepo(root, byDefault ? null : repo.path)}>
                            <IconStar size={12} className={byDefault ? "git-sc-default" : undefined} />
                        </ActButton>
                    )}
                    <ActButton label="Open the full Git view" onClick={() => openFullView(paneId, repo.path, live?.files ?? [])}>
                        <IconPanel size={12} />
                    </ActButton>
                </span>
                {changes > 0 && <span className="git-sc-count">{changes}</span>}
            </div>
            {open &&
                (live ? (
                    <RepoChanges paneId={paneId} repo={repo.path} status={live} busy={busy} />
                ) : status.status === "error" ? (
                    <p className="git-sc-error">{status.error ?? "Could not read this repository."}</p>
                ) : (
                    <SkeletonRows rows={2} />
                ))}
            {operation && !operation.busy && operation.error && <p className="git-sc-error">{operation.error}</p>}
            {busy && <p className="git-sc-busy">{operation?.label}…</p>}
        </div>
    );
}

function RepoChanges({ paneId, repo, status, busy }: { paneId: string; repo: string; status: GitStatus; busy: boolean }) {
    const draft = useGitWorkbench((state) => state.drafts[repo] ?? "");
    const generating = busy && (useGitWorkbench.getState().operations[repo]?.label.startsWith("Generate message") ?? false);
    const [menu, setMenu] = useState<{ x: number; y: number } | null>(null);
    const staged = status.files.filter(isStaged);
    const unstaged = status.files.filter(hasUnstaged);
    const action = repoAction(status, draft);
    const committing = action.kind === "commit" || action.kind === "commitAll";

    const commitAll = (thenPush: boolean) =>
        runIn(repo, thenPush ? "Commit all and push" : "Commit all changes", async () => {
            await git.stageAll(repo);
            const result = await git.commit(repo, draft.trim());
            if (useGitWorkbench.getState().drafts[repo] === draft) setGitDraft(repo, "");
            return thenPush ? `${result}\n${await git.push(repo)}` : result;
        });
    const commitStaged = async (thenPush: boolean) => {
        const committed = await commitGitDraft(repo);
        refreshRepo(repo);
        if (committed && thenPush) await runIn(repo, "Push", () => git.push(repo));
    };

    const perform = (chosen: RepoAction, thenPush = false) => {
        if (!chosen.ready || busy) return;
        if (chosen.kind === "commit") void commitStaged(thenPush);
        else if (chosen.kind === "commitAll") void commitAll(thenPush);
        else if (chosen.kind === "publish") void runIn(repo, "Publish branch", () => git.push(repo));
        else if (chosen.kind === "push") void runIn(repo, "Push", () => git.push(repo));
        else if (chosen.kind === "pull") void runIn(repo, "Pull", () => git.pull(repo));
    };

    const onKeyDown = (event: KeyboardEvent<HTMLTextAreaElement>) => {
        if (event.key === "Enter" && (event.metaKey || event.ctrlKey)) {
            event.preventDefault();
            perform(action);
        }
    };

    const discard = async (file: GitFile) => {
        const untracked = file.worktree === "?";
        const yes = await confirmDialog({
            title: `Discard changes to ${basename(file.path)}?`,
            body: untracked ? "The file is new, so it is deleted. This can't be undone." : "Its unstaged changes are lost. This can't be undone.",
            confirmLabel: "Discard",
            destructive: true,
        });
        if (yes) void runIn(repo, `Discard ${file.path}`, () => git.discardFiles(repo, [file.path], "unstaged"));
    };

    const menuItems: CtxItem[] = [
        { label: "Commit & Push", disabled: !action.ready || !committing, run: () => perform(action, true) },
        { label: "Commit", disabled: !action.ready || !committing, run: () => perform(action) },
    ];

    return (
        <div className="git-sc-body">
            <div className="git-sc-message">
                <textarea
                    aria-label={`Commit message for ${basename(repo)}`}
                    placeholder={`Message (${PRIMARY_SHORTCUT}Enter to ${committing ? "commit" : action.label.toLowerCase()})`}
                    value={draft}
                    rows={1}
                    disabled={busy && !generating}
                    onChange={(event) => setGitDraft(repo, event.target.value)}
                    onKeyDown={onKeyDown}
                />
                <ActButton label="Write the message with AI" disabled={busy || status.files.length === 0} onClick={() => void generateGitDraft(repo)}>
                    <IconSparkle size={11} />
                </ActButton>
            </div>
            <div className="git-sc-commit">
                <button type="button" className="git-sc-primary" disabled={!action.ready || busy} onClick={() => perform(action)}>
                    {action.label}
                </button>
                {committing && (
                    <button
                        type="button"
                        className="git-sc-primary git-sc-more"
                        aria-label="More ways to commit"
                        disabled={!action.ready || busy}
                        onClick={(event) => {
                            const box = event.currentTarget.getBoundingClientRect();
                            setMenu({ x: box.right, y: box.bottom + 4 });
                        }}>
                        <IconChevron size={9} />
                    </button>
                )}
            </div>
            {menu && <TreeContextMenu x={menu.x} y={menu.y} items={menuItems} alignRight onClose={() => setMenu(null)} />}
            {staged.length > 0 && (
                <FileGroup
                    title="Staged Changes"
                    files={staged}
                    side="staged"
                    groupAction={
                        <ActButton label="Unstage all" disabled={busy} onClick={() => void runIn(repo, "Unstage all", () => git.unstageAll(repo))}>
                            <IconMinus size={12} />
                        </ActButton>
                    }
                    onOpen={(file) => openFullView(paneId, repo, status.files, { file, side: "staged" })}
                    renderActions={(file) => (
                        <>
                            <ActButton label={`Open ${file.path}`} onClick={() => cmd.requestOpenFile(joinPath(repo, file.path))}>
                                <IconFile size={12} />
                            </ActButton>
                            <ActButton
                                label={`Unstage ${file.path}`}
                                disabled={busy}
                                onClick={() => void runIn(repo, `Unstage ${file.path}`, () => git.unstage(repo, file.path))}>
                                <IconMinus size={12} />
                            </ActButton>
                        </>
                    )}
                />
            )}
            {unstaged.length > 0 && (
                <FileGroup
                    title="Changes"
                    files={unstaged}
                    side="unstaged"
                    groupAction={
                        <ActButton label="Stage all" disabled={busy} onClick={() => void runIn(repo, "Stage all", () => git.stageAll(repo))}>
                            <IconPlus size={12} />
                        </ActButton>
                    }
                    onOpen={(file) => openFullView(paneId, repo, status.files, { file, side: "unstaged" })}
                    renderActions={(file) => (
                        <>
                            <ActButton label={`Open ${file.path}`} onClick={() => cmd.requestOpenFile(joinPath(repo, file.path))}>
                                <IconFile size={12} />
                            </ActButton>
                            <ActButton label="Discard changes" disabled={busy} onClick={() => void discard(file)}>
                                <IconDiscard size={12} />
                            </ActButton>
                            <ActButton
                                label={`Stage ${file.path}`}
                                disabled={busy}
                                onClick={() => void runIn(repo, `Stage ${file.path}`, () => git.stage(repo, file.path))}>
                                <IconPlus size={12} />
                            </ActButton>
                        </>
                    )}
                />
            )}
            {status.files.length === 0 && <p className="git-sc-clean">No changes.</p>}
        </div>
    );
}

function FileGroup({
    title,
    files,
    side,
    groupAction,
    onOpen,
    renderActions,
}: {
    title: string;
    files: readonly GitFile[];
    side: Side;
    groupAction: ReactNode;
    onOpen: (file: GitFile) => void;
    renderActions: (file: GitFile) => ReactNode;
}) {
    const [folded, setFolded] = useState(false);
    return (
        <div className="git-sc-group">
            <div className="git-sc-group-head" role="button" tabIndex={0} aria-expanded={!folded} onClick={() => setFolded(!folded)}>
                <IconChevron size={8} className="git-sc-chev" />
                <span>{title}</span>
                <span className="git-tb-grow" />
                <span className="git-sc-acts">{groupAction}</span>
                <span className="git-sc-count">{files.length}</span>
            </div>
            {!folded &&
                files.map((file) => {
                    const decoration = gitStatusDecoration(side === "staged" ? file.index : file.worktree);
                    const dir = dirname(file.path);
                    return (
                        <div
                            key={file.path}
                            role="button"
                            tabIndex={0}
                            aria-label={`${file.path}, ${side}`}
                            className="git-row git-sc-file"
                            onClick={() => onOpen(file)}
                            onKeyDown={(event) => {
                                if (event.key === "Enter") onOpen(file);
                            }}>
                            <FileIcon name={basename(file.path)} size={14} />
                            <span className="git-row-name">
                                {basename(file.path)}
                                {dir && <span className="git-row-dir">{dir}</span>}
                            </span>
                            <span className="git-row-actions">{renderActions(file)}</span>
                            {decoration && (
                                <span className={`git-status git-${decoration.cls}`} title={decoration.label}>
                                    {decoration.letter}
                                </span>
                            )}
                        </div>
                    );
                })}
        </div>
    );
}
