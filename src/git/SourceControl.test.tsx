import { cleanup, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { git } from "../api/git";
import { GitPane } from "./GitPane";
import { gitDiscoveredReposR, gitOverviewR, gitStatusR } from "../state/resources.defs";
import { getState, setState } from "../state/store";
import { useGitWorkbench } from "../state/gitWorkbench";
import { DEFAULT_GIT_VIEW } from "../state/types";

const resources = vi.hoisted(() => ({
    missing: { status: "error", data: null, error: "could not find repository at '/container'", refresh: vi.fn() },
    discovered: {
        status: "ok",
        data: [
            { path: "/container/docs", name: "docs", branch: "main", ahead: 0, behind: 0, changes: 2 },
            { path: "/container/web", name: "web", branch: "feat/x", ahead: 1, behind: 0, changes: 0 },
        ],
        refresh: vi.fn(),
    },
    docs: {
        status: "ok",
        data: {
            branch: "main",
            upstream: "origin/main",
            ahead: 0,
            behind: 0,
            files: [
                { path: "notes/plan.md", index: "M", worktree: " " },
                { path: "README.md", index: " ", worktree: "M" },
            ],
        },
        refresh: vi.fn(),
    },
    empty: { status: "ok", data: [], refresh: vi.fn() },
    listing: vi.fn(),
}));

vi.mock("../state/resources", async (original) => ({
    ...(await original<typeof import("../state/resources")>()),
    useCachedResourceEnabled: (enabled: boolean, definition: unknown, ...args: unknown[]) => {
        if (definition === gitOverviewR) return args[0] === "/container" ? resources.missing : resources.empty;
        if (definition === gitStatusR) return args[0] === "/container/docs" ? resources.docs : resources.missing;
        if (definition === gitDiscoveredReposR) {
            resources.listing(enabled);
            return resources.discovered;
        }
        return resources.empty;
    },
}));
vi.mock("./CommitReview", () => ({ CommitReview: () => <div>Review</div> }));
vi.mock("./MergeReview", () => ({ MergeReview: () => <div>Merge review</div> }));

beforeEach(() => {
    resources.listing.mockClear();
    vi.restoreAllMocks();
    setState({ gitViews: {}, gitModal: null, pickerOpen: false });
    useGitWorkbench.setState({ drafts: {}, operations: {} });
});
afterEach(cleanup);

const section = (name: string) => screen.getByRole("button", { name });

it("shows every repository in the folder, open while it has changes", () => {
    render(<GitPane paneId="git-test" cwd="/container" active visible />);

    expect(section("docs")).toHaveAttribute("aria-expanded", "true");
    expect(section("web")).toHaveAttribute("aria-expanded", "false");
    expect(screen.getByText("Staged Changes")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "notes/plan.md, staged" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "README.md, unstaged" })).toBeInTheDocument();
});

it("folds a section and remembers it", async () => {
    const user = userEvent.setup();
    render(<GitPane paneId="git-test" cwd="/container" active visible />);

    await user.click(section("docs"));

    expect(getState().gitViews["git-test"].repoFolds).toEqual({ "/container/docs": false });
    expect(screen.queryByText("Staged Changes")).not.toBeInTheDocument();
});

it("stages a file and commits from the repository's own section", async () => {
    const user = userEvent.setup();
    const stage = vi.spyOn(git, "stage").mockResolvedValue();
    const commit = vi.spyOn(git, "commit").mockResolvedValue("committed");
    vi.spyOn(git, "status").mockResolvedValue({ ...resources.docs.data });
    render(<GitPane paneId="git-test" cwd="/container" active visible />);

    await user.click(screen.getByRole("button", { name: "Stage README.md" }));
    expect(stage).toHaveBeenCalledWith("/container/docs", "README.md");

    await user.type(screen.getByRole("textbox", { name: "Commit message for docs" }), "docs: plan");
    await user.click(screen.getByRole("button", { name: "Commit" }));
    await vi.waitFor(() => expect(commit).toHaveBeenCalledWith("/container/docs", "docs: plan"));
});

it("opens a repository's full view from its section, with the clicked file picked", async () => {
    const user = userEvent.setup();
    render(<GitPane paneId="git-test" cwd="/container" active visible />);

    await user.click(screen.getByRole("button", { name: "README.md, unstaged" }));

    expect(getState().gitViews["git-test"]).toMatchObject({ repo: "/container/docs", sourceControl: false, panel: "files", selected: { files: 3 } });
});

it("goes back to every repository from a repository's full view", async () => {
    const user = userEvent.setup();
    setState({ gitViews: { "git-test": { ...DEFAULT_GIT_VIEW, repo: "/container/docs" } } });
    render(<GitPane paneId="git-test" cwd="/container" active visible />);

    expect(screen.queryByText("Source Control")).not.toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: /docs/ }));

    expect(getState().gitViews["git-test"]).toMatchObject({ repo: null, sourceControl: true });
    expect(within(screen.getByRole("radiogroup", { name: "Repositories from" })).getByRole("radio", { name: "This project" })).toBeChecked();
});

it("keeps the list live while it is on screen beside the pane being worked in", () => {
    render(<GitPane paneId="git-test" cwd="/container" active={false} visible />);

    expect(resources.listing).toHaveBeenLastCalledWith(true);
});

it("stops keeping it live once it is off screen", () => {
    render(<GitPane paneId="git-test" cwd="/container" active={false} visible={false} />);

    expect(resources.listing).not.toHaveBeenCalledWith(true);
});
