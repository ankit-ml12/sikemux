import { type ReactNode, useMemo } from "react";
import { agentDisplayOf, agentIdsOf, openAgents } from "../state/selectors";
import { useStore } from "../state/store";
import type { Agent, AgentRailScope, AgentType } from "../state/types";
import { AgentIcon, IconAgent, IconAgents, IconCommand, IconMessage } from "../ui/Icons";

/** What is running in the rail's scope: every agent with its subagents, the agents alone, their subagents and their shells. */
export function AgentRailStats({ scope, provider }: { scope: AgentRailScope; provider: AgentType | null }) {
    const activeSessionId = useStore((s) => s.activeSessionId);
    const sessionOrder = useStore((s) => s.sessionOrder);
    const sessions = useStore((s) => s.sessions);
    const windows = useStore((s) => s.windows);
    const windowsBySession = useStore((s) => s.windowsBySession);
    const agents = useStore((s) => s.agents);
    const agentActivity = useStore((s) => s.agentActivity);
    const agentTasks = useStore((s) => s.agentTasks);
    const agentSubagents = useStore((s) => s.agentSubagents);

    const stats = useMemo(() => {
        const inScope: Agent[] =
            scope === "all"
                ? [...openAgents({ sessionOrder, sessions, windows, windowsBySession, agents })].map((open) => open.agent)
                : (agentIdsOf({ windowsBySession, windows }, activeSessionId)
                      .map((id) => agents[id])
                      .filter(Boolean) as Agent[]);
        const shown = provider ? inScope.filter((agent) => agent.type === provider) : inScope;
        let subagents = 0;
        let shells = 0;
        for (const agent of shown) {
            const display = agentDisplayOf({ agentActivity, agentTasks, agentSubagents }, agent.id);
            subagents += display.subagents;
            shells += display.tasks;
        }
        const types = [...new Set(shown.map((agent) => agent.type))];
        return { agents: shown.length, subagents, shells, types };
    }, [scope, provider, activeSessionId, sessionOrder, sessions, windows, windowsBySession, agents, agentActivity, agentTasks, agentSubagents]);

    return (
        <div className="agent-stats">
            <Stat kind="all" count={stats.agents + stats.subagents} what="agents and subagents">
                <IconAgents size={14} />
            </Stat>
            <Stat kind="agents" count={stats.agents} what="agents">
                {stats.types.length > 0 ? (
                    <span className="proj-child-icons">
                        {stats.types.map((type) => (
                            <span key={type} className={`proj-pip proj-pip-${type}`}>
                                <AgentIcon type={type} size={14} />
                            </span>
                        ))}
                    </span>
                ) : (
                    <IconMessage size={14} />
                )}
            </Stat>
            <Stat kind="subagents" count={stats.subagents} what="subagents running">
                <IconAgent size={14} />
            </Stat>
            <Stat kind="shells" count={stats.shells} what="shells and monitors running">
                <IconCommand size={14} />
            </Stat>
        </div>
    );
}

function Stat({ kind, count, what, children }: { kind: string; count: number; what: string; children: ReactNode }) {
    const label = `${count} ${what}`;
    return (
        <span className={`agent-stat agent-stat-${kind}${count === 0 ? " is-empty" : ""}`} role="img" aria-label={label} title={label}>
            {children}
            {count > 0 && <b>{count}</b>}
        </span>
    );
}
