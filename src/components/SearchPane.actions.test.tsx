import { act, cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { SearchFile, SearchResults } from "../api/search";

const project = vi.fn();
const replace = vi.fn();
vi.mock("../api/search", () => ({
    searchApi: {
        project: (...args: unknown[]) => project(...args),
        cancel: vi.fn(async () => {}),
        replace: (...args: unknown[]) => replace(...args),
        readFileWindow: vi.fn(async () => ({ doc: "", start_line: 1, total_lines: 0, clipped_head: false, clipped_tail: false })),
    },
}));
vi.mock("../state/toast", async (original) => ({ ...(await original<typeof import("../state/toast")>()), notify: vi.fn() }));

import { SearchPane } from "./SearchPane";
import * as cmd from "../state/commands";
import { getState } from "../state/store";

const FILES: SearchFile[] = [
    {
        path: "src/a.ts",
        matches: [
            { line: 3, text: "const foo = 1;", ranges: [{ start: 6, end: 9 }] },
            { line: 9, text: "foo();", ranges: [{ start: 0, end: 3 }] },
        ],
    },
    { path: "src/b.ts", matches: [{ line: 1, text: "foo", ranges: [{ start: 0, end: 3 }] }] },
];
const DONE: SearchResults = { files: [], file_count: 2, match_count: 3, truncated: false, elapsed_ms: 4 };

let session = 0;
async function renderWithResults(options: { replaceOpen?: boolean } = {}) {
    const id = `search-actions-${++session}`;
    project.mockImplementation((_r: string, _q: string, _o: unknown, onFile: (f: SearchFile) => void) => {
        FILES.forEach(onFile);
        return Promise.resolve(DONE);
    });
    cmd.setGlobalSearchQuery(id, "foo");
    cmd.setGlobalSearchReplace(id, "bar");
    if (options.replaceOpen) cmd.toggleGlobalSearchReplaceOpen(id);
    render(<SearchPane sessionId={id} cwd="/repo" active visible compact />);
    await act(async () => {
        vi.advanceTimersByTime(400);
        await Promise.resolve();
    });
    await act(async () => {
        vi.advanceTimersByTime(100);
    });
    return id;
}

describe("search actions", () => {
    beforeEach(() => {
        vi.useFakeTimers();
        // The result list is virtualised; give it room so rows render.
        vi.spyOn(HTMLElement.prototype, "getBoundingClientRect").mockReturnValue({
            width: 400,
            height: 800,
            top: 0,
            left: 0,
            right: 400,
            bottom: 800,
        } as DOMRect);
        vi.spyOn(HTMLElement.prototype, "offsetHeight", "get").mockReturnValue(800);
        vi.spyOn(HTMLElement.prototype, "offsetWidth", "get").mockReturnValue(400);
    });
    afterEach(() => {
        cleanup();
        project.mockReset();
        replace.mockReset();
        vi.restoreAllMocks();
        vi.useRealTimers();
    });

    it("dismisses one line and then a whole file", async () => {
        await renderWithResults();
        expect(screen.getByRole("button", { name: "Dismiss on line 3" })).toBeInTheDocument();

        fireEvent.click(screen.getByRole("button", { name: "Dismiss on line 3" }));
        expect(screen.queryByRole("button", { name: "Dismiss on line 3" })).not.toBeInTheDocument();
        expect(screen.getByRole("button", { name: "Dismiss on line 9" })).toBeInTheDocument();

        fireEvent.click(screen.getByRole("button", { name: "Dismiss in b.ts" }));
        expect(screen.queryByRole("button", { name: "Dismiss in b.ts" })).not.toBeInTheDocument();
    });

    it("offers to replace a row only while the replace box is open", async () => {
        await renderWithResults();
        expect(screen.queryByRole("button", { name: "Replace on line 3" })).not.toBeInTheDocument();
    });

    it("replaces one line in place and drops that row", async () => {
        replace.mockResolvedValue({
            files: [{ path: "src/a.ts", match_count: 1 }],
            file_count: 1,
            match_count: 1,
            errors: [],
            dry_run: false,
            elapsed_ms: 1,
        });
        await renderWithResults({ replaceOpen: true });

        await act(async () => {
            fireEvent.click(screen.getByRole("button", { name: "Replace on line 3" }));
        });

        expect(replace).toHaveBeenCalledWith("/repo", "foo", "bar", expect.any(Object), false, { path: "src/a.ts", line: 3 });
        expect(screen.queryByRole("button", { name: "Replace on line 3" })).not.toBeInTheDocument();
        expect(screen.getByRole("button", { name: "Replace on line 9" })).toBeInTheDocument();
    });

    it("replaces a whole file in place", async () => {
        replace.mockResolvedValue({
            files: [{ path: "src/b.ts", match_count: 1 }],
            file_count: 1,
            match_count: 1,
            errors: [],
            dry_run: false,
            elapsed_ms: 1,
        });
        await renderWithResults({ replaceOpen: true });

        await act(async () => {
            fireEvent.click(screen.getByRole("button", { name: "Replace in b.ts" }));
        });

        expect(replace).toHaveBeenCalledWith("/repo", "foo", "bar", expect.any(Object), false, { path: "src/b.ts" });
        expect(screen.queryByRole("button", { name: "Replace in b.ts" })).not.toBeInTheDocument();
    });

    it("toggles preserve case from the replace row", async () => {
        const id = await renderWithResults({ replaceOpen: true });

        fireEvent.click(screen.getByRole("button", { name: "Preserve case" }));

        expect(getState().globalSearchBySession[id].options.preserveCase).toBe(true);
    });

    it("collapses and expands every file from the toolbar", async () => {
        const id = await renderWithResults();

        fireEvent.click(screen.getByRole("button", { name: "Collapse all" }));
        expect(getState().globalSearchBySession[id].collapsed).toEqual({ "src/a.ts": true, "src/b.ts": true });

        fireEvent.click(screen.getByRole("button", { name: "Expand all" }));
        expect(getState().globalSearchBySession[id].collapsed).toEqual({});
    });

    it("clears the boxes from the toolbar", async () => {
        const id = await renderWithResults();

        fireEvent.click(screen.getByRole("button", { name: "Clear search results" }));

        expect(getState().globalSearchBySession[id].query).toBe("");
        expect(getState().globalSearchBySession[id].replace).toBe("");
    });

    it("previews a replace-all first, then asks before writing", async () => {
        replace.mockResolvedValue({
            files: [{ path: "src/a.ts", match_count: 2 }],
            file_count: 1,
            match_count: 2,
            errors: [],
            dry_run: true,
            elapsed_ms: 1,
        });
        await renderWithResults({ replaceOpen: true });

        await act(async () => {
            fireEvent.click(screen.getByRole("button", { name: "Replace all (preview first)" }));
        });

        expect(replace).toHaveBeenLastCalledWith("/repo", "foo", "bar", expect.any(Object), true);
        expect(screen.getByText(/Replace 2 matches in 1 file\?/)).toBeInTheDocument();
    });
});
