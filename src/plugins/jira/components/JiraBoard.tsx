import { useRef, useState, type PointerEvent as ReactPointerEvent } from "react";
import { notify } from "../../../plugin-api/host";
import { invalidate, useResourceEnabled } from "../../../plugin-api/resources";
import { ContextMenu, EmptyState, IconRefresh, SkeletonRows, type ContextMenuItem } from "../../../plugin-api/ui";
import { failureMessage, jiraApi, type JiraBoardView, type JiraColumn, type JiraIssueSummary } from "../api";
import { boardPeople, matchingCards, pickKey, unassignedCount } from "../boardFilter";
import { jiraBoardR } from "../resources";
import { boardPicks, saveBoardPicks } from "../state";

/** Where each card sits after the moves still on their way to Jira, by issue key and column index. */
export function withMoves(board: JiraBoardView, moves: Record<string, number>): JiraColumn[] {
    const moving = new Set(Object.keys(moves));
    return board.columns.map((column, index) => ({
        ...column,
        issues: [
            ...column.issues.filter((issue) => !moving.has(issue.key)),
            ...board.columns.flatMap((other) => other.issues.filter((issue) => moves[issue.key] === index)),
        ],
    }));
}

/** How long is left in a sprint, in the words a standup uses. */
export function sprintLeft(end: string | null, now = Date.now()): string | null {
    if (!end) return null;
    const left = Date.parse(end) - now;
    if (Number.isNaN(left)) return null;
    if (left < 0) return "ended";
    const days = Math.floor(left / 86_400_000);
    if (days === 0) return "ends today";
    return days === 1 ? "1 day left" : `${days} days left`;
}

const DRAG_START_PX = 5;

export function JiraBoard({
    active,
    boardId,
    site,
    selected,
    onSelect,
}: {
    active: boolean;
    boardId: number;
    site: string;
    selected: string | null;
    onSelect: (key: string) => void;
}) {
    const [picked, setPickedOnly] = useState<ReadonlySet<string>>(() => new Set(boardPicks(boardId)));
    const setPicked = (next: ReadonlySet<string> | ((now: ReadonlySet<string>) => ReadonlySet<string>)) =>
        setPickedOnly((now) => {
            const picks = typeof next === "function" ? next(now) : next;
            saveBoardPicks(boardId, picks);
            return picks;
        });
    const [words, setWords] = useState("");
    const whole = useResourceEnabled(active, jiraBoardR, boardId, site, "");
    const found = useResourceEnabled(active, jiraBoardR, boardId, site, pickKey(picked));
    const [moves, setMoves] = useState<Record<string, number>>({});
    /* The card in hand: which one, the column it left and the one under the
       pointer, and its size, so its place and the drop slot keep that shape. */
    const [dragging, setDragging] = useState<{
        issue: JiraIssueSummary;
        from: number;
        over: number | null;
        width: number;
        height: number;
        /* Where it was first drawn, until the pointer moves it on. */
        at: { x: number; y: number };
    } | null>(null);
    const [landed, setLanded] = useState<string | null>(null);
    const [menu, setMenu] = useState<{ x: number; y: number; items: ContextMenuItem[] } | null>(null);
    const justDragged = useRef(false);
    const lifted = useRef<HTMLDivElement | null>(null);

    if (whole.status === "error") return <EmptyState message={whole.error ?? "Jira could not open this board."} tone="error" />;
    if (!whole.data) return <SkeletonRows rows={6} label="Loading the board" />;
    const board = whole.data;
    const narrowed = picked.size === 0 ? board : found.data;
    const columns = narrowed ? matchingCards(withMoves(narrowed, moves), words) : [];
    const refresh = () => invalidate((kind) => kind === "jira.board");
    const toggle = (who: string) =>
        setPicked((now) => {
            const next = new Set(now);
            if (!next.delete(who)) next.add(who);
            return next;
        });
    const nobody = unassignedCount(whole.data);

    const move = async (issue: JiraIssueSummary, to: number) => {
        const from = columns.findIndex((column) => column.issues.some((each) => each.key === issue.key));
        const target = board.columns[to];
        if (from === to || !target) return;
        setMoves((now) => ({ ...now, [issue.key]: to }));
        setLanded(issue.key);
        try {
            await jiraApi.moveIssue(issue.key, target, site || undefined);
            refresh();
            await found.refresh();
            invalidate((kind) => kind === "jira.search" || kind === "jira.issue");
        } catch (error) {
            notify("error", `Move ${issue.key}: ${failureMessage(error)}`);
        } finally {
            setMoves((now) => {
                const next = { ...now };
                delete next[issue.key];
                return next;
            });
        }
    };

    const columnAt = (x: number, y: number): number | null => {
        const element = document.elementFromPoint(x, y)?.closest<HTMLElement>("[data-column]");
        return element ? Number(element.dataset.column) : null;
    };

    const startDrag = (event: ReactPointerEvent<HTMLElement>, issue: JiraIssueSummary, from: number) => {
        if (event.button !== 0) return;
        const start = { x: event.clientX, y: event.clientY };
        const card = event.currentTarget.getBoundingClientRect();
        const grip = { x: start.x - card.left, y: start.y - card.top };
        let moved = false;
        let over: number | null = from;
        // The lifted card follows the pointer straight from here, so a long
        // board is not drawn again for every move.
        const follow = (x: number, y: number) => {
            if (lifted.current) lifted.current.style.transform = `translate(${x - grip.x}px, ${y - grip.y}px) rotate(2deg)`;
        };
        const stop = () => {
            window.removeEventListener("pointermove", onMove);
            window.removeEventListener("pointerup", onUp);
            window.removeEventListener("keydown", onKey);
            setDragging(null);
        };
        const onMove = (next: PointerEvent) => {
            if (!moved && Math.hypot(next.clientX - start.x, next.clientY - start.y) < DRAG_START_PX) return;
            if (!moved) {
                moved = true;
                over = columnAt(next.clientX, next.clientY);
                setDragging({
                    issue,
                    from,
                    over,
                    width: card.width,
                    height: card.height,
                    at: { x: next.clientX - grip.x, y: next.clientY - grip.y },
                });
            }
            follow(next.clientX, next.clientY);
            const now = columnAt(next.clientX, next.clientY);
            if (now !== over) {
                over = now;
                setDragging((held) => held && { ...held, over: now });
            }
        };
        const onUp = (next: PointerEvent) => {
            stop();
            if (!moved) return;
            justDragged.current = true;
            setTimeout(() => (justDragged.current = false), 0);
            const to = columnAt(next.clientX, next.clientY);
            if (to !== null) void move(issue, to);
        };
        const onKey = (next: KeyboardEvent) => {
            if (next.key !== "Escape") return;
            next.preventDefault();
            moved = false;
            stop();
        };
        window.addEventListener("pointermove", onMove);
        window.addEventListener("pointerup", onUp);
        window.addEventListener("keydown", onKey);
    };

    const left = sprintLeft(board.sprint?.end ?? null);

    return (
        <section className="jira-board" aria-label={board.name}>
            <header className="jira-board-head">
                <div className="jira-board-title">
                    <h2>{board.name}</h2>
                    <span className="jira-chip static">{board.kind}</span>
                    {board.sprint && (
                        <span className="jira-meta">
                            {board.sprint.name}
                            {left && ` · ${left}`}
                        </span>
                    )}
                    <span className="jira-grow" />
                    <button type="button" className="jira-icon-button" title="Refresh" aria-label="Refresh the board" onClick={refresh}>
                        <IconRefresh size={13} />
                    </button>
                </div>
                {board.sprint?.goal && <p className="jira-board-goal">{board.sprint.goal}</p>}
                {(narrowed ?? board).truncated && (
                    <p className="jira-meta">Showing the first 300 issues. Pick people or narrow the board in Jira to see the rest.</p>
                )}
                <div className="jira-board-filter-row" role="group" aria-label="Filter the board">
                    <input
                        className="jira-board-search"
                        value={words}
                        onChange={(event) => setWords(event.target.value)}
                        placeholder="Find a card"
                        aria-label="Find a card"
                        spellCheck={false}
                    />
                    <button type="button" className="jira-board-pick" aria-pressed={picked.has("me")} onClick={() => toggle("me")}>
                        Only my issues
                    </button>
                    <span className="jira-board-people">
                        {boardPeople(whole.data).map((person) => (
                            <button
                                key={person.accountId}
                                type="button"
                                className="jira-avatar jira-avatar-pick"
                                title={`${person.name} · ${person.count}`}
                                aria-label={person.name}
                                aria-pressed={picked.has(person.accountId)}
                                onClick={() => toggle(person.accountId)}>
                                {initials(person.name)}
                            </button>
                        ))}
                    </span>
                    {nobody > 0 && (
                        <button type="button" className="jira-board-pick" aria-pressed={picked.has("none")} onClick={() => toggle("none")}>
                            Unassigned <span className="jira-board-count">{nobody}</span>
                        </button>
                    )}
                    {(picked.size > 0 || words) && (
                        <button
                            type="button"
                            className="jira-board-clear"
                            onClick={() => {
                                setPicked(new Set());
                                setWords("");
                            }}>
                            Clear
                        </button>
                    )}
                </div>
            </header>
            {board.kind === "scrum" && !board.sprint ? (
                <EmptyState message="No sprint is running on this board." />
            ) : found.status === "error" && picked.size > 0 ? (
                <EmptyState message={found.error ?? "Jira could not narrow the board to those people."} tone="error" />
            ) : !narrowed ? (
                <SkeletonRows rows={6} label="Loading their cards" />
            ) : (
                <div className="jira-columns">
                    {columns.map((column, index) => (
                        <div
                            key={column.name}
                            className={`jira-column${dragging?.over === index && dragging.from !== index ? " drop" : ""}`}
                            data-column={index}
                            role="list"
                            aria-label={column.name}>
                            <div className="jira-column-head">
                                <span>{column.name}</span>
                                <span className="jira-count">{column.issues.length}</span>
                            </div>
                            {dragging?.over === index && dragging.from !== index && (
                                <div className="jira-drop-slot" style={{ height: dragging.height }} aria-hidden />
                            )}
                            {column.issues.map((issue) => (
                                <button
                                    key={issue.key}
                                    type="button"
                                    role="listitem"
                                    className={`jira-card${issue.key === selected ? " active" : ""}${dragging?.issue.key === issue.key ? " dragging" : ""}${issue.key in moves ? " moving" : ""}${issue.key === landed ? " landed" : ""}`}
                                    onPointerDown={(event) => startDrag(event, issue, index)}
                                    onAnimationEnd={() => {
                                        if (issue.key === landed) setLanded(null);
                                    }}
                                    onClick={() => {
                                        if (!justDragged.current) onSelect(issue.key);
                                    }}
                                    onContextMenu={(event) => {
                                        event.preventDefault();
                                        setMenu({
                                            x: event.clientX,
                                            y: event.clientY,
                                            items: board.columns.map((target, to) => ({
                                                label: `Move to ${target.name}`,
                                                disabled: to === index,
                                                run: () => void move(issue, to),
                                            })),
                                        });
                                    }}>
                                    <CardBody issue={issue} />
                                </button>
                            ))}
                        </div>
                    ))}
                </div>
            )}
            {dragging && (
                <div
                    ref={lifted}
                    className="jira-card jira-card-lifted"
                    style={{ width: dragging.width, transform: `translate(${dragging.at.x}px, ${dragging.at.y}px) rotate(2deg)` }}
                    aria-hidden>
                    <CardBody issue={dragging.issue} />
                </div>
            )}
            {menu && <ContextMenu x={menu.x} y={menu.y} items={menu.items} onClose={() => setMenu(null)} />}
        </section>
    );
}

function CardBody({ issue }: { issue: JiraIssueSummary }) {
    return (
        <>
            <span className="jira-card-summary">{issue.summary}</span>
            <span className="jira-card-foot">
                <span className="jira-key">{issue.key}</span>
                {issue.issueType && <span className="jira-meta">{issue.issueType}</span>}
                {issue.priority && <span className="jira-meta">{issue.priority}</span>}
                <span className="jira-grow" />
                {issue.assignee && (
                    <span className="jira-avatar" title={issue.assignee.name}>
                        {initials(issue.assignee.name)}
                    </span>
                )}
            </span>
        </>
    );
}

function initials(name: string): string {
    return name
        .split(/\s+/)
        .filter(Boolean)
        .slice(0, 2)
        .map((part) => part[0]?.toUpperCase() ?? "")
        .join("");
}
