import { useEffect } from "react";
import { getIpcTransport } from "../api/transport";
import * as cmd from "../state/commands";
import { getState } from "../state/store";
import { swallow } from "../state/toast";

export const REMOTE_CHAT_WAKE_EVENT = "remote_chat_wake";
export const FOCUS_AGENT_EVENT = "focus_agent";

function wake(agentId: string) {
    if (getState().agents[agentId]?.launchState === "dormant") cmd.resumeAgent(agentId);
}

/** Wakes a sleeping chat a phone opens, and shows the agent the notch asks for. */
export function RemoteChatBridge() {
    useEffect(() => {
        const controller = new AbortController();
        const transport = getIpcTransport();
        Promise.all([
            transport.subscribe<string>(REMOTE_CHAT_WAKE_EVENT, (event) => wake(event.payload), { signal: controller.signal }),
            transport.subscribe<string>(FOCUS_AGENT_EVENT, (event) => cmd.revealAgent(event.payload), { signal: controller.signal }),
        ]).catch((error: unknown) => {
            if (!controller.signal.aborted) swallow("remote chat listener")(error);
        });
        return () => controller.abort();
    }, []);
    return null;
}
