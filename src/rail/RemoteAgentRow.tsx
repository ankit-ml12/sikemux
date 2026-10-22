import { agentHandle } from "../agents/AgentLabel";
import { AgentMark } from "../agents/AgentMark";
import { AgentStateIndicator, showsAgentState } from "../agents/AgentStateIndicator";
import { isAgentType } from "../agents/catalog";
import type { TeamLink, TeamMember } from "../agents/teams";
import type { RemoteAgent } from "../api/inbox";
import { IconAgent } from "../ui/Icons";

export interface RemoteWorker extends TeamMember {
    readonly remote: RemoteAgent;
}

/** The rows, then the workers their agents started on linked hosts, ready for `inTeams` to place under their leads. */
export function withRemoteWorkers<T extends TeamMember>(rows: readonly T[], remote: readonly RemoteAgent[]): (T | RemoteWorker)[] {
    const leads = new Set(rows.map((row) => row.id));
    const workers = remote
        .filter((agent) => agent.lead && leads.has(agent.lead))
        .map((agent) => ({ id: agent.agentId, lead: agent.lead, remote: agent }));
    return [...rows, ...workers];
}

export function isRemoteWorker(row: TeamMember): row is RemoteWorker {
    return "remote" in row;
}

/** A lead's row heads its team's group, and its workers hang under it on the group's spine. */
export function teamWrapClass(link: TeamLink | undefined, open = true): string {
    return `agent-row-wrap${link === "head" ? ` team-head${open ? " open" : ""}` : link ? ` team-member${link === "last" ? " team-last" : ""}` : ""}`;
}

/** A worker on a linked host, in its lead's team. It has no chat here to open. */
export function RemoteAgentRow({ agent, link }: { agent: RemoteAgent; link?: TeamLink }) {
    return (
        <div className={teamWrapClass(link)}>
            <div className="agent-row remote" title={agent.reachable ? `Runs on ${agent.host}` : `${agent.host} is out of reach`}>
                <span className={`agent-glyph ${agent.cli}`}>
                    {isAgentType(agent.cli) ? <AgentMark agentId={agent.agentId} cli={agent.cli} size={20} /> : <IconAgent size={20} />}
                </span>
                <span className="agent-title">
                    <span className="agent-name">{agentHandle(agent.name)}</span>{" "}
                    <span className="agent-name-title">
                        on {agent.host}
                        {!agent.reachable && ", out of reach"}
                    </span>
                </span>
            </div>
            {agent.reachable && showsAgentState(agent) && (
                <span className="row-status">
                    <AgentStateIndicator display={agent} />
                </span>
            )}
        </div>
    );
}
