import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { TabBar, type TabDescriptor } from "./TabBar";

const tabs: TabDescriptor[] = ["a", "b", "c"].map((id) => ({ id, label: id }));

/* jsdom lays nothing out, so each pill reports where it would sit: 100px wide, side by side. */
function placePills() {
    document.querySelectorAll<HTMLElement>(".tab-wrap").forEach((pill) => {
        const left = Number(pill.dataset.index) * 100;
        vi.spyOn(pill, "getBoundingClientRect").mockReturnValue({ left, right: left + 100, top: 0, bottom: 30, width: 100, height: 30 } as DOMRect);
    });
}

function renderStrip(props: Partial<Parameters<typeof TabBar>[0]> = {}) {
    const onSelect = vi.fn();
    const onReorder = vi.fn();
    render(<TabBar variant="agent" tabs={tabs} onSelect={onSelect} onReorder={onReorder} {...props} />);
    placePills();
    return { onSelect, onReorder, tab: (name: string) => screen.getByRole("tab", { name }) };
}

describe("dragging a tab to a new place", () => {
    beforeEach(() => document.body.classList.remove("is-sorting-tabs"));
    afterEach(() => {
        cleanup();
        vi.restoreAllMocks();
    });

    it("moves a tab to where it is released", () => {
        const { onReorder, tab } = renderStrip();

        fireEvent.pointerDown(tab("a"), { button: 0, clientX: 10, clientY: 10 });
        fireEvent.pointerMove(window, { clientX: 260, clientY: 10 });
        fireEvent.pointerUp(window, { clientX: 260, clientY: 10 });

        expect(onReorder).toHaveBeenCalledWith("a", "c", "after");
    });

    it("marks the dragged tab and the gap it will land in", () => {
        const { tab } = renderStrip();

        fireEvent.pointerDown(tab("a"), { button: 0, clientX: 10, clientY: 10 });
        fireEvent.pointerMove(window, { clientX: 210, clientY: 10 });

        expect(tab("a").closest(".tab-wrap")).toHaveClass("tab-drag-source");
        expect(tab("c").closest(".tab-wrap")).toHaveClass("tab-drop-before");
        expect(document.body).toHaveClass("is-sorting-tabs");
    });

    it("does not select the tab when the drag's release lands as a click", () => {
        const { onSelect, tab } = renderStrip();

        fireEvent.pointerDown(tab("a"), { button: 0, clientX: 10, clientY: 10 });
        fireEvent.pointerMove(window, { clientX: 260, clientY: 10 });
        fireEvent.pointerUp(window, { clientX: 260, clientY: 10 });
        fireEvent.click(tab("a"));

        expect(onSelect).not.toHaveBeenCalled();
    });

    it("treats a press that barely moves as an ordinary click", () => {
        const { onSelect, onReorder, tab } = renderStrip();

        fireEvent.pointerDown(tab("b"), { button: 0, clientX: 150, clientY: 10 });
        fireEvent.pointerMove(window, { clientX: 152, clientY: 11 });
        fireEvent.pointerUp(window, { clientX: 152, clientY: 11 });
        fireEvent.click(tab("b"));

        expect(onReorder).not.toHaveBeenCalled();
        expect(onSelect).toHaveBeenCalledWith("b");
    });

    it("abandons the drag on Escape", () => {
        const { onReorder, tab } = renderStrip();

        fireEvent.pointerDown(tab("a"), { button: 0, clientX: 10, clientY: 10 });
        fireEvent.pointerMove(window, { clientX: 260, clientY: 10 });
        fireEvent.keyDown(window, { key: "Escape" });
        fireEvent.pointerUp(window, { clientX: 260, clientY: 10 });

        expect(onReorder).not.toHaveBeenCalled();
        expect(tab("a").closest(".tab-wrap")).not.toHaveClass("tab-drag-source");
        expect(document.body).not.toHaveClass("is-sorting-tabs");
    });

    it("leaves a strip without a reorder handler fixed", () => {
        const { tab } = renderStrip({ onReorder: undefined });

        fireEvent.pointerDown(tab("a"), { button: 0, clientX: 10, clientY: 10 });
        fireEvent.pointerMove(window, { clientX: 260, clientY: 10 });

        expect(tab("a").closest(".tab-wrap")).not.toHaveClass("tab-drag-source");
    });

    it("honours a drop the owner rules out", () => {
        const { onReorder, tab } = renderStrip({ canReorder: () => false });

        fireEvent.pointerDown(tab("a"), { button: 0, clientX: 10, clientY: 10 });
        fireEvent.pointerMove(window, { clientX: 260, clientY: 10 });
        fireEvent.pointerUp(window, { clientX: 260, clientY: 10 });

        expect(onReorder).not.toHaveBeenCalled();
    });
});
