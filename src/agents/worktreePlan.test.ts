import { describe, expect, it } from "vitest";
import { cwdOutsideWorktree } from "./worktreePlan";

const worktree = { repo: "/code/sikemux", path: "/code/sikemux.worktrees/fix-pty", branch: "sikemux/fix-pty", base: "main", startSha: "abc" };

describe("worktree targets", () => {
    it("keep the folder the project had open inside the checkout", () => {
        expect(cwdOutsideWorktree(worktree, "/code/sikemux.worktrees/fix-pty/src")).toBe("/code/sikemux/src");
        expect(cwdOutsideWorktree(worktree, "/code/sikemux.worktrees/fix-pty")).toBe("/code/sikemux");
    });
});
