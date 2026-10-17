import { type ComponentType, type ReactNode, useEffect, useLayoutEffect, useRef, useState } from "react";
import * as cmd from "../state/commands";
import { useStore } from "../state/store";
import type { AgentRailPage } from "../state/types";
import { claimsWheel, dragOffset, landing, scrollersUnder, springHome, watchSwipes } from "../lib/swipe";
import type { SwipeHandle } from "../lib/swipe";
import { IconAgents, IconFile, IconTerminal } from "../ui/Icons";
import { Tooltip } from "../ui/Tooltip";

const PAGES: readonly { page: AgentRailPage; label: string; Icon: ComponentType<{ size?: number }> }[] = [
    { page: "agents", label: "Agents", Icon: IconAgents },
    { page: "terminals", label: "Terminals", Icon: IconTerminal },
    { page: "files", label: "Files", Icon: IconFile },
];

const pageTransform = (at: number) => `translate3d(${-at * 100}%, 0, 0)`;

/**
 * The agent rail's pages side by side, chosen from the icons at its foot or
 * swiped between with two fingers, one page per swipe. The track is moved by
 * hand, never by a render, so a swipe costs no React work until it has landed.
 */
export function RailPages({ pages }: { pages: Record<AgentRailPage, ReactNode> }) {
    const page = useStore((s) => s.agentRailPage);
    const index = Math.max(
        0,
        PAGES.findIndex((entry) => entry.page === page),
    );
    const [firstIndex] = useState(index);
    const rootRef = useRef<HTMLDivElement>(null);
    const viewRef = useRef<HTMLDivElement>(null);
    const trackRef = useRef<HTMLDivElement>(null);
    /** Where the track is, where it is going, and how to finish getting there. */
    const motion = useRef({ at: index, target: index, finish: null as (() => void) | null });

    const place = (at: number) => {
        motion.current.at = at;
        if (trackRef.current) trackRef.current.style.transform = pageTransform(at);
    };
    const placeRef = useRef(place);
    placeRef.current = place;

    const glide = (target: number, velocity: number) => {
        const m = motion.current;
        const view = viewRef.current;
        m.finish?.();
        m.target = target;
        if (view) view.dataset.moving = "";
        m.finish = springHome(
            m.at - target,
            velocity,
            (at) => placeRef.current(target + at),
            () => {
                m.finish = null;
                if (view) delete view.dataset.moving;
            },
        );
    };
    const glideRef = useRef(glide);
    glideRef.current = glide;

    useLayoutEffect(() => {
        if (motion.current.target !== index) glideRef.current(index, 0);
    }, [index]);

    useEffect(() => {
        const root = rootRef.current;
        const view = viewRef.current;
        if (!root || !view) return;
        const start = (event: WheelEvent): SwipeHandle | null => {
            if (!claimsWheel(scrollersUnder(event.target, ".rail-page, .rail-pages-switch"), event.deltaX, event.deltaY)) return null;
            const width = view.clientWidth;
            if (width <= 0) return null;
            const m = motion.current;
            m.finish?.();
            const base = m.target;
            const ends = { hasPrevious: base > 0, hasNext: base < PAGES.length - 1 };
            let raw = 0;
            let offset = 0;
            let frame: number | null = null;
            view.dataset.moving = "";
            return {
                move(pixels) {
                    raw += pixels / width;
                    offset = dragOffset(raw, ends);
                    frame ??= requestAnimationFrame(() => {
                        frame = null;
                        placeRef.current(base + offset);
                    });
                },
                release(speed) {
                    if (frame != null) cancelAnimationFrame(frame);
                    const target = base + landing(offset, speed, ends);
                    placeRef.current(base + offset);
                    glideRef.current(target, speed / width);
                    if (target !== base) cmd.setAgentRailPage(PAGES[target].page);
                },
            };
        };
        return watchSwipes(root, start);
    }, []);

    return (
        <div className="rail-pages" ref={rootRef}>
            <div className="rail-pager" ref={viewRef}>
                <div className="rail-pages-track" ref={trackRef} style={{ transform: pageTransform(firstIndex) }}>
                    {PAGES.map((entry, at) => (
                        <section key={entry.page} className={`rail-page${at === index ? " is-current" : ""}`} aria-label={entry.label}>
                            {pages[entry.page]}
                        </section>
                    ))}
                </div>
            </div>
            <div className="rail-pages-switch" role="tablist" aria-label="Rail pages">
                {PAGES.map(({ page: entry, label, Icon }) => (
                    <Tooltip key={entry} label={label}>
                        <button
                            type="button"
                            role="tab"
                            aria-selected={entry === page}
                            aria-label={label}
                            onClick={() => cmd.setAgentRailPage(entry)}>
                            <Icon size={15} />
                        </button>
                    </Tooltip>
                ))}
            </div>
        </div>
    );
}
