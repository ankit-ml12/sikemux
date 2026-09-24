export type TabPlacement = "before" | "after";

export interface TabDrop {
    targetId: string;
    placement: TabPlacement;
}

/** A tab's horizontal extent on screen, in strip order. */
export interface TabBox {
    id: string;
    left: number;
    right: number;
}

/** Travel before a press becomes a drag, so a click still just selects. */
export const TAB_DRAG_THRESHOLD = 5;

/**
 * Where a dragged tab would land for a pointer at `x`, or null when it would
 * not move. Past either end of the strip counts as that end, so a tab can be
 * dropped first or last without aiming at the outermost pill. A drop that
 * leaves the tab where it already is returns null, so no marker appears for it.
 */
export function resolveTabDrop(
    x: number,
    boxes: readonly TabBox[],
    sourceId: string,
    canDrop: (sourceId: string, targetId: string, placement: TabPlacement) => boolean = () => true,
): TabDrop | null {
    if (boxes.length === 0) return null;
    const first = boxes[0];
    const last = boxes[boxes.length - 1];
    const box = x < first.left ? first : x > last.right ? last : boxes.find((b) => x >= b.left && x <= b.right);
    if (!box || box.id === sourceId) return null;

    const placement: TabPlacement = x < (box.left + box.right) / 2 ? "before" : "after";
    const from = boxes.findIndex((b) => b.id === sourceId);
    const to = boxes.indexOf(box);
    const unchanged = (placement === "after" && to === from - 1) || (placement === "before" && to === from + 1);
    if (unchanged || !canDrop(sourceId, box.id, placement)) return null;
    return { targetId: box.id, placement };
}
