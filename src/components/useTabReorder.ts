import { useCallback, useEffect, useRef, useState, type PointerEvent as ReactPointerEvent, type RefObject } from "react";
import { resolveTabDrop, TAB_DRAG_THRESHOLD, type TabBox, type TabDrop, type TabPlacement } from "./tabDrag";

export type TabReorderHandler = (sourceId: string, targetId: string, placement: TabPlacement) => void;
export type TabDropRule = (sourceId: string, targetId: string, placement: TabPlacement) => boolean;

interface DragSession {
    sourceId: string;
    startX: number;
    startY: number;
    active: boolean;
}

/**
 * Press-and-drag reordering for a tab strip. Nothing moves until the pointer
 * travels a few pixels, so a click still only selects; the strip commits the
 * move once, on release, and Escape abandons it.
 */
export function useTabReorder(
    tabElements: RefObject<Map<string, HTMLElement>>,
    orderedIds: readonly string[],
    onReorder: TabReorderHandler | undefined,
    canDrop: TabDropRule | undefined,
) {
    const [draggingId, setDraggingId] = useState<string | null>(null);
    const [drop, setDrop] = useState<TabDrop | null>(null);
    const session = useRef<DragSession | null>(null);
    const detach = useRef<(() => void) | null>(null);
    const swallowClick = useRef(false);
    const latest = useRef({ orderedIds, onReorder, canDrop });
    latest.current = { orderedIds, onReorder, canDrop };

    const boxes = (): TabBox[] =>
        latest.current.orderedIds.flatMap((id) => {
            const tab = tabElements.current?.get(id);
            const pill = tab?.closest(".tab-wrap") ?? tab;
            if (!pill) return [];
            const { left, right } = pill.getBoundingClientRect();
            return [{ id, left, right }];
        });

    const end = useCallback(() => {
        detach.current?.();
        detach.current = null;
        session.current = null;
        setDraggingId(null);
        setDrop(null);
        document.body.classList.remove("is-sorting-tabs");
    }, []);

    useEffect(() => end, [end]);

    const onPointerDown = (event: ReactPointerEvent, sourceId: string) => {
        if (!latest.current.onReorder || event.button !== 0) return;
        end();
        session.current = { sourceId, startX: event.clientX, startY: event.clientY, active: false };

        const dropAt = (x: number, drag: DragSession) => resolveTabDrop(x, boxes(), drag.sourceId, latest.current.canDrop);
        const move = (e: PointerEvent) => {
            const drag = session.current;
            if (!drag) return;
            if (!drag.active) {
                if (Math.hypot(e.clientX - drag.startX, e.clientY - drag.startY) < TAB_DRAG_THRESHOLD) return;
                drag.active = true;
                setDraggingId(drag.sourceId);
                document.body.classList.add("is-sorting-tabs");
            }
            e.preventDefault();
            const next = dropAt(e.clientX, drag);
            setDrop((current) => (current?.targetId === next?.targetId && current?.placement === next?.placement ? current : next));
        };
        const release = (e: PointerEvent) => {
            const drag = session.current;
            const target = drag?.active ? dropAt(e.clientX, drag) : null;
            if (drag?.active) {
                // The browser still delivers a click to the pill the drag began on.
                swallowClick.current = true;
                window.setTimeout(() => (swallowClick.current = false), 0);
            }
            end();
            if (drag && target) latest.current.onReorder?.(drag.sourceId, target.targetId, target.placement);
        };
        const cancelOnEscape = (e: KeyboardEvent) => {
            if (e.key !== "Escape" || !session.current?.active) return;
            e.preventDefault();
            e.stopPropagation();
            end();
        };

        window.addEventListener("pointermove", move);
        window.addEventListener("pointerup", release);
        window.addEventListener("pointercancel", end);
        window.addEventListener("keydown", cancelOnEscape, true);
        detach.current = () => {
            window.removeEventListener("pointermove", move);
            window.removeEventListener("pointerup", release);
            window.removeEventListener("pointercancel", end);
            window.removeEventListener("keydown", cancelOnEscape, true);
        };
    };

    /** True when this click is the tail of a drag and should not select. */
    const consumeClick = (): boolean => {
        if (!swallowClick.current) return false;
        swallowClick.current = false;
        return true;
    };

    const dragClass = (id: string): string => {
        if (draggingId === id) return " tab-drag-source";
        if (drop?.targetId === id) return ` tab-drop-${drop.placement}`;
        return "";
    };

    return { onPointerDown, consumeClick, dragClass, dragging: draggingId !== null };
}
