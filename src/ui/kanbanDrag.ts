import { useRef, useState, type PointerEvent as ReactPointerEvent } from "react";

const DRAG_START_PX = 5;
/** How close to a scrolling edge the pointer comes before the board scrolls, and how fast it then goes. */
const EDGE_PX = 48;
const EDGE_SPEED_PX = 14;

/**
 * The column under a point, by the `data-column` its element carries. Columns in a row are only as tall as their
 * cards, so a point in the row's empty space under a short column counts as that column.
 */
export function columnElementAt(x: number, y: number): HTMLElement | null {
    const hit = document.elementFromPoint(x, y)?.closest<HTMLElement>("[data-column]");
    if (hit) return hit;
    for (const column of document.querySelectorAll<HTMLElement>("[data-column]")) {
        const own = column.getBoundingClientRect();
        const row = (column.parentElement ?? column).getBoundingClientRect();
        if (x >= own.left && x <= own.right && y >= row.top && y <= row.bottom) return column;
    }
    return null;
}

export function columnAt(x: number, y: number): string | null {
    return columnElementAt(x, y)?.dataset.column ?? null;
}

function scrollerOf(element: Element | null, axis: "x" | "y"): HTMLElement | null {
    for (let node = element?.parentElement ?? null; node; node = node.parentElement) {
        const style = getComputedStyle(node);
        const overflow = axis === "y" ? style.overflowY : style.overflowX;
        const room = axis === "y" ? node.scrollHeight > node.clientHeight : node.scrollWidth > node.clientWidth;
        if (room && (overflow === "auto" || overflow === "scroll")) return node;
    }
    return null;
}

/** How far to scroll a box this frame, when the pointer sits near one of its edges. */
export function edgeStep(position: number, start: number, end: number): number {
    if (position < start + EDGE_PX) return -Math.ceil(EDGE_SPEED_PX * (1 - Math.max(0, position - start) / EDGE_PX));
    if (position > end - EDGE_PX) return Math.ceil(EDGE_SPEED_PX * (1 - Math.max(0, end - position) / EDGE_PX));
    return 0;
}

/** A copy of the card that follows the pointer, so the card looks picked up. */
function liftedCopy(card: HTMLElement): HTMLElement {
    const box = card.getBoundingClientRect();
    const copy = card.cloneNode(true) as HTMLElement;
    copy.classList.add("kanban-card-lifted");
    copy.removeAttribute("id");
    copy.setAttribute("aria-hidden", "true");
    Object.assign(copy.style, {
        position: "fixed",
        left: `${box.left}px`,
        top: `${box.top}px`,
        width: `${box.width}px`,
        margin: "0",
        pointerEvents: "none",
    });
    document.body.append(copy);
    return copy;
}

/**
 * Dragging a board's cards between columns with the pointer. A card counts as dragged once it moves a few pixels;
 * then a copy of it follows the pointer, the column under it is marked unless it carries `data-drop="false"`, the board
 * scrolls near its edges, and Escape puts it back. `wasDragged` is
 * true for the click that ends a drag, so the card does not also open.
 */
export function useCardDrag(onDrop: (card: string, column: string) => void) {
    const [dragging, setDragging] = useState<{ card: string; over: string | null } | null>(null);
    const justDragged = useRef(false);

    const startDrag = (event: ReactPointerEvent, card: string) => {
        if (event.button !== 0) return;
        event.preventDefault();
        const element = event.currentTarget instanceof HTMLElement ? event.currentTarget : null;
        const start = { x: event.clientX, y: event.clientY };
        const pointer = { ...start };
        let copy: HTMLElement | null = null;
        let over: HTMLElement | null = null;
        let frame = 0;
        const scrollers = { x: scrollerOf(element, "x"), y: scrollerOf(element, "y") };

        const follow = () => {
            if (copy) copy.style.transform = `translate(${pointer.x - start.x}px, ${pointer.y - start.y}px) rotate(1.5deg)`;
            const next = columnElementAt(pointer.x, pointer.y);
            if (next === over) return;
            over?.classList.remove("drop");
            over = next;
            if (over && over.dataset.drop !== "false") over.classList.add("drop");
            setDragging({ card, over: over?.dataset.column ?? null });
        };
        const scroll = () => {
            frame = requestAnimationFrame(scroll);
            const { x, y } = scrollers;
            const down = y ? edgeStep(pointer.y, y.getBoundingClientRect().top, y.getBoundingClientRect().bottom) : 0;
            const across = x ? edgeStep(pointer.x, x.getBoundingClientRect().left, x.getBoundingClientRect().right) : 0;
            if (down && y) y.scrollTop += down;
            if (across && x) x.scrollLeft += across;
            if (down || across) follow();
        };
        const onMove = (next: PointerEvent) => {
            pointer.x = next.clientX;
            pointer.y = next.clientY;
            if (!copy) {
                if (Math.hypot(pointer.x - start.x, pointer.y - start.y) < DRAG_START_PX) return;
                if (element) copy = liftedCopy(element);
                document.documentElement.classList.add("kanban-dragging");
                setDragging({ card, over: null });
                frame = requestAnimationFrame(scroll);
            }
            follow();
        };
        const blockNativeDrag = (next: Event) => next.preventDefault();
        const onKey = (next: KeyboardEvent) => {
            if (next.key !== "Escape") return;
            next.preventDefault();
            next.stopPropagation();
            const moved = copy !== null;
            stop();
            if (moved) settle();
        };
        const settle = () => {
            justDragged.current = true;
            setTimeout(() => (justDragged.current = false), 0);
        };
        const stop = () => {
            window.removeEventListener("pointermove", onMove);
            window.removeEventListener("pointerup", onUp);
            window.removeEventListener("pointercancel", stop);
            window.removeEventListener("dragstart", blockNativeDrag);
            window.removeEventListener("keydown", onKey, true);
            cancelAnimationFrame(frame);
            copy?.remove();
            over?.classList.remove("drop");
            document.documentElement.classList.remove("kanban-dragging");
            setDragging(null);
        };
        const onUp = (next: PointerEvent) => {
            const moved = copy !== null;
            stop();
            if (!moved) return;
            settle();
            const to = columnAt(next.clientX, next.clientY);
            if (to !== null) onDrop(card, to);
        };
        window.addEventListener("pointermove", onMove);
        window.addEventListener("pointerup", onUp);
        window.addEventListener("pointercancel", stop);
        window.addEventListener("dragstart", blockNativeDrag);
        window.addEventListener("keydown", onKey, true);
    };

    return { dragging, startDrag, wasDragged: () => justDragged.current };
}
