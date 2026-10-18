import { acpApi } from "../api/acp";
import { agentApi } from "../api/agents";
import { profileOf } from "./commands/agentLaunchCommand";
import type { StoreState } from "./store";

/* The GUI hears about a shell or a monitor over its own connection, but a TUI
   agent keeps that to itself. Claude Code answers for its own sessions, and a
   session reads idle only when a turn, a question and every background shell
   are all finished, so idle is the one answer that lets the agent go. */
export async function agentIdsWithLiveSessions(state: StoreState, ids: readonly string[]): Promise<Set<string>> {
    const live = await chatsAtWork(ids);
    const asking = ids.map((id) => state.agents[id]).filter((agent) => agent?.type === "claude" && agent.resumeId && agent.launchState !== "dormant");
    if (asking.length === 0) return live;

    const byConfig = new Map<string, typeof asking>();
    for (const agent of asking) {
        const configPath = profileOf(state, agent)?.configPath;
        const group = byConfig.get(configPath ?? "");
        if (group) group.push(agent);
        else byConfig.set(configPath ?? "", [agent]);
    }

    await Promise.all(
        [...byConfig].map(async ([configPath, group]) => {
            const rows = await agentApi.liveSessions(configPath || undefined).catch(() => null);
            // Claude Code could not say, so nothing here overrides what the
            // screen already reported.
            if (!rows) return;
            const status = new Map(rows.map((row) => [row.sessionId, row.status]));
            for (const agent of group) {
                const reported = agent.resumeId ? status.get(agent.resumeId) : undefined;
                if (reported !== undefined && reported !== "idle") live.add(agent.id);
            }
        }),
    );
    return live;
}

/* A chat pane taken up mid-turn can read idle while the core is still running
   the turn, so the core has the last word on its own chats. */
async function chatsAtWork(ids: readonly string[]): Promise<Set<string>> {
    const asked = new Set(ids);
    const chats = await acpApi.list().catch(() => []);
    return new Set(
        chats.filter((chat) => asked.has(chat.agentId) && (chat.running || chat.pendingPermissions.length > 0)).map((chat) => chat.agentId),
    );
}
