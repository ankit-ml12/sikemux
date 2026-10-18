import { acpApi } from "../api/acp";
import { coreSessionsApi } from "../api/coreSessions";
import type { ChatInfo, SessionInfo } from "../api/generated/wire";
import { QUIT_AND_STOP_SHORTCUT } from "../commands/keybindings";
import * as cmd from "../state/commands";
import { agentSessionPlan, claimedSessionIds, unclaimedTerminals } from "../state/coreSessionClaims";
import { getState, setState } from "../state/store";
import { notify } from "../state/toast";
import { offerResumableSessions, spawnedThisPage } from "../terminal/sessionResume";
import { relaunchTuiAgent } from "../agents/tuiResume";

/** Long enough for every pane of a restored layout to have taken its terminal back. */
export const UNCLAIMED_GRACE_MS = 30_000;

export const KEPT_RUNNING_NOTICE = `Your terminals and agents kept running while Sikemux was closed. Use Quit and Stop Everything${
    QUIT_AND_STOP_SHORTCUT ? ` (${QUIT_AND_STOP_SHORTCUT})` : ""
} to stop them.`;

/** Call right after the saved layout is applied, before any pane mounts. */
export function offerSavedSessions(): void {
    offerResumableSessions(claimedSessionIds(getState()));
}

export interface CoreSessionRestoreDeps {
    list(): Promise<SessionInfo[]>;
    resume(agentId: string, ptyId: number): Promise<void>;
    kill(id: number): Promise<void>;
    chats(): Promise<ChatInfo[]>;
    schedule(callback: () => void, delayMs: number): void;
}

function defaultDeps(): CoreSessionRestoreDeps {
    return {
        list: coreSessionsApi.list,
        resume: relaunchTuiAgent,
        kill: coreSessionsApi.kill,
        chats: acpApi.list,
        schedule: (callback, delayMs) => void window.setTimeout(callback, delayMs),
    };
}

/**
 * Takes back what the core kept while the app was closed or reloading:
 * terminal agents still running come back live, ones that crashed meanwhile
 * are resumed in their pane, and terminals from before this page that nothing
 * in the layout names are stopped after a grace, so none of them runs forever
 * unseen. Chats are the core's: their panes take them up as they mount.
 */
export async function restoreCoreSessions(deps: CoreSessionRestoreDeps = defaultDeps()): Promise<void> {
    let sessions: SessionInfo[];
    try {
        sessions = await deps.list();
    } catch {
        return;
    }
    const chats = await deps.chats().catch(() => []);
    const plan = agentSessionPlan(getState(), sessions);
    cmd.applyAgentSessionPlan(plan);
    for (const id of plan.resume) {
        const ptyId = getState().agents[id]?.ptyId;
        if (ptyId !== undefined) void deps.resume(id, ptyId).catch(() => {});
    }
    const tasks = sessions.filter((session) => session.kind === "task" && session.running).length;

    const claimed = claimedSessionIds(getState());
    const earlier = sessions.filter((session) => !spawnedThisPage(session.id));
    const terminals = earlier.filter((session) => session.kind === "terminal" && session.running && claimed.has(session.id)).length;
    const agents = getState().agents;
    const keptChats = chats.filter((chat) => agents[chat.agentId]).length;
    if (terminals + tasks + keptChats > 0 && !getState().keptRunningNoticeShown) {
        setState({ keptRunningNoticeShown: true });
        notify("info", KEPT_RUNNING_NOTICE, { timeoutMs: 12_000 });
    }

    const candidates = unclaimedTerminals(earlier, claimed);
    if (candidates.length === 0) return;
    deps.schedule(() => {
        const stillClaimed = claimedSessionIds(getState());
        for (const id of candidates) if (!stillClaimed.has(id)) void deps.kill(id).catch(() => {});
    }, UNCLAIMED_GRACE_MS);
}
