import { coreCall, coreRequest } from "../api/core";
import type { ChatInfo, DeviceView, ProjectInfo, SessionInfo } from "../api/generated/wire";
import { invokeCommand as invoke } from "../api/invoke";
import { getIpcTransport } from "../api/transport";
import { agentLabel, isAgentType } from "../agents/catalog";
import { agentWindow } from "../state/agentWindow";
import { makeWindow } from "../state/commands/shared";
import { createSshSession } from "../state/commands/sessions";
import { offerResumableSessions } from "../terminal/sessionResume";
import { agentWindowId } from "../state/selectors";
import { collectPanes } from "../state/layout";
import { getState, mutate, setState, useStore, type StoreState } from "../state/store";
import { notify, reportError, swallow } from "../state/toast";
import { accountApi } from "../api/account";
import type { Agent, PaneNode, Session } from "../state/types";

/** Another host on the account the app joined, as the app reaches it. */
export interface HostEntry {
    /** The host's core key, which every request to it names. */
    readonly id: string;
    readonly name: string;
    readonly platform: string;
    readonly reach: "connecting" | "connected";
    /** The SSH host it was installed on from here, which the rail shows it as. */
    readonly ssh: string | null;
}

export const hostsApi = {
    list: () => invoke<HostEntry[]>("hosts_list"),
};

/**
 * Installs Sikemux on the SSH host `alias` over the person's own SSH, in a terminal tab of its SSH session where a
 * password sudo asks for is typed, and resolves once the host is on the account.
 */
export async function installHere(alias: string): Promise<void> {
    const controller = new AbortController();
    const done = new Promise<void>((resolve, reject) => {
        void getIpcTransport()
            .subscribe<{ alias: string; problem: string | null }>(
                "host_installed",
                ({ payload }) => {
                    if (payload.alias !== alias) return;
                    controller.abort();
                    if (payload.problem) reject(new Error(payload.problem));
                    else resolve();
                },
                { signal: controller.signal },
            )
            .catch(reject);
    });
    try {
        const id = await invoke<number>("host_install", { alias, cols: 100, rows: 30 });
        offerResumableSessions([id]);
        createSshSession(alias);
        mutate((d) => {
            const session = d.sessions[d.activeSessionId];
            if (!session) return;
            const window = makeWindow("", "Install Sikemux", { role: "named" });
            window.root = { ...(window.root as PaneNode), ptyId: id };
            d.windows[window.id] = window;
            d.windowsBySession[session.id] = [...(d.windowsBySession[session.id] ?? []), window.id];
            session.activeWindowId = window.id;
        });
    } catch (error) {
        controller.abort();
        throw error;
    }
    await done;
}

/** One project's folder on another host. */
export interface RemoteCopy {
    readonly host: HostEntry;
    readonly project: ProjectInfo;
    readonly view: DeviceView;
}

const REMOTE_SESSION_PREFIX = "host:";

function remoteSessionId(host: string, project: string): string {
    return `${REMOTE_SESSION_PREFIX}${host}:${project}`;
}

function inFolder(path: string | null | undefined, folder: string): boolean {
    return !!path && (path === folder || path.startsWith(folder.endsWith("/") ? folder : `${folder}/`));
}

/**
 * The copies of `session`'s project on the other hosts the app reaches. A project is one repository wherever it is
 * (ADR 0017): its copies are the projects whose identity, which each core works out the same way, matches. A project
 * only another host holds is its own copy.
 */
export function remoteCopiesOf(state: Pick<StoreState, "hosts" | "hostViews" | "projectIdentities">, session: Session): RemoteCopy[] {
    const copies: RemoteCopy[] = [];
    const identity = session.host ? undefined : state.projectIdentities[session.cwd];
    for (const host of state.hosts) {
        const view = state.hostViews[host.id];
        if (!view) continue;
        for (const project of view.workspace.projects) {
            const same = session.host ? session.host === host.id && project.path === session.cwd : !!identity && project.identity === identity;
            if (same) copies.push({ host, project, view });
        }
    }
    return copies;
}

/** The chats running in a copy, newest attention first. */
export function chatsIn(copy: RemoteCopy): ChatInfo[] {
    return copy.view.chats.filter((chat) => inFolder(chat.cwd, copy.project.path));
}

/** The plain terminals running in a copy. */
export function terminalsIn(copy: RemoteCopy): SessionInfo[] {
    return copy.view.sessions.filter(
        (session) => session.kind === "terminal" && session.running && !session.agentId && inFolder(session.project, copy.project.path),
    );
}

/**
 * The host the person works on now: where the focused chat or terminal runs, else the host the session belongs to.
 * None is this one. New chats and terminals start there.
 */
export function focusedHost(state: Pick<StoreState, "sessions" | "activeSessionId" | "windows" | "agents">): string | undefined {
    const session = state.sessions[state.activeSessionId];
    if (!session) return undefined;
    const window = state.windows[session.activeWindowId];
    const pane = window && collectPanes(window.root).find((candidate) => candidate.id === window.activePaneId);
    return pane?.host ?? (pane && state.agents[pane.id]?.host) ?? session.host;
}

/** Where a new chat or terminal on `host` starts in `session`: its copy of the project there, if it has one. */
export function copyOn(state: Pick<StoreState, "hosts" | "hostViews" | "projectIdentities">, session: Session, host: string): RemoteCopy | undefined {
    return remoteCopiesOf(state, session).find((copy) => copy.host.id === host);
}

/** Opens a terminal on another host from its row under Hosts, in the session that row keeps for it. */
export function openHostTerminal(host: string, name: string): void {
    mutate((d) => {
        const existing = d.sessionOrder.map((id) => d.sessions[id]).find((session) => session?.kind === "command" && session.host === host);
        const window = remoteTerminalWindow(host, "");
        d.windows[window.id] = window;
        if (existing) {
            d.windowsBySession[existing.id] = [...(d.windowsBySession[existing.id] ?? []), window.id];
            existing.activeWindowId = window.id;
            d.activeSessionId = existing.id;
        } else {
            const id = `${REMOTE_SESSION_PREFIX}${host}`;
            d.sessions[id] = { id, name, kind: "command", cwd: "", pinned: false, activeWindowId: window.id, host };
            d.sessionOrder.push(id);
            d.windowsBySession[id] = [window.id];
            d.activeSessionId = id;
        }
        d.zoomedPaneId = null;
    });
}

/**
 * The lock of ADR 0018 on a host, this one when there is none, as a menu item that flips it. Read when the menu opens,
 * since only that host knows it.
 */
export async function approvalItem(host?: string): Promise<{ label: string; selected: boolean; disabled: boolean; run: () => void }> {
    const label = "Require approval for new devices";
    const read = await coreCall<{ requireApproval: boolean }>("requireApproval", null, host).catch(() => null);
    if (!read) return { label, selected: false, disabled: true, run: () => {} };
    return {
        label,
        selected: read.requireApproval,
        disabled: false,
        run: () => void coreCall("setRequireApproval", { requireApproval: !read.requireApproval }, host).catch(reportError(label)),
    };
}

export function hostName(state: Pick<StoreState, "hosts">, id: string | undefined): string | undefined {
    return id ? (state.hosts.find((host) => host.id === id)?.name ?? "another host") : undefined;
}

/**
 * Gives every project only another host holds a row of its own, and drops those of hosts the app left or projects that
 * now have a copy here. They stay out of this host's core's workspace.
 */
function placeRemoteProjects(): void {
    const state = getState();
    const sessions = Object.values(state.sessions);
    const here = new Set(
        sessions.flatMap((session) => {
            const identity = session.kind === "project" && !session.host ? state.projectIdentities[session.cwd] : undefined;
            return identity ? [identity] : [];
        }),
    );
    const wanted = new Map<string, { host: string; project: ProjectInfo }>();
    // A host whose view has not come yet keeps the rows it had.
    const unseen = new Set<string>();
    for (const host of state.hosts) {
        const view = state.hostViews[host.id];
        if (!view) {
            unseen.add(host.id);
            continue;
        }
        for (const project of view.workspace.projects) {
            if (!project.identity || !here.has(project.identity)) wanted.set(remoteSessionId(host.id, project.id), { host: host.id, project });
        }
    }
    const stale = sessions.filter((session) => session.host && !unseen.has(session.host) && !wanted.has(session.id));
    const missing = [...wanted].filter(([id]) => !state.sessions[id]);
    const renamed = [...wanted].filter(([id, { project }]) => state.sessions[id] && state.sessions[id].name !== project.name);
    if (stale.length === 0 && missing.length === 0 && renamed.length === 0) return;
    mutate((d) => {
        for (const session of stale) {
            for (const windowId of d.windowsBySession[session.id] ?? []) delete d.windows[windowId];
            delete d.windowsBySession[session.id];
            delete d.sessions[session.id];
            d.sessionOrder = d.sessionOrder.filter((id) => id !== session.id);
            if (d.activeSessionId === session.id) d.activeSessionId = d.sessionOrder[0] ?? "";
        }
        for (const [id, { host, project }] of missing) {
            const window = remoteTerminalWindow(host, project.path);
            d.sessions[id] = { id, name: project.name, kind: "project", cwd: project.path, pinned: false, activeWindowId: window.id, host };
            d.sessionOrder.push(id);
            d.windows[window.id] = window;
            d.windowsBySession[id] = [window.id];
        }
        for (const [id, { project }] of renamed) d.sessions[id].name = project.name;
    });
}

function remoteTerminalWindow(host: string, cwd: string) {
    const window = makeWindow(cwd, "Terminal", { role: "term" });
    window.root = { ...(window.root as PaneNode), host };
    return window;
}

/** Opens a terminal on `host` in `cwd`, a folder there, as a tab of `sessionId`. */
export function newRemoteTerminal(sessionId: string, host: string, cwd: string): void {
    mutate((d) => {
        const session = d.sessions[sessionId];
        if (!session) return;
        const window = remoteTerminalWindow(host, cwd);
        d.windows[window.id] = window;
        d.windowsBySession[sessionId] = [...(d.windowsBySession[sessionId] ?? []), window.id];
        session.activeWindowId = window.id;
        d.activeSessionId = sessionId;
        d.zoomedPaneId = null;
    });
}

/** Shows a terminal already running on `host` as a tab of `sessionId`. */
export function openRemoteTerminal(sessionId: string, host: string, terminal: SessionInfo, cwd: string): void {
    const open = Object.values(getState().windows).find((window) => window.root.type === "pane" && window.root.ptyId === terminal.id);
    mutate((d) => {
        const session = d.sessions[sessionId];
        if (!session) return;
        let windowId = open?.id;
        if (!windowId) {
            const window = remoteTerminalWindow(host, cwd);
            window.root = { ...(window.root as PaneNode), ptyId: terminal.id };
            d.windows[window.id] = window;
            d.windowsBySession[sessionId] = [...(d.windowsBySession[sessionId] ?? []), window.id];
            windowId = window.id;
        }
        session.activeWindowId = windowId;
        d.activeSessionId = sessionId;
        d.zoomedPaneId = null;
    });
}

/** Shows a chat running on `host` as a tab of `sessionId`, taking it up there. */
export function openRemoteChat(sessionId: string, host: string, chat: Pick<ChatInfo, "agentId" | "provider" | "title" | "name" | "cwd">): void {
    if (!isAgentType(chat.provider)) {
        notify("info", `This app can't show ${chat.provider} chats yet`);
        return;
    }
    const type = chat.provider;
    mutate((d) => {
        const session = d.sessions[sessionId];
        if (!session) return;
        const windowId = agentWindowId(d, chat.agentId);
        if (windowId) {
            session.activeWindowId = windowId;
            d.activeSessionId = sessionId;
            return;
        }
        const agent: Agent = {
            id: chat.agentId,
            type,
            title: chat.title ?? agentLabel(type),
            name: chat.name ?? undefined,
            createdAt: Date.now(),
            permissionMode: d.defaultAgentPermissionMode,
            cwd: chat.cwd,
            launchState: "live",
            host,
        };
        d.agents[agent.id] = agent;
        const window = agentWindow(agent, chat.cwd);
        d.windows[window.id] = window;
        d.windowsBySession[sessionId] = [...(d.windowsBySession[sessionId] ?? []), window.id];
        session.activeWindowId = window.id;
        d.activeSessionId = sessionId;
        d.zoomedPaneId = null;
    });
}

/** Starts a chat with `type` in a copy on another host, on that host's own sign-in, and opens it here. */
export async function startRemoteChat(sessionId: string, copy: RemoteCopy, type: string): Promise<void> {
    const launcher = copy.view.workspace.launchers.find((candidate) => candidate.provider === type);
    if (!launcher) {
        notify("info", `${agentLabel(type)} isn't set up on ${copy.host.name}`);
        return;
    }
    const begun = await coreRequest<{ agentId: string }>(
        { op: "startChat", launcher: launcher.id, project: copy.project.id, permissionMode: null, model: null, effort: null },
        copy.host.id,
    );
    openRemoteChat(sessionId, copy.host.id, { agentId: begun.agentId, provider: type, title: null, name: null, cwd: copy.project.path });
}

/** Says when a device or host joins the account, with one tap to take it off again (ADR 0018). */
function noticeNewDevices(signal: AbortSignal): void {
    void getIpcTransport()
        .subscribe<{ key: string; name: string }>(
            "account_device",
            ({ payload }) =>
                notify("info", `New device on your account: ${payload.name || "a device"}`, {
                    timeoutMs: null,
                    action: {
                        label: "Remove",
                        run: () => accountApi.removeDevice(payload.key).catch(reportError(`Remove ${payload.name || "the device"}`)),
                    },
                }),
            { signal },
        )
        .catch(swallow("follow new devices on the account"));
}

/** Keeps the store's hosts and their views as the app reaches them. */
export function startHosts(): () => void {
    const controller = new AbortController();
    const transport = getIpcTransport();
    noticeNewDevices(controller.signal);
    const setHosts = (hosts: HostEntry[]) => {
        const ids = new Set(hosts.map((host) => host.id));
        setState({
            hosts,
            hostViews: Object.fromEntries(Object.entries(getState().hostViews).filter(([id]) => ids.has(id))),
        });
        placeRemoteProjects();
    };
    void transport
        .subscribe<HostEntry[]>("hosts_changed", (event) => setHosts(event.payload), { signal: controller.signal })
        .catch(swallow("follow other hosts"));
    void transport
        .subscribe<{ host: string; view: DeviceView }>(
            "host_view",
            ({ payload }) => {
                setState({ hostViews: { ...getState().hostViews, [payload.host]: payload.view } });
                placeRemoteProjects();
            },
            { signal: controller.signal },
        )
        .catch(swallow("follow other hosts' views"));
    void hostsApi.list().then(setHosts).catch(swallow("list other hosts"));
    const stopFollowing = useStore.subscribe((state, previous) => {
        if (state.projectIdentities !== previous.projectIdentities) placeRemoteProjects();
    });
    return () => {
        controller.abort();
        stopFollowing();
    };
}
