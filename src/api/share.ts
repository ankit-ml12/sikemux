import { coreCall } from "./core";
import { invokeCommand as invoke } from "./invoke";

/** A chat this computer shares, as its core runs it. */
export interface ShareView {
    readonly id: string;
    readonly agentId: string;
    /** The link to hand out, key included. */
    readonly url: string;
    /** Milliseconds since the Unix epoch. */
    readonly expiresAt: number;
    readonly viewers: number;
    readonly state: "connecting" | "open" | "refused";
    /** Why the server would not open it. */
    readonly problem?: string;
}

/** Where to watch a shared chat from, and the sign-in to watch with; no token when signed out. */
export interface ShareAccess {
    readonly socket: string;
    readonly token: string | null;
}

export const shareApi = {
    /** Shares the chat, or with a share already open, tells its viewers its name. */
    share: (agentId: string, lifetimeMs: number, title: string) => coreCall<ShareView>("shareChat", { agentId, lifetimeMs, title }),
    stop: (id: string) => coreCall<null>("stopShare", { id }),
    list: () => coreCall<ShareView[]>("shares", {}),
    access: () => invoke<ShareAccess>("share_access"),
};
