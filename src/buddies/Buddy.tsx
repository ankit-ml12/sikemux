import { memo, useEffect, useMemo, useState } from "react";
import { useStore } from "../state/store";
import type { Agent } from "../state/types";
import { buddyStateOf, resolveBuddy, type BuddyPack, type BuddyShape, type BuddyState } from "./catalogue";
import { assignBuddyColors } from "./commands";
import { buddyMarkup } from "./render";
import "./buddies.css";

let nextUid = 0;

export const BuddyArt = memo(function BuddyArt({
    pack,
    shape,
    color,
    state,
    size,
    className,
}: {
    pack: BuddyPack;
    shape: BuddyShape;
    color: string;
    state: BuddyState;
    size: number;
    className?: string;
}) {
    const [uid] = useState(() => `bd${++nextUid}`);
    const still = !useStore((s) => s.buddyMotion);
    const html = useMemo(() => buddyMarkup(pack, shape, color, state, uid), [pack, shape, color, state, uid]);
    return (
        <span
            className={`buddy${still ? " still" : ""}${className ? ` ${className}` : ""}`}
            style={{ width: size, height: size }}
            dangerouslySetInnerHTML={{ __html: html }}
        />
    );
});

/** An agent's character, showing what the agent is doing unless `state` says otherwise. */
export const AgentBuddy = memo(function AgentBuddy({
    agent,
    size,
    state,
    className,
}: {
    agent: Pick<Agent, "id" | "buddy">;
    size: number;
    state?: BuddyState;
    className?: string;
}) {
    const live = useStore((s) => (state ? undefined : s.agentActivity[agent.id]?.state));
    const pack = useStore((s) => s.buddyPack);
    const shape = useStore((s) => s.buddyShape);
    const look = useMemo(() => resolveBuddy(agent, { pack, shape }), [agent, pack, shape]);
    return <BuddyArt pack={look.pack} shape={look.shape} color={look.color} state={state ?? buddyStateOf(live)} size={size} className={className} />;
});

/** Gives each new agent its colour. Mounted once, at the app's root. */
export function BuddyBridge() {
    const agents = useStore((s) => s.agents);
    useEffect(() => {
        if (Object.values(agents).some((agent) => !agent.buddy)) assignBuddyColors();
    }, [agents]);
    return null;
}
