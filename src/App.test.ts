import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { activeTaskControls, gitChangeInvalidates, onGitChanged } from "./App";
import { installIpcTransportForTests, MemoryIpcTransport, resetIpcTransportForTests } from "./api/transport";
import { subscribe } from "./state/bus";
import type { ProjectTask, TaskRun } from "./tasks/coreTasks";

let transport: MemoryIpcTransport;

beforeEach(() => {
    resetIpcTransportForTests();
    transport = new MemoryIpcTransport();
    installIpcTransportForTests(transport);
});

afterEach(() => resetIpcTransportForTests());

function task(id: string): ProjectTask {
    return Object.freeze({ id, label: id, command: `run ${id}`, cwd: ".", source: "project" as const });
}

function run(taskId: string): TaskRun {
    return Object.freeze({
        executionId: "run-1",
        taskId,
        project: "/repo",
        status: "running" as const,
        ptyId: 7,
        agentId: null,
        terminalKey: "task",
    });
}

describe("active task command reachability", () => {
    it("keeps Stop reachable for a run after its task is gone from the project", () => {
        expect(activeTaskControls(run("watch"), [])).toEqual({ canStop: true, restartTaskId: null });
    });

    it("allows restart only through the current project inventory", () => {
        expect(activeTaskControls(run("watch"), [task("watch")])).toEqual({ canStop: true, restartTaskId: "watch" });
        expect(activeTaskControls(run("watch"), [task("build")])).toEqual({ canStop: true, restartTaskId: null });
    });

    it("offers neither with nothing running", () => {
        expect(activeTaskControls(null, [task("watch")])).toEqual({ canStop: false, restartTaskId: null });
    });
});

describe("Git change IPC subscription", () => {
    it("keeps file changes to status resources and reserves broad Git refreshes for metadata signals", () => {
        expect(gitChangeInvalidates("files.list", ["/repo"], "/repo", ["src/file.ts"])).toBe(true);
        expect(gitChangeInvalidates("git.overview", ["/repo"], "/repo", ["src/file.ts"])).toBe(true);
        expect(gitChangeInvalidates("git.status", ["/repo"], "/repo", ["src/file.ts"])).toBe(true);
        expect(gitChangeInvalidates("git.stashes", ["/repo"], "/repo", ["src/file.ts"])).toBe(false);
        expect(gitChangeInvalidates("git.remotes", ["/repo"], "/repo", null)).toBe(true);
        expect(gitChangeInvalidates("git.status", ["/other"], "/repo", null)).toBe(false);
    });

    it("refreshes the repositories inside a project folder as their files change", () => {
        const folder = "/work/internal";
        // The list of repositories, with their change counts, is kept for the folder.
        expect(gitChangeInvalidates("git.discoveredRepos", [folder], folder, ["web/src/app.ts"])).toBe(true);
        expect(gitChangeInvalidates("git.discoveredRepos", [folder], folder, null)).toBe(true);
        // A repository opened from that list is kept under its own path.
        expect(gitChangeInvalidates("git.overview", [`${folder}/web`], folder, ["web/src/app.ts"])).toBe(true);
        expect(gitChangeInvalidates("git.status", [`${folder}/web`], folder, ["web/src/app.ts"])).toBe(true);
        expect(gitChangeInvalidates("git.overview", [`${folder}/crm`], folder, ["web/src/app.ts"])).toBe(false);
        expect(gitChangeInvalidates("git.stashes", [`${folder}/web`], folder, ["web/src/app.ts"])).toBe(false);
        // A commit or a stage names no files, so everything Git knows about each repository refreshes.
        expect(gitChangeInvalidates("git.stashes", [`${folder}/web`], folder, null)).toBe(true);
        expect(gitChangeInvalidates("files.list", [`${folder}/web`], folder, null)).toBe(false);
        expect(gitChangeInvalidates("git.overview", ["/work/internal-old/web"], folder, null)).toBe(false);
    });

    it("tells the page what moved, and everything when git's own state did", () => {
        const changed = vi.fn();
        const unsubscribeBus = subscribe("fs-changed", changed);

        onGitChanged({ topic: "git", project: "/repo", paths: null });
        expect(changed).toHaveBeenCalledWith({ type: "fs-changed", repo: "/repo" });
        onGitChanged({ topic: "git", project: "/repo", paths: ["src/file.ts"] });
        expect(changed).toHaveBeenLastCalledWith({ type: "fs-changed", repo: "/repo", paths: ["src/file.ts"] });
        onGitChanged({ topic: "git", project: "/repo", paths: [".git", "src/file.ts"] });
        expect(changed).toHaveBeenLastCalledWith({ type: "fs-changed", repo: "/repo" });

        unsubscribeBus();
    });
});
