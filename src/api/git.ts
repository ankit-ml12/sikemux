import { coreCall } from "./core";
import { subscribe } from "../state/bus";

export interface GitFile {
    path: string;
    index: string;
    worktree: string;
}

export interface GitStatus {
    branch: string;
    upstream: string | null;
    ahead: number;
    behind: number;
    files: GitFile[];
}

export interface DiscoveredRepo {
    path: string;
    name: string;
    branch: string;
    ahead: number;
    behind: number;
    changes: number;
}

export interface GitBranch {
    name: string;
    current: boolean;
    upstream: string | null;
}

export interface GitWorktree {
    path: string;
    head: string | null;
    branch: string | null;
    reference: string | null;
    detached: boolean;
    locked: boolean;
    lock_reason: string | null;
    prunable: boolean;
    prune_reason: string | null;
    bare: boolean;
    current: boolean;
    is_main: boolean;
}

export interface CreateGitWorktreeOptions {
    path: string;
    branch: string;
    createBranch: boolean;
    startPoint?: string | null;
}

export interface GitCommit {
    hash: string;
    full_hash: string;
    parents: string[];
    author: string;
    author_email: string;
    date: string;
    subject: string;
    refs: string[];
    unpushed: boolean;
    /** Its message ends in an agent footer, which only the core can check. */
    footer?: boolean;
}

/** An agent a commit's signed footer names, as this host knows it now. */
export interface CommitOwner {
    name: string;
    cli: string;
    cliVersion?: string;
    model?: string;
    /** The version of Sikemux the agent ran in. */
    sikemux?: string;
    /** The agent the footer names. */
    chat: string;
    title?: string;
    /** The agent that has its session open now. */
    open?: string;
    sessionId?: string;
    /** The project its session ran in. */
    project?: string;
    /** The lines it wrote, first and last of each run by path, as the commit numbers them, when the commit's note says so because it holds others' lines too. */
    lines?: Record<string, [number, number][]>;
}

export interface GitCompare {
    merge_base: string;
    files: { path: string; status: "A" | "M" | "D" | "R" }[];
    commits: GitCommit[];
}

export interface GitOverview {
    status: GitStatus;
    branches: GitBranch[];
    log: GitCommit[];
}

export interface GitStash {
    index: number;
    sha: string;
    refname: string;
    branch: string;
    message: string;
}

export type DiscardMode = "unstaged" | "staged" | "all";
export type StashMode = "all" | "staged" | "unstaged";

export interface GitRemote {
    name: string;
    url: string;
}

export interface GitRemoteBranch {
    name: string;
    full_ref: string;
    is_head_pointer: boolean;
    tracked_by: string | null;
    subject: string | null;
}

export interface BlameCommit {
    sha: string;
    short: string;
    author: string;
    author_email: string;
    time: string;
    timestamp: number;
    summary: string;
    uncommitted: boolean;
}

/** A unified diff row: added, deleted or unchanged text with the line number it shows, or a count of unchanged lines left out. */
export type DiffRow = [kind: 0 | 1 | 2, line: number, text: string] | [kind: 3, hidden: number, text: ""];

export const DIFF_ROW = { context: 0, added: 1, deleted: 2, hidden: 3 } as const;

/** An agent that wrote some of a checkout's uncommitted lines, or the agents at work when no one of them could have, named "Otter or Briar". */
export interface ChangeAuthor {
    agentId: string;
    name: string;
    cli: string;
    model?: string;
    /** The chat run of its latest edit. */
    feed: string;
    /** The agents it stands for, when several could have made the change. */
    shared?: string[];
}

/** `count` lines from `start` that `author` wrote last; `over` is the agent whose lines it rewrote there. */
export interface OwnedLines {
    start: number;
    count: number;
    author: number;
    over?: number;
}

/** The changed lines of a file agents wrote, numbered as in the working tree, or in the index for staged lines. */
export interface FileOwners {
    path: string;
    lines: OwnedLines[];
    /** Lines `author` took out just before line `at`. */
    removed: { at: number; author: number }[];
}

/** Who wrote a checkout's uncommitted lines, as the core reads it from agents' edits. Lines no file lists have no known author. */
export interface ChangeOwners {
    authors: ChangeAuthor[];
    files: FileOwners[];
}

/** Compact per-file blame: unique `commits` + a per-line index into them. */
export interface GitBlame {
    commits: BlameCommit[];
    lines: number[];
}

const fileAtCache = new Map<string, string>();
const fileAtInflight = new Map<string, Promise<string>>();
const FILE_AT_MAX_ENTRIES = 200;
const FILE_AT_MAX_CHARS = 16 * 1024 * 1024;
let fileAtChars = 0;
let fileAtGeneration = 0;
/** A revision that names a commit by hash always reads back the same bytes; HEAD, a branch and the index do not. */
const SHA_RE = /^[0-9a-f]{7,}([~^][0-9]*)*$/i;

/** A commit named by its hash, whose contents can never change. */
export function isFixedRevision(rev: string): boolean {
    return SHA_RE.test(rev);
}

function cacheKey(repo: string, rev: string, path: string) {
    return `${repo}\0${rev}\0${path}`;
}

function revisionOf(key: string): string {
    return key.split("\0")[1] ?? "";
}

function rememberFileAt(key: string, content: string): void {
    if (typeof content !== "string") return;
    const previous = fileAtCache.get(key);
    if (previous !== undefined) fileAtChars -= previous.length;
    fileAtCache.delete(key);
    fileAtCache.set(key, content);
    fileAtChars += content.length;
    for (const oldest of fileAtCache.keys()) {
        if (fileAtCache.size <= FILE_AT_MAX_ENTRIES && fileAtChars <= FILE_AT_MAX_CHARS) break;
        fileAtChars -= fileAtCache.get(oldest)!.length;
        fileAtCache.delete(oldest);
    }
}

export function forgetMovingRevisions(repo?: string): void {
    fileAtGeneration += 1;
    const prefix = repo ? `${repo}\0` : null;
    const matches = (key: string) => (!prefix || key.startsWith(prefix)) && !SHA_RE.test(revisionOf(key));
    for (const [key, content] of fileAtCache) {
        if (!matches(key)) continue;
        fileAtCache.delete(key);
        fileAtChars -= content.length;
    }
    for (const key of [...fileAtInflight.keys()]) if (matches(key)) fileAtInflight.delete(key);
}

subscribe("fs-changed", (event) => forgetMovingRevisions(event.repo || undefined));
subscribe("git-refresh", (event) => forgetMovingRevisions(event.repo || undefined));

const call = <T>(method: string, params: Record<string, unknown>) => coreCall<T>(`git.${method}`, params);

export const git = {
    status: (repo: string) => call<GitStatus>("status", { repo }),
    discoverRepos: (root: string) => call<DiscoveredRepo[]>("discoverRepos", { root }),
    overview: (repo: string) => call<GitOverview>("overview", { repo }),
    diff: (repo: string, path: string, staged: boolean) => call<string>("diff", { repo, path, staged }),
    stage: (repo: string, path: string) => call<void>("stage", { repo, path }),
    unstage: (repo: string, path: string) => call<void>("unstage", { repo, path }),
    stagePaths: (repo: string, paths: string[]) => call<void>("stagePaths", { repo, paths }),
    stageAll: (repo: string) => call<void>("stageAll", { repo }),
    unstageAll: (repo: string) => call<void>("unstageAll", { repo }),
    branches: (repo: string) => call<GitBranch[]>("branches", { repo }),
    worktrees: (repo: string) => call<GitWorktree[]>("worktrees", { repo }),
    worktreeCreate: (repo: string, options: CreateGitWorktreeOptions) =>
        call<GitWorktree>("worktreeCreate", {
            repo,
            path: options.path,
            branch: options.branch,
            createBranch: options.createBranch,
            startPoint: options.startPoint ?? null,
        }),
    worktreeRemove: (repo: string, path: string, force = false) => call<GitWorktree>("worktreeRemove", { repo, path, force }),
    checkout: (repo: string, branch: string) => call<void>("checkout", { repo, branch }),
    checkoutSmart: (repo: string, branch: string) => call<string>("checkoutSmart", { repo, branch }),
    branchCreate: (repo: string, name: string, startPoint?: string) => call<void>("branchCreate", { repo, name, startPoint: startPoint ?? null }),
    branchDelete: (repo: string, name: string, force: boolean) => call<void>("branchDelete", { repo, name, force }),
    branchRename: (repo: string, oldName: string, newName: string) => call<void>("branchRename", { repo, oldName, newName }),
    merge: (repo: string, branch: string) => call<string>("merge", { repo, branch }),
    mergeSquash: (repo: string, branch: string) => call<string>("mergeSquash", { repo, branch }),
    reset: (repo: string, rev: string, mode: "soft" | "mixed" | "hard") => call<void>("reset", { repo, rev, mode }),
    revert: (repo: string, rev: string) => call<void>("revert", { repo, rev }),
    log: (repo: string) => call<GitCommit[]>("log", { repo }),
    show: (repo: string, rev: string) => call<string>("show", { repo, rev }),
    fileAt: (repo: string, rev: string, path: string): Promise<string> => {
        const key = cacheKey(repo, rev, path);
        const hit = fileAtCache.get(key);
        if (hit !== undefined) {
            fileAtCache.delete(key);
            fileAtCache.set(key, hit);
            return Promise.resolve(hit);
        }
        const pending = fileAtInflight.get(key);
        if (pending) return pending;

        const immutable = SHA_RE.test(rev);
        const generation = fileAtGeneration;
        const request = call<string>("fileAt", { repo, rev, path }).then(
            (content) => {
                if (fileAtInflight.get(key) === request) fileAtInflight.delete(key);
                if (immutable || generation === fileAtGeneration) rememberFileAt(key, content);
                return content;
            },
            (error: unknown) => {
                if (fileAtInflight.get(key) === request) fileAtInflight.delete(key);
                throw error;
            },
        );
        fileAtInflight.set(key, request);
        return request;
    },
    /** Without `headRev` the diff runs to the working tree. `full` keeps every unchanged line. */
    fileDiff: (repo: string, path: string, baseRev: string, headRev: string | null, full: boolean) =>
        call<DiffRow[]>("fileDiff", { repo, path, baseRev, headRev, full }),
    commitFiles: (repo: string, rev: string) => call<string[]>("commitFiles", { repo, rev }),
    compare: (repo: string, base: string, head: string) => call<GitCompare>("compare", { repo, base, head }),
    blame: (repo: string, path: string, contents?: string | null) => call<GitBlame>("blame", { repo, path, contents: contents ?? null }),
    /** Commits what is staged; the core's footer names the agents whose lines it holds. */
    commit: (repo: string, message: string) => coreCall<string>("commitStaged", { dir: repo, message }),
    changeOwners: (repo: string, staged: boolean) => coreCall<ChangeOwners>("changeOwners", { dir: repo, staged }),
    commitOwners: (repo: string, commits: string[]) => coreCall<Record<string, CommitOwner[]>>("commitOwners", { dir: repo, commits }),
    push: (repo: string) => call<string>("push", { repo }),
    pull: (repo: string) => call<string>("pull", { repo }),
    aiMessage: (repo: string, provider: string, model: string) => call<string>("aiMessage", { repo, provider, model }),
    pullRequestUrl: (repo: string) => call<string>("pullRequestUrl", { repo }),

    discardFiles: (repo: string, paths: string[], mode: DiscardMode) => call<void>("discardFiles", { repo, paths, mode }),

    stashList: (repo: string) => call<GitStash[]>("stashList", { repo }),
    stashPush: (repo: string, mode: StashMode, message?: string | null) => call<void>("stashPush", { repo, mode, message: message ?? null }),
    stashApply: (repo: string, refname: string, sha: string) => call<void>("stashApply", { repo, refname, sha }),
    stashPop: (repo: string, refname: string, sha: string) => call<void>("stashPop", { repo, refname, sha }),
    stashDrop: (repo: string, refname: string, sha: string) => call<void>("stashDrop", { repo, refname, sha }),
    stashBranch: (repo: string, refname: string, sha: string, name: string) => call<void>("stashBranch", { repo, refname, sha, name }),
    stashRename: (repo: string, refname: string, sha: string, newMessage: string) => call<void>("stashRename", { repo, refname, sha, newMessage }),

    remotes: (repo: string) => call<GitRemote[]>("remotes", { repo }),
    remoteAdd: (repo: string, name: string, url: string) => call<void>("remoteAdd", { repo, name, url }),
    remoteRemove: (repo: string, name: string) => call<void>("remoteRemove", { repo, name }),
    remoteRename: (repo: string, oldName: string, newName: string) => call<void>("remoteRename", { repo, oldName, newName }),
    remoteSetUrl: (repo: string, name: string, url: string) => call<void>("remoteSetUrl", { repo, name, url }),
    fetch: (repo: string, remote?: string | null) => call<string>("fetch", { repo, remote: remote ?? null }),
    /** One ref from a remote into a local branch, such as a pull request that lives only on the remote it was opened on. */
    fetchRef: (repo: string, remote: string, source: string, branch: string) => call<string>("fetchRef", { repo, remote, source, branch }),

    remoteBranches: (repo: string, remote: string) => call<GitRemoteBranch[]>("remoteBranches", { repo, remote }),
    checkoutRemoteBranch: (repo: string, remote: string, branch: string, localName?: string | null) =>
        call<void>("checkoutRemoteBranch", { repo, remote, branch, localName: localName ?? null }),
    deleteRemoteBranch: (repo: string, remote: string, branch: string) => call<void>("deleteRemoteBranch", { repo, remote, branch }),
    setUpstream: (repo: string, branch: string, upstream: string | null) => call<void>("setUpstream", { repo, branch, upstream }),
};

export const isStaged = (f: GitFile) => f.index !== " " && f.index !== "?";
export const hasUnstaged = (f: GitFile) => f.worktree !== " ";
