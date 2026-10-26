import type { CatalogAgent } from "../wire.generated";
import { externalValue, useExternalValue } from "../lib/externalValue";

export type { CatalogAgent };

/** An agent's id in the core's catalog, such as `claude`. */
export type AgentType = string;

/** The agents the core said it can run, read before anything shows an agent. */
const catalog = externalValue<ReadonlyMap<string, CatalogAgent>>(new Map());

/** Takes the catalog a core sent; agents it does not list keep what an earlier core said of them. */
export function setAgentCatalog(agents: readonly CatalogAgent[]): void {
    const byType = new Map(catalog.get());
    for (const agent of agents) byType.set(agent.type, agent);
    catalog.set(byType);
}

const entry = (type: string | null | undefined) => (type ? catalog.get().get(type) : undefined);

export function isAgentType(value: unknown): value is AgentType {
    return typeof value === "string" && !!entry(value);
}

/** The agent's name; anything not in the catalog keeps the id it came with. */
export function agentLabel(type: string): string {
    return entry(type)?.label ?? type;
}

/** The agent's colour, drawing again once a core has said it. */
export const useAgentBrand = (type: string): string | undefined => useExternalValue(catalog, (byType) => byType.get(type)?.brand);

/** Whether the agent's CLI can start without asking for permissions. */
export function canBypassPermissions(type: AgentType): boolean {
    return entry(type)?.bypass === true;
}
