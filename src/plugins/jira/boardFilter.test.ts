import { describe, expect, it } from "vitest";
import type { JiraBoardView, JiraIssueSummary } from "./api";
import {
    boardPeople,
    dueLabel,
    dueState,
    facetPick,
    facets,
    isLocalPick,
    lanes,
    matchingCards,
    parentColour,
    pickKey,
    unassignedCount,
    withFacets,
} from "./boardFilter";

const card = (key: string, summary: string, assignee: string | null) =>
    ({ key, summary, assignee: assignee ? { accountId: assignee.toLowerCase(), name: assignee } : null }) as JiraIssueSummary;

const board = {
    columns: [
        { name: "To Do", statusIds: [], issues: [card("CIQ-1", "Dossier picker", "Vivek"), card("CIQ-2", "Login page", null)] },
        { name: "Done", statusIds: [], issues: [card("CIQ-3", "Dossier chart", "Kiran"), card("CIQ-4", "Render queue", "Vivek")] },
    ],
} as unknown as JiraBoardView;

describe("boardFilter", () => {
    it("lists everyone with a card, busiest first, and counts the cards nobody has", () => {
        expect(boardPeople(board)).toEqual([
            { accountId: "vivek", name: "Vivek", count: 2 },
            { accountId: "kiran", name: "Kiran", count: 1 },
        ]);
        expect(unassignedCount(board)).toBe(1);
    });

    it("makes the same key for the same picks in any order", () => {
        expect(pickKey(new Set(["me", "abc"]))).toBe(pickKey(new Set(["abc", "me"])));
        expect(pickKey(new Set())).toBe("");
    });

    it("keeps the cards whose key or title holds the words", () => {
        const keys = (words: string) => matchingCards(board.columns, words).map((column) => column.issues.map((issue) => issue.key));
        expect(keys("dossier")).toEqual([["CIQ-1"], ["CIQ-3"]]);
        expect(keys("ciq-4")).toEqual([[], ["CIQ-4"]]);
        expect(matchingCards(board.columns, "  ")).toBe(board.columns);
    });
});

const rich = (key: string, extra: Partial<JiraIssueSummary>) => ({ ...card(key, key, null), ...extra }) as JiraIssueSummary;

const filled = {
    columns: [
        {
            name: "To Do",
            statusIds: [],
            issues: [
                rich("CIQ-1", {
                    issueType: "Bug",
                    priority: "High",
                    labels: ["web"],
                    versions: ["1.4"],
                    parent: { key: "CIQ-100", summary: "Studio", issueType: "Epic" },
                    assignee: { accountId: "a", name: "Ana" },
                }),
                rich("CIQ-2", { issueType: "Task", priority: "Low", labels: ["web", "api"], versions: [] }),
            ],
        },
        {
            name: "Done",
            statusIds: [],
            issues: [
                rich("CIQ-3", {
                    issueType: "Bug",
                    priority: "Highest",
                    labels: [],
                    versions: ["1.4"],
                    parent: { key: "CIQ-100", summary: "Studio", issueType: "Epic" },
                    assignee: { accountId: "a", name: "Ana" },
                }),
            ],
        },
    ],
} as unknown as JiraBoardView;

describe("the board's filters and lanes", () => {
    it("offers each version, epic, type and label on the board with its count", () => {
        const found = facets(filled);
        expect(found.version).toEqual([{ value: "1.4", label: "1.4", count: 2 }]);
        expect(found.epic).toEqual([{ value: "CIQ-100", label: "Studio", count: 2 }]);
        expect(found.type.map((option) => `${option.label} ${option.count}`)).toEqual(["Bug 2", "Task 1"]);
        expect(found.label.map((option) => option.value)).toEqual(["api", "web"]);
    });

    it("keeps cards matching any value within a filter and every filter picked", () => {
        const keys = (picks: string[]) => withFacets(filled.columns, new Set(picks)).flatMap((column) => column.issues.map((issue) => issue.key));
        expect(keys([])).toEqual(["CIQ-1", "CIQ-2", "CIQ-3"]);
        expect(keys([facetPick("type", "Bug")])).toEqual(["CIQ-1", "CIQ-3"]);
        expect(keys([facetPick("type", "Bug"), facetPick("type", "Task")])).toEqual(["CIQ-1", "CIQ-2", "CIQ-3"]);
        expect(keys([facetPick("type", "Bug"), facetPick("label", "web")])).toEqual(["CIQ-1"]);
        expect(isLocalPick(facetPick("epic", "CIQ-100"))).toBe(true);
        expect(isLocalPick("me")).toBe(false);
        expect(isLocalPick("q:7")).toBe(false);
    });

    it("splits the board into lanes, with the lane of cards without a value last", () => {
        expect(lanes(filled.columns, "none")).toHaveLength(1);
        const byPerson = lanes(filled.columns, "assignee");
        expect(byPerson.map((lane) => `${lane.title} ${lane.count}`)).toEqual(["Ana 2", "Unassigned 1"]);
        expect(byPerson[0]?.columns.map((column) => column.issues.length)).toEqual([1, 1]);
        expect(lanes(filled.columns, "priority").map((lane) => lane.title)).toEqual(["Highest", "High", "Low"]);
        expect(lanes(filled.columns, "epic").map((lane) => lane.title)).toEqual(["Studio", "No epic"]);
    });

    it("keeps an epic's colour, and reads due dates as Jira does", () => {
        expect(parentColour("CIQ-100")).toBe(parentColour("CIQ-100"));
        const today = new Date(2026, 9, 14);
        expect(dueState("2026-10-14", false, today)).toBe("today");
        expect(dueState("2026-10-02", false, today)).toBe("overdue");
        expect(dueState("2026-10-02", true, today)).toBe("later");
        expect(dueState(null, false, today)).toBeNull();
        expect(dueLabel("2026-10-15")).toBe("Oct 15, 2026");
    });
});
