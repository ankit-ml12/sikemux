import { useSyncExternalStore } from "react";
import { INBOX_EVENT, inboxApi, type Inbox, type InboxItem } from "../api/inbox";
import { getIpcTransport } from "../api/transport";
import { agentWindowId, ownerSessionId, projectSessionId } from "../state/selectors";
import type { StoreState } from "../state/store";
import { swallow } from "../state/toast";

interface InboxState {
    inbox: Inbox;
}

let state: InboxState = { inbox: { items: [], working: 0, away: false, agentMessages: {}, remoteAgents: [], order: [] } };
const listeners = new Set<() => void>();
let started = false;

function take(inbox: Inbox): void {
    state = { inbox };
    for (const listener of listeners) listener();
}

function start(): void {
    started = true;
    void getIpcTransport()
        .subscribe<Inbox>(INBOX_EVENT, (event) => take(event.payload))
        .then(() => inboxApi.read())
        .then(take)
        .catch(swallow("read the inbox"));
}

function subscribe(listener: () => void): () => void {
    if (!started) start();
    listeners.add(listener);
    return () => listeners.delete(listener);
}

/** The core's inbox, kept for the whole app once anything reads it. */
export function useInbox(): InboxState {
    return useSyncExternalStore(subscribe, () => state);
}

/** Agents in the order the core lists them on every device; those it does not list keep their order after it. */
export function inCoreOrder<T extends { id: string }>(agents: readonly T[], order: readonly string[]): T[] {
    const rank = (agent: T) => {
        const index = order.indexOf(agent.id);
        return index < 0 ? order.length : index;
    };
    return [...agents].sort((a, b) => rank(a) - rank(b));
}

/** The project an item belongs to among those open: its agent's, else its folder's. */
export function itemSessionId(state: Pick<StoreState, "windows" | "sessionOrder" | "windowsBySession" | "sessions">, item: InboxItem): string | null {
    const windowId = item.agentId && agentWindowId(state, item.agentId);
    if (windowId) return ownerSessionId(state, windowId);
    const project = item.project ?? item.card?.project;
    return project ? projectSessionId(state, project) : null;
}
