import { describe, expect, it } from "vitest";
import { resolveTabDrop, type TabBox } from "./tabDrag";

// Four 100px tabs side by side: a 0–100, b 100–200, c 200–300, d 300–400.
const boxes: TabBox[] = ["a", "b", "c", "d"].map((id, i) => ({ id, left: i * 100, right: i * 100 + 100 }));

describe("where a dragged tab lands", () => {
    it("drops before a tab when over its left half and after it over the right", () => {
        expect(resolveTabDrop(310, boxes, "a")).toEqual({ targetId: "d", placement: "before" });
        expect(resolveTabDrop(390, boxes, "a")).toEqual({ targetId: "d", placement: "after" });
    });

    it("treats past either end of the strip as that end", () => {
        expect(resolveTabDrop(-40, boxes, "c")).toEqual({ targetId: "a", placement: "before" });
        expect(resolveTabDrop(900, boxes, "a")).toEqual({ targetId: "d", placement: "after" });
    });

    it("shows nothing while the pointer is over the dragged tab itself", () => {
        expect(resolveTabDrop(150, boxes, "b")).toBeNull();
    });

    it("shows nothing for a drop that would leave the tab where it is", () => {
        expect(resolveTabDrop(90, boxes, "b")).toBeNull(); // after a, which b already follows
        expect(resolveTabDrop(210, boxes, "b")).toBeNull(); // before c, which b already precedes
    });

    it("still moves a tab one place over a neighbour's far half", () => {
        expect(resolveTabDrop(10, boxes, "b")).toEqual({ targetId: "a", placement: "before" });
        expect(resolveTabDrop(290, boxes, "b")).toEqual({ targetId: "c", placement: "after" });
    });

    it("respects a drop the caller rules out", () => {
        const onlyAfterD = (_s: string, target: string, placement: string) => target === "d" && placement === "after";
        expect(resolveTabDrop(310, boxes, "a", onlyAfterD)).toBeNull();
        expect(resolveTabDrop(390, boxes, "a", onlyAfterD)).toEqual({ targetId: "d", placement: "after" });
    });

    it("has nowhere to land in an empty strip", () => {
        expect(resolveTabDrop(50, [], "a")).toBeNull();
    });
});
