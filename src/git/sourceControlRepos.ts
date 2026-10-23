import { hasUnstaged, isStaged, type DiscoveredRepo, type GitStatus } from "../api/git";
import { basename } from "../lib/paths";
import type { StoreState } from "../state/store";

/** A repository as its section's header shows it, before its files are read. */
export interface RepoSummary {
    path: string;
    name: string;
    branch: string;
    ahead: number;
    behind: number;
    changes: number;
}

/** The folders the list looks in: this project's, or every open project's with this one first. */
export function sourceControlRoots(state: Pick<StoreState, "sessions" | "sessionOrder">, cwd: string, scope: "project" | "all"): string[] {
    if (scope === "project") return cwd ? [cwd] : [];
    const roots = state.sessionOrder.flatMap((id) => {
        const session = state.sessions[id];
        return session?.kind === "project" && session.cwd ? [session.cwd] : [];
    });
    return [...new Set([cwd, ...roots].filter(Boolean))];
}

/** The project folder's own repository, when it is one, then those directly inside it. */
export function reposIn(root: string, rootStatus: GitStatus | null, discovered: readonly DiscoveredRepo[]): RepoSummary[] {
    const own: RepoSummary[] = rootStatus
        ? [
              {
                  path: root,
                  name: basename(root),
                  branch: rootStatus.branch,
                  ahead: rootStatus.ahead,
                  behind: rootStatus.behind,
                  changes: rootStatus.files.length,
              },
          ]
        : [];
    const inside = discovered
        .filter((repo) => repo.path !== root)
        .map(({ path, name, branch, ahead, behind, changes }) => ({ path, name, branch, ahead, behind, changes }));
    return [...own, ...inside];
}

/** Whether a section is open: as the person left it, or open while it has changes. */
export function sectionOpen(folds: Readonly<Record<string, boolean>>, repo: RepoSummary): boolean {
    return folds[repo.path] ?? repo.changes > 0;
}

export type RepoAction =
    | { kind: "commit"; label: "Commit"; ready: boolean }
    | { kind: "commitAll"; label: "Commit All"; ready: boolean }
    | { kind: "publish"; label: "Publish Branch"; ready: true }
    | { kind: "push"; label: string; ready: true }
    | { kind: "pull"; label: string; ready: true }
    | { kind: "none"; label: "Commit"; ready: false };

/**
 * What the section's main button does, as a code editor's source control offers it: commit what is
 * staged, or every change when nothing is; then publish a branch the remote lacks, or push and pull.
 */
export function repoAction(status: GitStatus, message: string): RepoAction {
    const written = message.trim().length > 0;
    if (status.files.some(isStaged)) return { kind: "commit", label: "Commit", ready: written };
    if (status.files.some(hasUnstaged)) return { kind: "commitAll", label: "Commit All", ready: written };
    if (status.branch && !status.upstream) return { kind: "publish", label: "Publish Branch", ready: true };
    if (status.ahead > 0) return { kind: "push", label: `Push ${status.ahead}↑`, ready: true };
    if (status.behind > 0) return { kind: "pull", label: `Pull ${status.behind}↓`, ready: true };
    return { kind: "none", label: "Commit", ready: false };
}
