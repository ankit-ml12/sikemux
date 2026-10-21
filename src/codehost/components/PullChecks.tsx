import { useResourceEnabled } from "../../plugin-api/resources";
import type { RepoRef } from "../api";
import { useMemo } from "react";
import { runListR } from "../lists";
import { checksSummary, elapsedMs, eventLabel, isUnfinished, outcomeOf, OUTCOME_LABEL, overallOutcome } from "../runStatus";
import { StatusMark } from "./ActionsIcon";
import { useNow } from "../../plugin-api/hooks";
import { useCiWatch } from "../ciWatch";
import { formatDuration } from "../../lib/format";

interface Props {
    repo: RepoRef;
    project: string;
    sha: string;
    active: boolean;
    onOpenRun: (runId: string) => void;
}

/** The checks part of a pull request's merge box, one row per workflow run on its head commit. */
export function PullChecks({ repo, project, sha, active, onOpenRun }: Props) {
    const found = useResourceEnabled(active, runListR, project, { headSha: sha, perPage: 30 });
    const runs = useMemo(() => found.data?.runs.map(({ run }) => run) ?? [], [found.data]);
    const live = active && runs.some(isUnfinished);
    const now = useNow(live);
    useCiWatch(repo, active);
    if (runs.length === 0) return null;

    const summary = checksSummary(runs);
    return (
        <div className="gha-merge-part">
            <div className="gha-merge-row">
                <StatusMark outcome={overallOutcome(runs)} size={14} />
                <span className="gha-merge-title">{summary === "all passed" ? "All checks passed" : `Checks: ${summary}`}</span>
            </div>
            <div className="gha-checks">
                {runs.map((run) => {
                    const outcome = outcomeOf(run);
                    const going = isUnfinished(run);
                    return (
                        <button key={run.id} type="button" className="gha-check" data-outcome={outcome} onClick={() => onOpenRun(run.id)}>
                            <StatusMark outcome={outcome} size={13} />
                            <span className="gha-check-name">
                                {run.name} <span className="gha-item-number gha-dim">#{run.runNumber}</span>
                            </span>
                            <span className="gha-dim">{eventLabel(run.event)}</span>
                            <span className="gha-check-spacer" />
                            <span className="gha-dim">{OUTCOME_LABEL[outcome]}</span>
                            <span className="gha-check-took">
                                {formatDuration(elapsedMs(run.startedAt ?? run.createdAt, going ? null : run.updatedAt, now))}
                            </span>
                        </button>
                    );
                })}
            </div>
        </div>
    );
}
