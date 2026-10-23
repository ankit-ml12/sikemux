import { describe, expect, it } from "vitest";
import type { GitFile, GitStatus } from "../api/git";
import { repoAction, reposIn, sectionOpen, sourceControlRoots } from "./sourceControlRepos";

const file = (path: string, index: string, worktree: string): GitFile => ({ path, index, worktree });
const status = (patch: Partial<GitStatus> = {}): GitStatus => ({ branch: "main", upstream: "origin/main", ahead: 0, behind: 0, files: [], ...patch });

describe("which repositories the list shows", () => {
    const sessions = {
        a: { kind: "project", cwd: "/work/app" },
        b: { kind: "ssh", cwd: "/remote" },
        c: { kind: "project", cwd: "/work/site" },
        d: { kind: "project", cwd: "/work/app" },
    } as never;

    it("looks in this project alone, or in every open project with this one first", () => {
        expect(sourceControlRoots({ sessions, sessionOrder: ["a", "b", "c", "d"] }, "/work/site", "project")).toEqual(["/work/site"]);
        expect(sourceControlRoots({ sessions, sessionOrder: ["a", "b", "c", "d"] }, "/work/site", "all")).toEqual(["/work/site", "/work/app"]);
    });

    it("lists the project folder's own repository first, then those inside it", () => {
        const inside = [{ path: "/work/app/api", name: "api", branch: "dev", ahead: 1, behind: 0, changes: 2 }];
        expect(reposIn("/work/app", status({ files: [file("a.ts", " ", "M")] }), inside)).toEqual([
            { path: "/work/app", name: "app", branch: "main", ahead: 0, behind: 0, changes: 1 },
            { path: "/work/app/api", name: "api", branch: "dev", ahead: 1, behind: 0, changes: 2 },
        ]);
        expect(reposIn("/work/docs", null, inside).map((repo) => repo.name)).toEqual(["api"]);
    });

    it("opens a section with changes unless the person folded it", () => {
        const dirty = { path: "/r", name: "r", branch: "main", ahead: 0, behind: 0, changes: 3 };
        expect(sectionOpen({}, dirty)).toBe(true);
        expect(sectionOpen({}, { ...dirty, changes: 0 })).toBe(false);
        expect(sectionOpen({ "/r": false }, dirty)).toBe(false);
        expect(sectionOpen({ "/r": true }, { ...dirty, changes: 0 })).toBe(true);
    });
});

describe("what a section's main button does", () => {
    it("commits what is staged, once there is a message", () => {
        const staged = status({ files: [file("a.ts", "M", " "), file("b.ts", " ", "M")] });
        expect(repoAction(staged, "")).toEqual({ kind: "commit", label: "Commit", ready: false });
        expect(repoAction(staged, "fix: a")).toEqual({ kind: "commit", label: "Commit", ready: true });
    });

    it("commits every change when nothing is staged", () => {
        expect(repoAction(status({ files: [file("new.ts", "?", "?")] }), "add")).toEqual({ kind: "commitAll", label: "Commit All", ready: true });
    });

    it("publishes a branch the remote lacks, then pushes or pulls what differs", () => {
        expect(repoAction(status({ upstream: null, branch: "feat/x" }), "")).toMatchObject({ kind: "publish", label: "Publish Branch" });
        expect(repoAction(status({ ahead: 2 }), "")).toMatchObject({ kind: "push", label: "Push 2↑" });
        expect(repoAction(status({ behind: 3 }), "")).toMatchObject({ kind: "pull", label: "Pull 3↓" });
        expect(repoAction(status(), "")).toEqual({ kind: "none", label: "Commit", ready: false });
    });
});
