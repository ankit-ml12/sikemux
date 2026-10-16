import { useEffect, useRef, useState } from "react";
import { useModalFocus } from "../hooks/useModalFocus";
import { useOccludeNativeViews } from "../state/nativeViews";
import { useStore } from "../state/store";
import type { Agent } from "../state/types";
import { AgentBuddy, BuddyArt } from "./Buddy";
import { BUDDY_PACKS, resolveBuddy } from "./catalogue";
import { closeBuddyPicker, resetAgentBuddy, setAgentBuddyColor, setAgentBuddyShape, useBuddyPicker } from "./commands";

export function BuddyPickerHost() {
    const agentId = useBuddyPicker((s) => s.agentId);
    const agent = useStore((s) => (agentId ? s.agents[agentId] : undefined));
    useOccludeNativeViews(agent !== undefined);
    if (!agent) return null;
    return <BuddyPicker key={agent.id} agent={agent} />;
}

function BuddyPicker({ agent }: { agent: Agent }) {
    const modalRef = useRef<HTMLDivElement>(null);
    useModalFocus(modalRef);
    const defaultPack = useStore((s) => s.buddyPack);
    const defaultShape = useStore((s) => s.buddyShape);
    const look = resolveBuddy(agent, { pack: defaultPack, shape: defaultShape });
    const [packId, setPackId] = useState(look.pack.id);
    const pack = BUDDY_PACKS.find((candidate) => candidate.id === packId) ?? look.pack;
    const color = pack.colors[look.colorIndex % pack.colors.length];
    const chosen = agent.buddy?.pack !== undefined;

    useEffect(() => {
        const onKey = (event: KeyboardEvent) => {
            if (event.key !== "Escape") return;
            event.preventDefault();
            event.stopPropagation();
            closeBuddyPicker();
        };
        window.addEventListener("keydown", onKey, true);
        return () => window.removeEventListener("keydown", onKey, true);
    }, []);

    return (
        <div className="dlg-scrim" onMouseDown={closeBuddyPicker}>
            <div
                ref={modalRef}
                tabIndex={-1}
                className="dlg buddy-dlg"
                role="dialog"
                aria-modal="true"
                aria-label={`Buddy for ${agent.title}`}
                onMouseDown={(event) => event.stopPropagation()}>
                <div className="dlg-head">
                    <AgentBuddy agent={agent} size={32} className="buddy-dlg-current" />
                    <h2 className="dlg-title">{agent.name ? `${agent.name}'s buddy` : "Buddy"}</h2>
                </div>
                <div className="dlg-body">
                    <div className="buddy-packs" role="tablist" aria-label="Packs">
                        {BUDDY_PACKS.map((candidate) => (
                            <button
                                key={candidate.id}
                                type="button"
                                role="tab"
                                aria-selected={candidate.id === pack.id}
                                className={`buddy-chip${candidate.id === pack.id ? " on" : ""}`}
                                onClick={() => setPackId(candidate.id)}>
                                {candidate.name}
                            </button>
                        ))}
                    </div>
                    <div className="buddy-grid" role="listbox" aria-label={pack.name}>
                        {pack.shapes.map((shape) => {
                            const on = look.pack.id === pack.id && look.shape.id === shape.id;
                            return (
                                <button
                                    key={shape.id}
                                    type="button"
                                    role="option"
                                    aria-selected={on}
                                    title={shape.name}
                                    className={`buddy-tile${on ? " on" : ""}`}
                                    onClick={() => setAgentBuddyShape(agent.id, pack.id, shape.id)}>
                                    <BuddyArt pack={pack} shape={shape} color={color} state="idle" size={34} />
                                    <span>{shape.name}</span>
                                </button>
                            );
                        })}
                    </div>
                    <div className="buddy-colors" role="radiogroup" aria-label="Colour">
                        {pack.colors.map((swatch, index) => (
                            <button
                                key={swatch}
                                type="button"
                                role="radio"
                                aria-checked={index === look.colorIndex % pack.colors.length}
                                aria-label={`Colour ${index + 1}`}
                                className={`buddy-swatch${index === look.colorIndex % pack.colors.length ? " on" : ""}`}
                                onClick={() => setAgentBuddyColor(agent.id, index)}>
                                <span style={{ background: swatch }} />
                            </button>
                        ))}
                    </div>
                </div>
                <div className="dlg-foot">
                    <span className="dlg-hint">{chosen ? "Picked for this agent" : "Following the default in Settings"}</span>
                    {chosen && (
                        <button type="button" className="dlg-btn" onClick={() => resetAgentBuddy(agent.id)}>
                            Use Default
                        </button>
                    )}
                    <button type="button" className="dlg-btn primary" onClick={closeBuddyPicker}>
                        Done
                    </button>
                </div>
            </div>
        </div>
    );
}
