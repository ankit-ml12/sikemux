import { invokeCommand as invoke } from "./invoke";
import type { PublishedBuddies } from "../buddies/published";
import { getIpcTransport } from "./transport";

import type { AllowedDevice, DeviceAccess, PublishedChat, PublishedRecent, RemotePairing, RemoteStatus } from "./generated/wire";

export type { AccountLink, DeviceInfo, LinkedHost, NotificationState, PendingDevice, UpdateRequired } from "./generated/wire";
export type { AllowedDevice, DeviceAccess, PublishedChat, PublishedRecent, RemotePairing, RemoteStatus };

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
