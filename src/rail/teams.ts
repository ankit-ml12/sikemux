import { useCallback, useState } from "react";
import type { TeamMember, TeamRow } from "../agents/teams";

/** How a lead's row folds its team: whether the workers show, and how many there are. */
export interface TeamFold {
    open: boolean;
    count: number;
    toggle: () => void;
}

export type FoldedRow<T extends TeamMember> = TeamRow<T> & { fold?: TeamFold };

/** Which leads have their teams folded away, for one list of agents. */
export function useTeamFolds(): { folded: ReadonlySet<string>; toggle: (lead: string) => void } {
    const [folded, setFolded] = useState<ReadonlySet<string>>(new Set());
    const toggle = useCallback(
        (lead: string) =>
            setFolded((now) => {
                const next = new Set(now);
                if (!next.delete(lead)) next.add(lead);
                return next;
            }),
        [],
    );
    return { folded, toggle };
}

/** The rows with each lead's fold on it, and a folded lead's workers left out. */
export function foldTeams<T extends TeamMember>(
    rows: readonly TeamRow<T>[],
    folded: ReadonlySet<string>,
    toggle: (lead: string) => void,
): FoldedRow<T>[] {
    const shown: FoldedRow<T>[] = [];
    let lead: string | null = null;
    rows.forEach((row, index) => {
        if (row.link === "head") {
            lead = row.agent.id;
            const id = lead;
            let count = 0;
            for (let next = index + 1; next < rows.length && (rows[next].link === "member" || rows[next].link === "last"); next++) count++;
            shown.push({ ...row, fold: { open: !folded.has(id), count, toggle: () => toggle(id) } });
        } else if (row.link === "member" || row.link === "last") {
            if (!lead || !folded.has(lead)) shown.push(row);
        } else {
            lead = null;
            shown.push(row);
        }
    });
    return shown;
}
