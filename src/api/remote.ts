import { invokeCommand as invoke } from "./invoke";
import type { PublishedBuddies } from "../buddies/published";
import { getIpcTransport } from "./transport";

import type { AllowedDevice, DeviceAccess, DeviceInfo, PendingDevice, PublishedChat, PublishedRecent, RemoteStatus } from "./generated/wire";

export type { AccountLink, NotificationState, UpdateRequired } from "./generated/wire";
export type { AllowedDevice, DeviceAccess, DeviceInfo, PendingDevice, PublishedChat, PublishedRecent, RemoteStatus };

/** Another host on the account linked with this one, whose agents and this host's may start each other's workers. */
export interface LinkedHost {
    /** The host's public key. */
    readonly id: string;
    readonly name: string;
    readonly platform: string;
    readonly linkedAt: number;
    readonly lastSeen: number | null;
    readonly connected: boolean;
    /** Agents of teams shared with this host that run on it. */
    readonly agents: number;
}

/** Who waits to join or link with this host, and who already has. */
export interface RemotePairing {
    readonly pending: readonly PendingDevice[];
    readonly devices: readonly DeviceInfo[];
    readonly links: readonly LinkedHost[];
    readonly pendingLinks: readonly PendingDevice[];
}

/** A project a paired device may start an agent in. */
export interface PublishedProject {
    readonly id: string;
    readonly name: string;
    readonly path: string;
}

export const REMOTE_STATUS_EVENT = "remote_status_changed";

export const remoteApi = {
    status: () => invoke<RemoteStatus>("remote_status"),
    pairing: () => invoke<RemotePairing>("remote_pairing"),
    setEnabled: (enabled: boolean) => invoke<RemoteStatus>("remote_set_enabled", { enabled }),
    setDeviceAccess: (id: string, access: DeviceAccess) => invoke<RemoteStatus>("remote_set_device_access", { id, access }),
    revokeDevice: (id: string) => invoke<RemoteStatus>("remote_revoke_device", { id }),
    answerPairing: (id: string, allow: boolean, access: DeviceAccess) => invoke<RemoteStatus>("remote_answer_pairing", { id, allow, access }),
    allowDevices: (devices: readonly AllowedDevice[]) => invoke<RemoteStatus>("remote_allow_devices", { devices }),
    publishWorkspace: (projects: readonly PublishedProject[], permissionMode: string) =>
        invoke<void>("remote_publish_workspace", { projects, permissionMode }),
    publishAgents: (chats: readonly PublishedChat[], titles: Readonly<Record<string, string>>) =>
        invoke<void>("remote_publish_agents", { chats, titles }),
    publishRecent: (chats: readonly PublishedRecent[]) => invoke<void>("remote_publish_recent", { chats }),
    publishOnScreen: (agentIds: readonly string[], focused: boolean) => invoke<void>("remote_publish_on_screen", { agentIds, focused }),
    publishPalette: (palette: Readonly<Record<string, string>>) => invoke<void>("remote_publish_palette", { palette }),
    publishBuddies: (buddies: PublishedBuddies | null) => invoke<void>("remote_publish_buddies", { buddies }),
    publishBackdrop: (texture: boolean, image: { readonly id: string; readonly dataUrl: string } | null) =>
        invoke<void>("remote_publish_backdrop", { texture, image }),
    subscribe: (listener: (status: RemoteStatus) => void, signal: AbortSignal) =>
        getIpcTransport().subscribe<RemoteStatus>(REMOTE_STATUS_EVENT, (event) => listener(event.payload), { signal }),
};

/** The first eight characters of a key, enough to tell two devices apart by eye. */
export function shortKey(key: string): string {
    return key.slice(0, 8);
}
