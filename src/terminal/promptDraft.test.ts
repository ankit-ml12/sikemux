import { describe, expect, it } from "vitest";
import { promptDraft } from "./promptDraft";

describe("what is typed in an agent's TUI", () => {
    it("reads Claude's input box, its continuation lines and not the transcript above", () => {
        const screen = [
            "❯ fix the login flake",
            "⏺ Done. The test now waits for the page.",
            "",
            "────────────────────────────────────────",
            "❯ now check the logout path too",
            "  and the session timeout",
            "────────────────────────────────────────",
            "  ⏵⏵ bypass permissions on (shift+tab to cycle)",
        ];
        expect(promptDraft(screen)).toBe("now check the logout path too\nand the session timeout");
    });

    it("reads Codex's input line", () => {
        expect(promptDraft(["• Ran the tests", "", "› add a test for the empty case", "", "  ⏎ send   ⌃J newline"])).toBe(
            "add a test for the empty case",
        );
    });

    it("reads an empty box as nothing, and a screen with no box as nothing", () => {
        expect(promptDraft(["────────────", "❯", "────────────"])).toBe("");
        expect(promptDraft(["$ ls", "README.md  src"])).toBe("");
    });

    it("strips the box's side borders", () => {
        expect(promptDraft(["╭──────────────────╮", "│ › keep this     │", "╰──────────────────╯"])).toBe("keep this");
    });
});
