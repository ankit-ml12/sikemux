import { useStore } from "../state/store";
import type { Agent, ProviderProfile } from "../state/types";

type Named = Pick<Agent, "name" | "title"> & Partial<Pick<Agent, "type" | "profileId" | "renamed">>;

/** An agent's name as people see it and @mention it. */
export function agentHandle(name: string): string {
    return `@${name}`;
}

/** What an agent is called and its @name beside it. Until it has a title of its own, as the core judges one (not
    empty, its kind or its account's name), its @name is the title and nothing sits beside it. */
export function agentNaming(agent: Named, profiles: readonly ProviderProfile[]): { title: string; handle?: string } {
    if (!agent.name) return { title: agent.title };
    const handle = agentHandle(agent.name);
    const title = agent.title.trim();
    const account = agent.profileId ? profiles.find((profile) => profile.id === agent.profileId)?.name : undefined;
    if (!agent.renamed && (title === "" || title === agent.type || title === account)) return { title: handle };
    return { title: agent.title, handle };
}

/** An agent as its rail row and header show it: what it is working on, then its name, dimmed. */
export function AgentLabel({ agent }: { agent: Named }) {
    const profiles = useStore((s) => s.providerProfiles);
    const { title, handle } = agentNaming(agent, profiles);
    if (!handle) return <>{title}</>;
    return (
        <>
            <span className="agent-label-title">{title}</span>
            <span className="agent-label-name">{handle}</span>
        </>
    );
}
