import type { AgentRailScope } from "../state/types";

/** This project, by name, or every open project, each with how many agents it has open. */
export function ScopeTabs({
    scope,
    projectName,
    counts,
    waitingElsewhere,
    onChange,
}: {
    scope: AgentRailScope;
    projectName: string;
    counts: { project: number; all: number };
    waitingElsewhere: number;
    onChange: (scope: AgentRailScope) => void;
}) {
    return (
        <div className="scope-tabs" role="tablist" aria-label="Which projects">
            <button
                type="button"
                role="tab"
                aria-selected={scope === "project"}
                aria-label={`${projectName}, ${counts.project} open`}
                onClick={() => onChange("project")}>
                <span className="scope-name">{projectName}</span>
                <span className="scope-n">{counts.project}</span>
            </button>
            <button
                type="button"
                role="tab"
                aria-selected={scope === "all"}
                aria-label={`All projects, ${counts.all} open${waitingElsewhere > 0 ? `, ${waitingElsewhere} waiting` : ""}`}
                onClick={() => onChange("all")}>
                All projects
                <span className="scope-n">{counts.all}</span>
                {waitingElsewhere > 0 && <span className="scope-count">{waitingElsewhere}</span>}
            </button>
        </div>
    );
}
