import { useEffect, useRef, type ReactNode, type Ref, type UIEventHandler } from "react";

const MIN_THUMB = 24;
const LINGER_MS = 700;

/**
 * A rail's scrolling list, with a thin thumb drawn over the rows while it
 * scrolls. The native bar is hidden: styled, it takes a column of the rail's
 * width, and unstyled, macOS widens it under the pointer.
 */
export function RailScroll({
    ref,
    onScroll,
    children,
}: {
    ref?: Ref<HTMLDivElement>;
    onScroll?: UIEventHandler<HTMLDivElement>;
    children: ReactNode;
}) {
    const box = useRef<HTMLDivElement | null>(null);
    const thumb = useRef<HTMLDivElement>(null);

    useEffect(() => {
        const el = box.current!;
        const bar = thumb.current!;
        let hideTimer = 0;
        let drag: { pointerY: number; scrollTop: number; scale: number } | null = null;

        const thumbHeight = () => Math.max(MIN_THUMB, (el.clientHeight * el.clientHeight) / el.scrollHeight);
        const place = () => {
            const travel = el.scrollHeight - el.clientHeight;
            if (travel <= 0) return false;
            const height = thumbHeight();
            const top = el.scrollTop + (el.scrollTop / travel) * (el.clientHeight - height);
            bar.style.height = `${height}px`;
            bar.style.transform = `translateY(${top}px)`;
            return true;
        };
        const hideLater = () => {
            clearTimeout(hideTimer);
            hideTimer = window.setTimeout(() => {
                if (!drag) bar.dataset.shown = "false";
            }, LINGER_MS);
        };
        const reveal = () => {
            if (!place()) return;
            bar.dataset.shown = "true";
            hideLater();
        };
        const grab = (event: PointerEvent) => {
            event.preventDefault();
            bar.setPointerCapture(event.pointerId);
            const travel = el.scrollHeight - el.clientHeight;
            drag = { pointerY: event.clientY, scrollTop: el.scrollTop, scale: travel / (el.clientHeight - thumbHeight()) };
        };
        const move = (event: PointerEvent) => {
            if (drag) el.scrollTop = drag.scrollTop + (event.clientY - drag.pointerY) * drag.scale;
        };
        const release = () => {
            drag = null;
            hideLater();
        };

        el.addEventListener("scroll", reveal, { passive: true });
        bar.addEventListener("pointerdown", grab);
        bar.addEventListener("pointermove", move);
        bar.addEventListener("pointerup", release);
        bar.addEventListener("pointercancel", release);
        return () => {
            clearTimeout(hideTimer);
            el.removeEventListener("scroll", reveal);
            bar.removeEventListener("pointerdown", grab);
            bar.removeEventListener("pointermove", move);
            bar.removeEventListener("pointerup", release);
            bar.removeEventListener("pointercancel", release);
        };
    }, []);

    const setBox = (el: HTMLDivElement | null) => {
        box.current = el;
        if (typeof ref === "function") ref(el);
        else if (ref) ref.current = el;
    };

    return (
        <div className="rail-scroll" ref={setBox} onScroll={onScroll}>
            {children}
            <div className="rail-scroll-thumb" ref={thumb} data-shown="false" aria-hidden />
        </div>
    );
}
