import type { JiraBoardView, JiraColumn, JiraIssueSummary, JiraPerson } from "./api";

export interface BoardPerson {
    accountId: string;
    name: string;
    count: number;
}

/** Everyone with a card on the board, busiest first. */
export function boardPeople(board: JiraBoardView): BoardPerson[] {
    const people = new Map<string, BoardPerson>();
    for (const issue of board.columns.flatMap((column) => column.issues)) {
        if (!issue.assignee) continue;
        const seen = people.get(issue.assignee.accountId);
        if (seen) seen.count += 1;
        else people.set(issue.assignee.accountId, { ...issue.assignee, count: 1 });
    }
    return [...people.values()].sort((a, b) => b.count - a.count || a.name.localeCompare(b.name));
}

export const unassignedCount = (board: JiraBoardView) =>
    board.columns.reduce((total, column) => total + column.issues.filter((issue) => !issue.assignee).length, 0);

/** The same picks always make the same key, so a board narrowed to them is fetched once. */
export const pickKey = (picked: ReadonlySet<string>) => [...picked].sort().join(",");

/** The cards whose key or title holds the typed words. */
export function matchingCards(columns: JiraColumn[], words: string): JiraColumn[] {
    const needle = words.trim().toLowerCase();
    if (!needle) return columns;
    return columns.map((column) => ({
        ...column,
        issues: column.issues.filter((issue) => issue.key.toLowerCase().includes(needle) || issue.summary.toLowerCase().includes(needle)),
    }));
}

/** The filters Jira's board offers beside people, which narrow the cards already loaded. */
export type Facet = "version" | "epic" | "type" | "label";

export interface FacetOption {
    value: string;
    label: string;
    count: number;
}

const FACET_PREFIX: Record<Facet, string> = { version: "v:", epic: "e:", type: "t:", label: "l:" };

/** A pick that narrows loaded cards rather than asking Jira again. */
export const isLocalPick = (pick: string) => Object.values(FACET_PREFIX).some((prefix) => pick.startsWith(prefix));

export const facetPick = (facet: Facet, value: string) => `${FACET_PREFIX[facet]}${value}`;

function valuesOf(issue: JiraIssueSummary, facet: Facet): { value: string; label: string }[] {
    switch (facet) {
        case "version":
            return issue.versions.map((version) => ({ value: version, label: version }));
        case "epic":
            return issue.parent ? [{ value: issue.parent.key, label: issue.parent.summary || issue.parent.key }] : [];
        case "type":
            return issue.issueType ? [{ value: issue.issueType, label: issue.issueType }] : [];
        case "label":
            return issue.labels.map((label) => ({ value: label, label }));
    }
}

/** What each filter can pick from on this board, with how many cards each would keep. */
export function facets(board: JiraBoardView): Record<Facet, FacetOption[]> {
    const issues = board.columns.flatMap((column) => column.issues);
    const of = (facet: Facet) => {
        const seen = new Map<string, FacetOption>();
        for (const issue of issues) {
            for (const { value, label } of valuesOf(issue, facet)) {
                const option = seen.get(value);
                if (option) option.count += 1;
                else seen.set(value, { value, label, count: 1 });
            }
        }
        return [...seen.values()].sort((a, b) => a.label.localeCompare(b.label));
    };
    return { version: of("version"), epic: of("epic"), type: of("type"), label: of("label") };
}

/** The cards that match every filter picked, and any of the values picked within one. */
export function withFacets(columns: JiraColumn[], picks: ReadonlySet<string>): JiraColumn[] {
    const wanted = (Object.keys(FACET_PREFIX) as Facet[])
        .map((facet) => ({ facet, values: [...picks].filter((pick) => pick.startsWith(FACET_PREFIX[facet])).map((pick) => pick.slice(2)) }))
        .filter(({ values }) => values.length > 0);
    if (wanted.length === 0) return columns;
    const keeps = (issue: JiraIssueSummary) =>
        wanted.every(({ facet, values }) => valuesOf(issue, facet).some(({ value }) => values.includes(value)));
    return columns.map((column) => ({ ...column, issues: column.issues.filter(keeps) }));
}

export type Grouping = "none" | "assignee" | "epic" | "priority" | "type";

export const GROUPINGS: { value: Grouping; label: string }[] = [
    { value: "none", label: "None" },
    { value: "assignee", label: "Assignee" },
    { value: "epic", label: "Epic" },
    { value: "priority", label: "Priority" },
    { value: "type", label: "Type" },
];

/** One swimlane: the cards of one person, epic, priority or type, in the board's columns. */
export interface Lane {
    id: string;
    title: string;
    person: JiraPerson | null;
    parentKey: string | null;
    count: number;
    columns: JiraColumn[];
}

const PRIORITY_ORDER = ["highest", "high", "medium", "low", "lowest"];

function laneOf(issue: JiraIssueSummary, grouping: Grouping): { id: string; title: string; person: JiraPerson | null; parentKey: string | null } {
    switch (grouping) {
        case "assignee":
            return issue.assignee
                ? { id: issue.assignee.accountId, title: issue.assignee.name, person: issue.assignee, parentKey: null }
                : { id: "", title: "Unassigned", person: null, parentKey: null };
        case "epic":
            return issue.parent
                ? { id: issue.parent.key, title: issue.parent.summary || issue.parent.key, person: null, parentKey: issue.parent.key }
                : { id: "", title: "No epic", person: null, parentKey: null };
        case "priority":
            return { id: issue.priority ?? "", title: issue.priority ?? "No priority", person: null, parentKey: null };
        case "type":
            return { id: issue.issueType ?? "", title: issue.issueType ?? "No type", person: null, parentKey: null };
        case "none":
            return { id: "all", title: "", person: null, parentKey: null };
    }
}

/** The board split into swimlanes, each keeping every column. The lane of cards with nothing to group by comes last. */
export function lanes(columns: JiraColumn[], grouping: Grouping): Lane[] {
    if (grouping === "none") {
        const count = columns.reduce((total, column) => total + column.issues.length, 0);
        return [{ id: "all", title: "", person: null, parentKey: null, count, columns }];
    }
    const found = new Map<string, Lane>();
    columns.forEach((column, index) => {
        for (const issue of column.issues) {
            const lane = laneOf(issue, grouping);
            let held = found.get(lane.id);
            if (!held) {
                held = { ...lane, count: 0, columns: columns.map((each) => ({ ...each, issues: [] })) };
                found.set(lane.id, held);
            }
            held.count += 1;
            held.columns[index]?.issues.push(issue);
        }
    });
    const order = (lane: Lane) => {
        if (!lane.id) return Number.MAX_SAFE_INTEGER;
        const rank = grouping === "priority" ? PRIORITY_ORDER.indexOf(lane.id.toLowerCase()) : -1;
        return rank < 0 ? PRIORITY_ORDER.length : rank;
    };
    return [...found.values()].sort((a, b) => order(a) - order(b) || a.title.localeCompare(b.title));
}

/** Jira's lozenge colours, one picked from a parent's key so an epic keeps its colour. */
const PARENT_COLOURS = ["#5e4db2", "#c9372c", "#1d7afc", "#22a06b", "#e2b203", "#ae4787", "#2898bd", "#d97008"];

export function parentColour(key: string): string {
    let hash = 0;
    for (const char of key) hash = (hash * 31 + char.charCodeAt(0)) >>> 0;
    return PARENT_COLOURS[hash % PARENT_COLOURS.length] ?? PARENT_COLOURS[0];
}

/** How urgent a due date reads on a card: past and not done, today, or neither. */
export function dueState(due: string | null, done: boolean, today = new Date()): "overdue" | "today" | "later" | null {
    if (!due) return null;
    const day = `${today.getFullYear()}-${String(today.getMonth() + 1).padStart(2, "0")}-${String(today.getDate()).padStart(2, "0")}`;
    if (due === day) return done ? "later" : "today";
    return due < day && !done ? "overdue" : "later";
}

/** A due date the way Jira's cards write it, such as Oct 15, 2026. */
export function dueLabel(due: string): string {
    const [year, month, day] = due.split("-").map(Number);
    if (!year || !month || !day) return due;
    return new Date(year, month - 1, day).toLocaleDateString("en-US", { month: "short", day: "numeric", year: "numeric" });
}
