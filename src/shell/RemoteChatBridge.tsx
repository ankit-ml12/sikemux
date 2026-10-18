import { useEffect } from "react";
import { agentBoardApi } from "../api/agentBoard";
import type { ChatInfo } from "../api/generated/wire";
import { runOfAgent } from "../agents/agentBoard";
import { getIpcTransport } from "../api/transport";
import * as cmd from "../state/commands";
import { getState } from "../state/store";
import { swallow } from "../state/toast";

export const REMOTE_CHAT_BEGUN_EVENT = "remote_chat_begun";
export const REMOTE_CHAT_WAKE_EVENT = "remote_chat_wake";
export const FOCUS_AGENT_EVENT = "focus_agent";

/** A chat the core starts for a board's Auto is placed by its card, which the board may not have brought yet. */
async function adopt(chat: ChatInfo): Promise<void> {
    if (chat.startedBy === null && !runOfAgent(getState().agentBoards, chat.agentId)) {
        await agentBoardApi
            .all()
            .then(cmd.setAgentBoards)
            .catch(() => {});
    }
    cmd.adoptChat(chat);
}

function wake(agentId: string) {
    if (getState().agents[agentId]?.launchState === "dormant") cmd.resumeAgent(agentId);
}

/**
 * Shows a chat a paired phone or a board's Auto starts among its project's agents, wakes a sleeping one a phone opens, and shows the agent the
 * notch asks for.
 */
export function RemoteChatBridge() {
    useEffect(() => {
        const controller = new AbortController();
        const transport = getIpcTransport();
        Promise.all([
            transport.subscribe<ChatInfo>(REMOTE_CHAT_BEGUN_EVENT, ({ payload }) => void adopt(payload), { signal: controller.signal }),
            transport.subscribe<string>(REMOTE_CHAT_WAKE_EVENT, (event) => wake(event.payload), { signal: controller.signal }),
            transport.subscribe<string>(FOCUS_AGENT_EVENT, (event) => cmd.revealAgent(event.payload), { signal: controller.signal }),
        ]).catch((error: unknown) => {
            if (!controller.signal.aborted) swallow("remote chat listener")(error);
        });
        return () => controller.abort();
    }, []);
    return null;
}
