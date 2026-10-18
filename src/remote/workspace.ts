import { agentLabel } from "../agents/catalog";
import { CHAT_AGENT_TYPES } from "../agents/agentLaunch";
import type { AgentType, ProviderProfile } from "../state/types";

/** The launcher the app offers for a provider and profile, as devices and the core name it. */
export const launcherId = (type: AgentType, profileId?: string) => (profileId ? `${type}:${profileId}` : type);

/** The chat agents the core offers, as its launchers name them: one per profile, or one for a provider with none. */
export function chatLaunchers(profiles: readonly ProviderProfile[]): { id: string; label: string }[] {
    return CHAT_AGENT_TYPES.flatMap((type) => {
        const own = profiles.filter((profile) => profile.provider === type);
        if (own.length === 0) return [{ id: launcherId(type), label: agentLabel(type) }];
        return own.map((profile) => ({
            id: launcherId(type, profile.id),
            label: own.length > 1 ? `${agentLabel(type)} · ${profile.name}` : agentLabel(type),
        }));
    });
}

/** The provider profile a chat was started with, read back from its launcher's id. */
export function profileOfLauncher(launcher: string | null, profiles: readonly ProviderProfile[], type: AgentType): string | undefined {
    const profileId = launcher?.startsWith(`${type}:`) ? launcher.slice(type.length + 1) : undefined;
    return profiles.some((profile) => profile.id === profileId && profile.provider === type) ? profileId : undefined;
}
