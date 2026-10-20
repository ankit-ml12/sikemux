import type { PluginStreamHandlers } from "../../plugin-api/backend";
import type {
    Annotation,
    Artifact,
    ChangedFile,
    Comment,
    DownloadProgress,
    Issue,
    IssuePage,
    JobLog,
    JobSummary,
    MergeMethod,
    NewPull,
    Notification,
    PendingApproval,
    Pull,
    PullCommit,
    RateLimit,
    Release,
    RepoListing,
    RepoRef,
    Resolved,
    Review,
    ReviewEvent,
    RunDetail,
    RunPage,
    RunQuery,
    RunTick,
    RunTiming,
    SavedArtifact,
    TimelineItem,
    Workflow,
    WorkflowFile,
    CodeHostApi,
    CommitAuthor,
    HostAccount,
    HostAccountEntry,
} from "../../plugin-api/codehost";
import { createCodeHostBackend } from "../../plugin-api/codehost";
import { GITHUB_PLUGIN } from "./kinds";

export type * from "../../plugin-api/codehost";
const { backend, call, image, watch } = createCodeHostBackend(GITHUB_PLUGIN.id);

export type TokenSource = "keychain" | "environment" | "ghCli";

export interface ActionsStatus {
    configured: boolean;
    /** Which signed-in account this is; null while a borrowed token stands in before anyone has signed in. */
    account: string | null;
    host: string;
    login: string;
    tokenSource: TokenSource | null;
    /** The variable an environment token was read from. */
    tokenVariable: string | null;
    scopes: string[];
    canWriteWorkflows: boolean;
    ok: boolean;
    authFailed: boolean;
    message: string | null;
}

interface DownloadTick extends DownloadProgress {
    saved: SavedArtifact | null;
}

/** Files can take longer than any single call may, so they arrive as a stream of progress that ends where the file was saved. */
function download(method: string, params: unknown, onProgress?: (progress: DownloadProgress) => void): Promise<SavedArtifact> {
    return new Promise((resolve, reject) => {
        let saved: SavedArtifact | null = null;
        backend.stream<DownloadTick>(method, params, {
            onItem: (tick) => {
                if (tick.saved) saved = tick.saved;
                else onProgress?.(tick);
            },
            onEnd: () => (saved ? resolve(saved) : reject(new Error("the download ended without saving anything"))),
            onError: reject,
        });
    });
}

export const actionsApi = {
    status: (account: string | null = null) => backend.call<ActionsStatus>("status", { account }),
    signIn: (host: string, token?: string) => backend.call<ActionsStatus>("signIn", { host, token }),
    signOut: (account: string | null) => backend.call<void>("signOut", { account }),

    resolveRemote: (url: string) => backend.call<Resolved>("resolveRemote", { url }),
    myRepos: (account: string | null, limit = 50) => call<RepoListing[]>("myRepos", { account, limit }),

    workflows: (repo: RepoRef) => call<Workflow[]>("workflows", repo),
    branches: (repo: RepoRef) => call<string[]>("branches", repo),
    runs: (query: RunQuery) => call<RunPage>("runs", query),
    run: (repo: RepoRef, runId: string) => call<RunDetail>("run", { ...repo, runId }),
    jobLog: (repo: RepoRef, jobId: string) => call<JobLog>("jobLog", { ...repo, jobId }),
    annotations: (repo: RepoRef, checkRunId: string) => call<Annotation[]>("annotations", { ...repo, checkRunId }),
    jobSummary: (repo: RepoRef, checkRunId: string) => call<JobSummary | null>("jobSummary", { ...repo, checkRunId }),
    runTiming: (repo: RepoRef, runId: string) => call<RunTiming>("runTiming", { ...repo, runId }),
    workflowFile: (repo: RepoRef, workflowId: string) => call<WorkflowFile>("workflowFile", { ...repo, workflowId }),
    workflowFileAt: (repo: RepoRef, path: string, gitRef: string) => call<WorkflowFile>("workflowFileAt", { ...repo, path, gitRef }),
    artifacts: (repo: RepoRef, runId: string) => call<Artifact[]>("artifacts", { ...repo, runId }),
    pendingApprovals: (repo: RepoRef, runId: string) => call<PendingApproval[]>("pendingApprovals", { ...repo, runId }),
    runAttempt: (repo: RepoRef, runId: string, attempt: number) => call<RunDetail>("runAttempt", { ...repo, runId, attempt }),

    pulls: (repo: RepoRef, state: string) => call<Pull[]>("pulls", { ...repo, state }),
    pullCounts: (repo: RepoRef) => call<{ open: number; closed: number }>("pullCounts", { ...repo }),
    pull: (repo: RepoRef, number: number) => call<Pull>("pull", { ...repo, number }),
    pullFiles: (repo: RepoRef, number: number) => call<ChangedFile[]>("pullFiles", { ...repo, number, fullPatches: true }),
    pullCommits: (repo: RepoRef, number: number) => call<PullCommit[]>("pullCommits", { ...repo, number }),
    timeline: (repo: RepoRef, number: number) => call<TimelineItem[]>("timeline", { ...repo, number }),
    commitAuthors: (repo: RepoRef, gitRef: string | null) => call<CommitAuthor[]>("commitAuthors", { ...repo, gitRef }),
    pullReviews: (repo: RepoRef, number: number) => call<Review[]>("pullReviews", { ...repo, number }),
    issues: (repo: RepoRef, state: string, page: number) => call<IssuePage>("issues", { ...repo, state, page }),
    issueCounts: (repo: RepoRef) => call<{ open: number; closed: number }>("issueCounts", { ...repo }),
    issue: (repo: RepoRef, number: number) => call<Issue>("issue", { ...repo, number }),
    comments: (repo: RepoRef, number: number) => call<Comment[]>("comments", { ...repo, number }),
    releases: (repo: RepoRef) => call<Release[]>("releases", repo),
    inbox: (account: string | null, all: boolean) => call<Notification[]>("inbox", { account, all }),
    image,

    /** `sha` is the head commit the person saw; GitHub refuses the merge if the branch has moved since. */
    mergePull: (repo: RepoRef, number: number, method: MergeMethod, sha: string) => call<void>("mergePull", { ...repo, number, method, sha }),
    createPull: (repo: RepoRef, pull: NewPull) => call<Pull>("createPull", { ...repo, ...pull }),
    setPullState: (repo: RepoRef, number: number, state: "open" | "closed") => call<void>("setPullState", { ...repo, number, state }),
    reviewPull: (repo: RepoRef, number: number, event: ReviewEvent, body: string) => call<void>("reviewPull", { ...repo, number, event, body }),
    createIssue: (repo: RepoRef, title: string, body: string) => call<Issue>("createIssue", { ...repo, title, body }),
    setIssueState: (repo: RepoRef, number: number, state: "open" | "closed") => call<void>("setIssueState", { ...repo, number, state }),
    addComment: (repo: RepoRef, number: number, body: string) => call<void>("addComment", { ...repo, number, body }),
    downloadAsset: (repo: RepoRef, assetId: number, name: string, onProgress?: (progress: DownloadProgress) => void) =>
        download("downloadAsset", { ...repo, assetId, fileName: name }, onProgress),
    markRead: (account: string | null, id: string) => call<void>("markRead", { account, id }),
    markAllRead: (account: string | null) => call<void>("markAllRead", { account }),

    dispatch: (repo: RepoRef, workflowId: string, gitRef: string, inputs: Record<string, string>) =>
        call<void>("dispatch", { ...repo, workflowId, gitRef, inputs }),
    rerun: (repo: RepoRef, runId: string, failedOnly: boolean, debug = false) => call<void>("rerun", { ...repo, runId, failedOnly, debug }),
    rerunJob: (repo: RepoRef, jobId: string, debug = false) => call<void>("rerunJob", { ...repo, jobId, debug }),
    cancel: (repo: RepoRef, runId: string) => call<void>("cancel", { ...repo, runId }),
    deleteRunLogs: (repo: RepoRef, runId: string) => call<void>("deleteRunLogs", { ...repo, runId }),
    deleteRun: (repo: RepoRef, runId: string) => call<void>("deleteRun", { ...repo, runId }),
    downloadArtifact: (repo: RepoRef, artifactId: string, name: string, onProgress?: (progress: DownloadProgress) => void) =>
        download("downloadArtifact", { ...repo, artifactId, fileName: name }, onProgress),
    reviewDeployment: (repo: RepoRef, runId: string, environmentIds: number[], state: "approved" | "rejected", comment = "") =>
        call<void>("reviewDeployment", { ...repo, runId, environmentIds, state, comment }),

    watch: (repo: RepoRef, runId: string, handlers: PluginStreamHandlers<RunTick>) => watch<RunTick>({ ...repo, runId }, handlers),
};

/** GitHub serves every avatar from one place, and a company's own GitHub serves its users' from itself. */
function avatarOf(login: string, host: string): string {
    return host === "github.com" ? `https://avatars.githubusercontent.com/${login}?s=64` : `https://${host}/${login}.png?size=64`;
}

/** Where the signed-in account lives, so a name alone can find its picture. */
let accountHost = "github.com";

export function avatarForLogin(login: string): string | null {
    return login.endsWith("[bot]") || login.includes("/") ? null : avatarOf(login, accountHost);
}

function accountOf(status: ActionsStatus): HostAccount {
    if (status.host) accountHost = status.host;
    return {
        id: status.account,
        ok: status.ok,
        login: status.login,
        avatarUrl: status.login ? avatarOf(status.login, status.host) : null,
        host: status.host,
        canWriteCi: status.canWriteWorkflows,
        warning: status.ok && !status.canWriteWorkflows ? "This token cannot start or re-run workflows. It is missing the workflow scope." : null,
    };
}

/** GitHub as the git pane reads any code host. */
interface ListedAccount {
    id: string;
    host: string;
    login: string;
    isDefault: boolean;
}

function entryOf(account: ListedAccount): HostAccountEntry {
    return {
        id: account.id,
        login: account.login,
        detail: account.host === "github.com" ? null : account.host,
        avatarUrl: avatarOf(account.login, account.host),
        isDefault: account.isDefault,
    };
}

export const githubHostApi: CodeHostApi = {
    ...actionsApi,
    rateLimit: (account: string | null) => backend.call<RateLimit>("rateLimit", { account }),
    status: (account: string | null) => actionsApi.status(account).then(accountOf),
    accounts: () => backend.call<ListedAccount[]>("accounts").then((listed) => listed.map(entryOf)),
    setDefaultAccount: (account: string) => backend.call<void>("setDefaultAccount", { id: account }),
    accountFor: (repo: RepoRef) => call<string | null>("accountFor", repo),
};

const NOREPLY = /^(\d+)\+[^@]+@users\.noreply\.github\.com$/iu;

/** GitHub's private commit emails carry the account's number, which is all its avatar address needs. */
export function avatarForEmail(email: string): string | null {
    const found = NOREPLY.exec(email.trim());
    return found ? `https://avatars.githubusercontent.com/u/${found[1]}?s=64` : null;
}
