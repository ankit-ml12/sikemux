import { coreCall, coreRequest } from "../api/core";
import type { CoreChange } from "../api/coreChanged";
import type { ChatInfo, DeviceView, ProjectInfo, SessionInfo, SshLink } from "../api/generated/wire";
import { coreSessionsApi } from "../api/coreSessions";
import { invokeCommand as invoke } from "../api/invoke";
import { getIpcTransport } from "../api/transport";
import { agentLabel, isAgentType } from "../agents/catalog";
import { agentWindow } from "../state/agentWindow";
import { makeWindow } from "../state/commands/shared";
import { createSshSession } from "../state/commands/sessions";
import { offerResumableSessions } from "../terminal/sessionResume";
import { agentWindowId, ownerSessionId } from "../state/selectors";
import { collectPanes } from "../state/layout";
import { getState, mutate, setState, useStore, type StoreState } from "../state/store";
import { notify, reportError, swallow } from "../state/toast";
import { accountApi } from "../api/account";
import type { Agent, PaneNode, Session } from "../state/types";
import type { Draft } from "immer";

/** A row of the rail's Hosts group, as the core lists it: a host linked with this one, or an SSH host a terminal is open to. */
export interface HostRow {
    /** The host's core key, which every request to it names; none for an SSH host without Sikemux. */
    readonly id: string | null;
    readonly name: string;
    readonly platform: string | null;
    /** How this host's core reaches it; `failed` only for an SSH host ssh gave up on. */
    readonly reach: "connecting" | "connected" | "away" | "failed";
    /** The SSH host it was installed on from here, or the row's own alias. */
    readonly ssh: string | null;
}

/** Another host on the account. */
export type HostEntry = HostRow & { readonly id: string; readonly platform: string };

export const hostsApi = {
    list: () => coreCall<HostRow[]>("hosts"),
};

/** What asking an SSH host found: which host it is, and whether it is on this host's account, signed out, or another's. */
export interface SshProbe {
    readonly key: string;
    readonly account: "same" | "signedOut" | "other";
}

/**
 * Runs `command` on the SSH host `alias` over the person's own SSH, in a terminal tab of its SSH session where a
 * password ssh or sudo asks for is typed, and resolves once the host is on the account.
 */
async function signInOverSsh(alias: string, command: "host_install" | "host_sign_in", title: string): Promise<void> {
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
        const id = await invoke<number>(command, { alias, cols: 100, rows: 30 });
        offerResumableSessions([id]);
        createSshSession(alias);
        mutate((d) => {
            const session = d.sessions[d.activeSessionId];
            if (!session) return;
            const window = makeWindow("", title, { role: "named" });
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

/** Installs Sikemux on the SSH host `alias` and signs it in to the account. */
export const installHere = (alias: string): Promise<void> => signInOverSsh(alias, "host_install", "Install Sikemux");

/** Signs the SSH host `alias`, which runs Sikemux but is off the account, in to it. */
export async function signInHere(alias: string): Promise<void> {
    await signInOverSsh(alias, "host_sign_in", "Sign in");
    await probeSshHost(alias);
}

/** Asks the SSH host `alias` whether it runs Sikemux and which host it is, keeping the answer for its row. */
async function probeSshHost(alias: string): Promise<void> {
    const probed = await invoke<SshProbe | null>("host_probe", { alias });
    const { [alias]: _, ...others } = getState().sshProbes;
    setState({ sshProbes: probed ? { ...others, [alias]: probed } : others });
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
        (session) => session.kind === "terminal" && session.running && !session.agentId && session.project === copy.project.id,
    );
}

/** Where a new chat or terminal on `host` starts in `session`: its copy of the project there, if it has one. */
export function copyOn(state: Pick<StoreState, "hosts" | "hostViews" | "projectIdentities">, session: Session, host: string): RemoteCopy | undefined {
    return remoteCopiesOf(state, session).find((copy) => copy.host.id === host);
}

/** The project a new chat on `host` starts in: `session`'s copy there, else the first open project with one. */
export function projectCopyOn(
    state: Pick<StoreState, "hosts" | "hostViews" | "projectIdentities" | "sessionOrder" | "sessions">,
    host: string,
    session: Session | undefined,
): { session: Session; copy: RemoteCopy } | undefined {
    const candidates = session
        ? [session, ...state.sessionOrder.map((id) => state.sessions[id])]
        : state.sessionOrder.map((id) => state.sessions[id]);
    for (const candidate of candidates) {
        const copy = candidate?.kind === "project" ? copyOn(state, candidate, host) : undefined;
        if (copy) return { session: candidate, copy };
    }
    return undefined;
}

/** A host the side rail chooses: this one or one on the account, by its core's key (none for this one), or an SSH host by its alias. */
export type HostChoice = { readonly kind: "core"; readonly host?: string } | { readonly kind: "ssh"; readonly alias: string };

/** A chosen host, with the session that was in front when it was chosen or that it brought forward since. */
export type ChosenHost = HostChoice & { readonly over: string };

/** The host chosen in the side rail, while the session it was chosen over is still in front. */
export function chosenHostOf(state: Pick<StoreState, "chosenHost" | "activeSessionId">): HostChoice | undefined {
    return state.chosenHost?.over === state.activeSessionId ? state.chosenHost : undefined;
}

export function sameHost(a: HostChoice | undefined, b: HostChoice): boolean {
    if (!a) return false;
    return a.kind === "core" ? b.kind === "core" && a.host === b.host : b.kind === "ssh" && a.alias === b.alias;
}

/** Chooses a host's row in the side rail as a project's row is chosen; what is in front stays. */
export function chooseHost(choice: HostChoice): void {
    mutate((d) => {
        d.chosenHost = { ...choice, over: d.activeSessionId };
        d.settingsOpen = false;
        d.pickerOpen = false;
    });
}

/** Brings `sessionId` forward for something opened from a host, keeping that host chosen. */
export function bringForward(d: Draft<StoreState>, sessionId: string): void {
    if (d.chosenHost?.over === d.activeSessionId) d.chosenHost.over = sessionId;
    d.activeSessionId = sessionId;
    d.zoomedPaneId = null;
}

const THIS_HOST: HostChoice = { kind: "core" };

/**
 * Where new chats and terminals start: the chosen host, else the host of the chat or terminal in front, else this one.
 * Its row under Hosts says "here".
 */
export function newWorkHost(state: Pick<StoreState, "chosenHost" | "sessions" | "activeSessionId" | "windows" | "agents">): HostChoice {
    const chosen = chosenHostOf(state);
    if (chosen) return chosen;
    const session = state.sessions[state.activeSessionId];
    const window = session && state.windows[session.activeWindowId];
    const pane = window && collectPanes(window.root).find((candidate) => candidate.id === window.activePaneId);
    if (session?.kind === "ssh") return pane?.kind === "terminal" ? { kind: "ssh", alias: session.name } : THIS_HOST;
    const host = pane?.host ?? (pane && state.agents[pane.id]?.host) ?? session?.host;
    return host ? { kind: "core", host } : THIS_HOST;
}

/** Runs `open`, keeping a host chosen before it chosen over whatever it brings forward. */
export function keepHostChosen(open: () => void): void {
    const chosen = chosenHostOf(getState());
    open();
    if (chosen) setState({ chosenHost: { ...chosen, over: getState().activeSessionId } });
}

/** Another login on the SSH host `alias`, as a tab of its session, which its row under Hosts means is open. */
export function newSshTerminalOn(alias: string): void {
    mutate((d) => {
        const session = d.sessionOrder.map((id) => d.sessions[id]).find((candidate) => candidate?.kind === "ssh" && candidate.name === alias);
        if (!session) return;
        const window = makeWindow("", alias, { role: "named" });
        d.windows[window.id] = window;
        d.windowsBySession[session.id] = [...(d.windowsBySession[session.id] ?? []), window.id];
        session.activeWindowId = window.id;
        bringForward(d, session.id);
    });
}

/** Shows a tab that is open already, from a chosen host's list. */
export function showTab(windowId: string): void {
    mutate((d) => {
        const sessionId = ownerSessionId(d, windowId);
        if (!sessionId) return;
        d.sessions[sessionId].activeWindowId = windowId;
        bringForward(d, sessionId);
    });
}

/** The session a host keeps for its terminals and chats outside any project open here, made the first time. */
function hostSession(d: Draft<StoreState>, host: string): Draft<Session> {
    const id = `${REMOTE_SESSION_PREFIX}${host}`;
    if (!d.sessions[id]) {
        d.sessions[id] = { id, name: hostName(d, host) ?? host, kind: "command", cwd: "", pinned: false, activeWindowId: "", host };
        d.sessionOrder.push(id);
        d.windowsBySession[id] = [];
    }
    return d.sessions[id];
}

/**
 * Where something at `path` on `host` belongs here: the project there that holds it, and the session open here for
 * that project, which is null when none is and it opens in the host's own.
 */
export function placeOn(
    state: Pick<StoreState, "hostViews" | "sessionOrder" | "sessions" | "projectIdentities">,
    host: string,
    path: string | null | undefined,
): { project: ProjectInfo | undefined; sessionId: string | null } {
    const project = state.hostViews[host]?.workspace.projects.find((candidate) => inFolder(path, candidate.path));
    const sessionId =
        project &&
        state.sessionOrder.find((id) => {
            const session = state.sessions[id];
            if (session?.kind !== "project") return false;
            return session.host
                ? id === remoteSessionId(host, project.id)
                : !!project.identity && state.projectIdentities[session.cwd] === project.identity;
        });
    return { project, sessionId: sessionId ?? null };
}

/** Opens a terminal on another host outside any project, in the session that host keeps. */
export function openHostTerminal(host: string): void {
    newRemoteTerminal(null, host, "");
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

/** What the rail calls a host. */
export function hostTitle(state: Pick<StoreState, "hosts">, choice: HostChoice): string {
    if (choice.kind === "ssh") return choice.alias;
    return hostName(state, choice.host) ?? "This host";
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

/** The session `sessionId` names, or the one `host` keeps for what runs outside a project when it is null. */
function sessionIn(d: Draft<StoreState>, sessionId: string | null, host: string): Draft<Session> | undefined {
    return sessionId === null ? hostSession(d, host) : d.sessions[sessionId];
}

/** Opens a terminal on `host` in `cwd`, a folder there, as a tab of `sessionId`, or of the host's own session when null. */
export function newRemoteTerminal(sessionId: string | null, host: string, cwd: string): void {
    mutate((d) => {
        const session = sessionIn(d, sessionId, host);
        if (!session) return;
        const window = remoteTerminalWindow(host, cwd);
        d.windows[window.id] = window;
        d.windowsBySession[session.id] = [...(d.windowsBySession[session.id] ?? []), window.id];
        session.activeWindowId = window.id;
        bringForward(d, session.id);
    });
}

/** Shows a terminal already running on `host` as a tab of `sessionId`, or of the host's own session when null. */
export function openRemoteTerminal(sessionId: string | null, host: string, terminal: SessionInfo, cwd: string): void {
    const open = Object.values(getState().windows).find((window) => window.root.type === "pane" && window.root.ptyId === terminal.id);
    if (open) return showTab(open.id);
    mutate((d) => {
        const session = sessionIn(d, sessionId, host);
        if (!session) return;
        const window = remoteTerminalWindow(host, cwd);
        window.root = { ...(window.root as PaneNode), ptyId: terminal.id };
        d.windows[window.id] = window;
        d.windowsBySession[session.id] = [...(d.windowsBySession[session.id] ?? []), window.id];
        session.activeWindowId = window.id;
        bringForward(d, session.id);
    });
}

/** Shows a chat running on `host` as a tab of `sessionId`, or of the host's own session when null, taking it up there. */
export function openRemoteChat(
    sessionId: string | null,
    host: string,
    chat: Pick<ChatInfo, "agentId" | "provider" | "title" | "name" | "cwd">,
): void {
    if (!isAgentType(chat.provider)) {
        notify("info", `This app can't show ${chat.provider} chats yet`);
        return;
    }
    const type = chat.provider;
    const open = agentWindowId(getState(), chat.agentId);
    if (open) return showTab(open);
    mutate((d) => {
        const session = sessionIn(d, sessionId, host);
        if (!session) return;
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
        d.windowsBySession[session.id] = [...(d.windowsBySession[session.id] ?? []), window.id];
        session.activeWindowId = window.id;
        bringForward(d, session.id);
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

/** The SSH host a terminal runs on, by the session it is a tab of. */
function sshAliasOf(state: Pick<StoreState, "sessionOrder" | "sessions" | "windowsBySession" | "windows">, ptyId: number): string | undefined {
    const holds = (windowId: string) => {
        const window = state.windows[windowId];
        return !!window && collectPanes(window.root).some((pane) => pane.ptyId === ptyId);
    };
    return state.sessionOrder
        .map((id) => state.sessions[id])
        .find((session) => session?.kind === "ssh" && (state.windowsBySession[session.id] ?? []).some(holds))?.name;
}

/**
 * Asks each SSH host once, as a terminal on it connects, whether it runs Sikemux and which host it is, so a host on
 * the account shows as one row (ADR 0018).
 */
function probeSshHosts(signal: AbortSignal): void {
    const asked = new Set<string>();
    const probe = (ptyId: number) => {
        const alias = sshAliasOf(getState(), ptyId);
        if (!alias || asked.has(alias)) return;
        asked.add(alias);
        void probeSshHost(alias).catch(swallow(`ask ${alias} which host it is`));
    };
    void getIpcTransport()
        .subscribe<{ id: number; link: SshLink }>(
            "pty_ssh_link",
            ({ payload }) => {
                if (payload.link === "connected") probe(payload.id);
            },
            { signal },
        )
        .catch(swallow("follow SSH terminals connecting"));
    void coreSessionsApi
        .list()
        .then((sessions) => {
            for (const session of sessions) if (session.ssh === "connected") probe(session.id);
        })
        .catch(swallow("list SSH terminals"));
}

/** Keeps the store's hosts and their views as the app reaches them. */
export function startHosts(): () => void {
    const controller = new AbortController();
    const transport = getIpcTransport();
    noticeNewDevices(controller.signal);
    probeSshHosts(controller.signal);
    const setHosts = (rows: HostRow[]) => {
        const hosts = rows.filter((row): row is HostEntry => row.id !== null);
        const ids = new Set(hosts.map((host) => host.id));
        setState({
            hosts,
            openSshHosts: rows.filter((row) => row.id === null),
            hostViews: Object.fromEntries(Object.entries(getState().hostViews).filter(([id]) => ids.has(id))),
        });
        placeRemoteProjects();
    };
    void transport
        .subscribe<CoreChange>(
            "core_changed",
            ({ payload }) => {
                if (payload.topic === "hosts") void hostsApi.list().then(setHosts).catch(swallow("list other hosts"));
            },
            { signal: controller.signal },
        )
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
