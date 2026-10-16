import type { AgentRailScope } from "../state/types";
import { IconFolder, IconStack } from "../ui/Icons";

/** This project, by name, or every open project. */
export function ScopeTabs({
    scope,
    projectName,
    waitingElsewhere,
    onChange,
}: {
    scope: AgentRailScope;
    projectName: string;
    waitingElsewhere: number;
    onChange: (scope: AgentRailScope) => void;
}) {
    return (
        <div className="scope-tabs" role="tablist" aria-label="Which projects">
            <button type="button" role="tab" aria-selected={scope === "project"} aria-label={projectName} onClick={() => onChange("project")}>
                <IconFolder size={13} />
                <span className="scope-name">{projectName}</span>
            </button>
            <button
                type="button"
                role="tab"
                aria-selected={scope === "all"}
                aria-label={waitingElsewhere > 0 ? `All projects, ${waitingElsewhere} waiting` : "All projects"}
                onClick={() => onChange("all")}>
                <IconStack size={13} />
                All projects
                {waitingElsewhere > 0 && <span className="scope-count">{waitingElsewhere}</span>}
            </button>
        </div>
    );
}
