import { act, cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { columnAt, edgeStep, useCardDrag } from "./kanbanDrag";

/** Two columns side by side in one row: "todo" holds a card down to y 300, "done" is empty under its header. */
function Board({ onDrop }: { onDrop: (card: string, column: string) => void }) {
    const drag = useCardDrag(onDrop);
    return (
        <div className="kanban-columns" data-testid="row">
            <div data-column="todo" data-testid="todo">
                <button type="button" className="kanban-card" onPointerDown={(event) => drag.startDrag(event, "CIQ-1")}>
                    Card
                </button>
            </div>
            <div data-column="done" data-testid="done" />
            <span data-testid="state">{drag.dragging ? `carrying ${drag.dragging.card} over ${drag.dragging.over ?? "nothing"}` : "still"}</span>
        </div>
    );
}

const box = (left: number, top: number, right: number, bottom: number) =>
    ({ left, top, right, bottom, width: right - left, height: bottom - top, x: left, y: top, toJSON: () => ({}) }) as DOMRect;

let under: Element | null = null;

beforeEach(() => {
    under = null;
    document.elementFromPoint = vi.fn(() => under);
    vi.spyOn(window, "requestAnimationFrame").mockReturnValue(1);
    vi.spyOn(window, "cancelAnimationFrame").mockImplementation(() => {});
});
afterEach(() => {
    cleanup();
    vi.restoreAllMocks();
});

function lay() {
    vi.spyOn(screen.getByTestId("row"), "getBoundingClientRect").mockReturnValue(box(0, 0, 400, 600));
    vi.spyOn(screen.getByTestId("todo"), "getBoundingClientRect").mockReturnValue(box(0, 0, 200, 300));
    vi.spyOn(screen.getByTestId("done"), "getBoundingClientRect").mockReturnValue(box(200, 0, 400, 40));
}

describe("dragging a card between columns", () => {
    it("finds a column from the empty space under it, where a short column ends", () => {
        render(<Board onDrop={vi.fn()} />);
        lay();
        expect(columnAt(300, 500)).toBe("done");
        expect(columnAt(100, 500)).toBe("todo");
        expect(columnAt(300, 700)).toBeNull();
        under = screen.getByTestId("todo").firstElementChild;
        expect(columnAt(300, 500)).toBe("todo");
    });

    it("lifts a copy that follows the pointer, and drops the card on the column let go over", () => {
        const onDrop = vi.fn();
        render(<Board onDrop={onDrop} />);
        lay();
        fireEvent.pointerDown(screen.getByRole("button", { name: "Card" }), { button: 0, clientX: 50, clientY: 50 });
        act(() => window.dispatchEvent(new MouseEvent("pointermove", { clientX: 300, clientY: 450 })));
        expect(screen.getByTestId("state")).toHaveTextContent("carrying CIQ-1 over done");
        expect(screen.getByTestId("done")).toHaveClass("drop");
        expect(screen.getByTestId("todo")).not.toHaveClass("drop");
        const lifted = document.querySelector<HTMLElement>(".kanban-card-lifted");
        expect(lifted?.style.transform).toBe("translate(250px, 400px) rotate(1.5deg)");
        expect(document.documentElement).toHaveClass("kanban-dragging");
        act(() => window.dispatchEvent(new MouseEvent("pointerup", { clientX: 300, clientY: 450 })));
        expect(onDrop).toHaveBeenCalledWith("CIQ-1", "done");
        expect(screen.getByTestId("done")).not.toHaveClass("drop");
        expect(document.querySelector(".kanban-card-lifted")).toBeNull();
        expect(document.documentElement).not.toHaveClass("kanban-dragging");
        expect(screen.getByTestId("state")).toHaveTextContent("still");
    });

    it("marks only the column under the pointer, and none that refuses cards", () => {
        render(<Board onDrop={vi.fn()} />);
        lay();
        screen.getByTestId("todo").dataset.drop = "false";
        fireEvent.pointerDown(screen.getByRole("button", { name: "Card" }), { button: 0, clientX: 300, clientY: 50 });
        act(() => window.dispatchEvent(new MouseEvent("pointermove", { clientX: 100, clientY: 450 })));
        expect(screen.getByTestId("todo")).not.toHaveClass("drop");
        expect(screen.getByTestId("state")).toHaveTextContent("over todo");
        act(() => window.dispatchEvent(new MouseEvent("pointerup", { clientX: 100, clientY: 450 })));
    });

    it("treats a short press as a click, not a drag", () => {
        const onDrop = vi.fn();
        render(<Board onDrop={onDrop} />);
        lay();
        fireEvent.pointerDown(screen.getByRole("button", { name: "Card" }), { button: 0, clientX: 50, clientY: 50 });
        act(() => window.dispatchEvent(new MouseEvent("pointermove", { clientX: 52, clientY: 51 })));
        act(() => window.dispatchEvent(new MouseEvent("pointerup", { clientX: 52, clientY: 51 })));
        expect(onDrop).not.toHaveBeenCalled();
        expect(document.querySelector(".kanban-card-lifted")).toBeNull();
    });

    it("puts the card back on Escape", () => {
        const onDrop = vi.fn();
        render(<Board onDrop={onDrop} />);
        lay();
        fireEvent.pointerDown(screen.getByRole("button", { name: "Card" }), { button: 0, clientX: 50, clientY: 50 });
        act(() => window.dispatchEvent(new MouseEvent("pointermove", { clientX: 300, clientY: 450 })));
        act(() => {
            window.dispatchEvent(new KeyboardEvent("keydown", { key: "Escape" }));
        });
        act(() => window.dispatchEvent(new MouseEvent("pointerup", { clientX: 300, clientY: 450 })));
        expect(onDrop).not.toHaveBeenCalled();
        expect(document.querySelector(".kanban-card-lifted")).toBeNull();
        expect(screen.getByTestId("state")).toHaveTextContent("still");
    });

    it("scrolls faster the nearer the pointer comes to an edge", () => {
        expect(edgeStep(300, 0, 600)).toBe(0);
        expect(edgeStep(2, 0, 600)).toBeLessThan(edgeStep(40, 0, 600));
        expect(edgeStep(40, 0, 600)).toBeLessThan(0);
        expect(edgeStep(598, 0, 600)).toBeGreaterThan(edgeStep(560, 0, 600));
        expect(edgeStep(560, 0, 600)).toBeGreaterThan(0);
    });
});
