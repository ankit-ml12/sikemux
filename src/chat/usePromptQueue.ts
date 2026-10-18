import { useCallback, useEffect, useState, type Dispatch, type RefObject } from "react";
import { errorText } from "../lib/format";
import { acpApi, type PromptContext } from "../api/acp";
import type { Agent } from "../state/types";
import * as cmd from "../state/commands";
import { unknownCommand } from "./composerInput";
import type { OutgoingMessage, QueuedMessage } from "./queuedMessages";
import type { AcpAvailableCommand, ChatAction, ChatState } from "./types";

/* Left off when there is none, so a plain message goes out exactly as before. */
const contextArgs = (context: PromptContext[]): [] | [PromptContext[]] => (context.length ? [context] : []);

/** How long a queue waits after a turn ends, for word that it failed. */
const SETTLE_MS = 150;

export function usePromptQueue({
    agentRef,
    agentId,
    cwd,
    connection,
    running,
    replaying,
    queued,
    commands,
    steerable,
    dispatch,
    onError,
}: {
    agentRef: RefObject<Agent>;
    agentId: string;
    cwd: string;
    connection: ChatState["connection"];
    running: boolean;
    replaying: boolean;
    queued: QueuedMessage[];
    commands: AcpAvailableCommand[];
    steerable: boolean;
    dispatch: Dispatch<ChatAction>;
    onError: (message: string | null) => void;
}) {
    const [queued, setQueued] = useState<QueuedMessage[]>([]);
    const queuedCount = useRef(0);
    const [early, setEarly] = useState<{ messageId: string; message: OutgoingMessage } | null>(null);

    useEffect(() => setEarly(null), [agentId, cwd]);

    const showPrompt = useCallback(
        ({ text, paths, context }: OutgoingMessage) => {
            const messageId = crypto.randomUUID();
            dispatch({ type: "local_prompt", text, paths, context, messageId });
            cmd.titleAgentFromPrompt(agentRef.current.id, text || context[0]?.title || "");
            return messageId;
        },
        [agentRef, dispatch],
    );

    const deliver = useCallback(
        async ({ text, paths, context }: OutgoingMessage, messageId?: string) => {
            try {
                await acpApi.prompt(agentRef.current.id, text, paths, context, messageId);
            } catch (error) {
                dispatch({ type: "error", message: errorText(error) });
            }
        },
        [agentRef, dispatch],
    );

    useEffect(() => {
        if (connection !== "ready" || early === null) return;
        setEarly(null);
        void deliver(early.message, early.messageId);
    }, [connection, early, deliver]);

    const failed = (error: unknown) => dispatch({ type: "error", message: errorText(error) });

    /* Steering puts messages into what the agent has in flight, so a message only goes this way when it is asked to. */
    const steer = (messages: QueuedMessage[], message?: OutgoingMessage) =>
        acpApi.steerQueued(agentId, messages.map((queued) => queued.id), message).catch(failed);

    /** Says whether the composer may clear what it just handed over. */
    const send = (outgoing: OutgoingMessage, steerNow: boolean): boolean => {
        const unknown = unknownCommand(outgoing.text, commands);
        if (unknown) {
            onError(`/${unknown} is not available in this session`);
            return false;
        }
        onError(null);
        if (running && steerNow && steerable) {
            void steer(queued, outgoing);
            return true;
        }
        if (running) {
            void deliver(outgoing);
            return true;
        }

        /* Written mid-turn, or while the session is still coming up: it waits
           in the queue and goes out with the rest of it once the session is free. */
        queuedCount.current += 1;
        const message: QueuedMessage = { id: `queued-${queuedCount.current}`, ...outgoing };
        if (steerNow && steerable && running) {
            void steer([...queued, message]);
            return true;
        }
        onError("Wait for the agent to start");
        return false;
    };

    const drop = (id: string) => void acpApi.dropQueued(agentId, [id]).catch(failed);

    return { queued, send, steer, drop };
}
