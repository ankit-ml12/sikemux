import { useEffect } from "react";
import { getIpcTransport } from "../api/transport";
import * as cmd from "../state/commands";
import { swallow } from "../state/toast";

export const FOCUS_AGENT_EVENT = "focus_agent";

/** Shows the agent the notch asks for. */
export function RemoteChatBridge() {
    useEffect(() => {
        const controller = new AbortController();
        const transport = getIpcTransport();
        transport
            .subscribe<string>(FOCUS_AGENT_EVENT, (event) => cmd.revealAgent(event.payload), { signal: controller.signal })
            .catch((error: unknown) => {
                if (!controller.signal.aborted) swallow("remote chat listener")(error);
            });
        return () => controller.abort();
    }, []);
    return null;
}
