import { useEffect, useMemo, useState, type MouseEvent, type ReactNode } from "react";
import { accountApi } from "../api/account";
import { coreCall } from "../api/core";
import {
    approvalItem,
    chooseHost,
    chosenHostOf,
    installHere,
    newWorkHost,
    openHostTerminal,
    projectCopyOn,
    sameHost,
    signInHere,
    startRemoteChat,
    type HostChoice,
    type HostEntry,
    type HostRow as CoreHostRow,
} from "../hosts/hosts";
import * as cmd from "../state/commands";
import { confirmDialog, promptDialog } from "../state/dialog";
import { getState, useStore, type StoreState } from "../state/store";
import { reportError, swallow } from "../state/toast";
import {
    IconDownload,
    IconFolder,
    IconLock,
    IconLaptop,
    IconLogIn,
    IconMore,
    IconPencil,
    IconPlus,
    IconServer,
    IconTerminal,
    IconTrash,
} from "../ui/Icons";
import { StatusDot } from "../ui/StatusDot";
import type { CtxItem } from "./FileTree";

type OpenMenu = (event: MouseEvent, items: CtxItem[]) => void;

/** How many agents work on a host the app reaches, from what it last showed. */
function remoteAgents(state: Pick<StoreState, "hostViews">, host: string): number {
    const view = state.hostViews[host];
    if (!view) return 0;
    return view.chats.length + view.sessions.filter((session) => session.running && session.agentId).length;
}

const LOAD_EVERY_MS = 30_000;

/** How busy a host the app reaches is, read again every half minute while the app is in front; the core has no event for it. */
function useHostLoad(host: string, connected: boolean): string | null {
    const [load, setLoad] = useState<string | null>(null);
    useEffect(() => {
        if (!connected) return;
        let current = true;
        const read = () => {
            if (document.visibilityState !== "visible") return;
            void coreCall<{ cpu: number; memoryUsed: number | null }>("hostLoad", null, host)
                .then(({ cpu, memoryUsed }) => {
                    if (current) setLoad(memoryUsed === null ? `${cpu}%` : `${cpu}% · ${(memoryUsed / 1024 ** 3).toFixed(1)} GB`);
                })
                .catch(swallow("read a host's load"));
        };
        read();
        const timer = window.setInterval(read, LOAD_EVERY_MS);
        return () => {
            current = false;
            window.clearInterval(timer);
        };
    }, [host, connected]);
    return connected ? load : null;
}

/** Installing Sikemux on an SSH host, or signing in one that has it, while either runs. */
function useOverSsh(alias: string | null) {
    const [busy, setBusy] = useState<"installing" | "signing in" | null>(null);
    const run = (doing: "installing" | "signing in", action: (alias: string) => Promise<void>, failed: string) => {
        if (!alias) return;
        setBusy(doing);
        action(alias)
            .catch(reportError(failed))
            .finally(() => setBusy(null));
    };
    return {
        busy,
        install: () => run("installing", installHere, `Install Sikemux on ${alias}`),
        signIn: () => run("signing in", signInHere, `Sign in ${alias}`),
    };
}

/** What a row says of a host that runs Sikemux but is not on this host's account. */
const OFF_ACCOUNT = { signedOut: "not signed in", other: "on another account" } as const;

function agentsLabel(count: number): string | null {
    return count === 0 ? null : `${count} ${count === 1 ? "agent" : "agents"}`;
}

function HostRow({
    kind,
    name,
    load,
    detail,
    status,
    choice,
    menu,
    onMenu,
}: {
    kind: "this" | "sikemux" | "ssh";
    name: string;
    load?: string | null;
    detail?: string | null;
    status: ReactNode;
    choice: HostChoice;
    menu: () => Promise<CtxItem[]>;
    onMenu: OpenMenu;
}) {
    const chosen = useStore((s) => !s.settingsOpen && sameHost(chosenHostOf(s), choice));
    const here = useStore((s) => sameHost(newWorkHost(s), choice));
    const show = (event: MouseEvent) => {
        event.preventDefault();
        void menu().then((items) => onMenu(event, items));
    };
    const icon = kind === "this" ? <IconLaptop size={14} /> : <IconServer size={14} />;
    return (
        <div className="session-row-shell host-row" onContextMenu={show}>
            <button type="button" className={`sess-row${chosen ? " active" : ""}${here ? " is-here" : ""}`} onClick={() => chooseHost(choice)}>
                <span className={`sess-icon host-${kind}`}>
                    <span className="sess-icon-glyph">{icon}</span>
                </span>
                <span className="sess-name">{name}</span>
                {here && <span className="host-row-here">here</span>}
                {load && <span className="host-row-load">{load}</span>}
                {detail && <span className="host-row-detail">{detail}</span>}
            </button>
            <span className="row-status">{status}</span>
            <button type="button" className="row-x host-row-more" aria-label={`More for ${name}`} onClick={show}>
                <IconMore size={12} />
            </button>
        </div>
    );
}

const THIS_HOST: HostChoice = { kind: "core" };

function ThisHostRow({ onMenu }: { onMenu: OpenMenu }) {
    const agents = useStore((s) => Object.values(s.agents).filter((agent) => !agent.host).length);
    return (
        <HostRow
            kind="this"
            name="This host"
            detail={agentsLabel(agents)}
            status={<StatusDot tone="ok" label="This host" />}
            choice={THIS_HOST}
            menu={async () => [
                { label: "New terminal here", icon: <IconTerminal size={13} />, run: cmd.createCommandSession },
                { sep: true },
                { ...(await approvalItem()), icon: <IconLock size={13} /> },
            ]}
            onMenu={onMenu}
        />
    );
}

function AccountHostRow({ host, onMenu }: { host: HostEntry; onMenu: OpenMenu }) {
    const agents = useStore((s) => remoteAgents(s, host.id));
    const choice = useMemo<HostChoice>(() => ({ kind: "core", host: host.id }), [host.id]);
    const connected = host.reach === "connected";
    const load = useHostLoad(host.id, connected);
    const chat = () => {
        const state = getState();
        const place = projectCopyOn(state, host.id, state.sessions[state.activeSessionId]);
        if (!place) return;
        void startRemoteChat(place.session.id, place.copy, cmd.lastAgentType() ?? "claude").catch(reportError(`New chat on ${host.name}`));
    };
    const files = () => {
        const state = getState();
        const place = projectCopyOn(state, host.id, state.sessions[state.activeSessionId]);
        if (!place) return;
        cmd.selectSession(place.session.id);
        cmd.setTabRailPage("files");
    };
    const rename = async () => {
        const name = await promptDialog({ title: `Rename ${host.name}`, label: "Name", initial: host.name, confirmLabel: "Rename" });
        if (name?.trim()) await accountApi.renameDevice(host.id, name.trim()).catch(reportError(`Rename ${host.name}`));
    };
    const remove = async () => {
        const confirmed = await confirmDialog({
            title: `Remove ${host.name} from your account?`,
            body: "Every device and host on your account stops reaching it at once. To add it again, sign it in again.",
            confirmLabel: "Remove",
            destructive: true,
        });
        if (confirmed) await accountApi.removeDevice(host.id).catch(reportError(`Remove ${host.name}`));
    };
    const hasProject = useStore((s) => !!projectCopyOn(s, host.id, undefined));
    const account = useStore((s) => (host.ssh ? s.sshProbes[host.ssh]?.account : undefined));
    const off = account === "signedOut" || account === "other" ? OFF_ACCOUNT[account] : null;
    const { busy, signIn } = useOverSsh(host.ssh);
    return (
        <HostRow
            kind="sikemux"
            name={host.name}
            load={load}
            detail={busy ?? off ?? (connected ? agentsLabel(agents) : host.reach === "away" ? "out of reach" : "connecting")}
            status={
                busy ? (
                    <StatusDot tone="pending" label="Signing in" />
                ) : off ? (
                    <StatusDot tone="idle" label={off} />
                ) : (
                    <StatusDot
                        tone={connected ? "ok" : host.reach === "away" ? "idle" : "pending"}
                        label={connected ? "Connected" : host.reach === "away" ? "Out of reach" : "Connecting"}
                    />
                )
            }
            choice={choice}
            menu={async () => [
                ...(account === "signedOut"
                    ? [{ label: "Sign in", icon: <IconLogIn size={13} />, run: signIn, disabled: !!busy }, { sep: true as const }]
                    : []),
                { label: "New chat here", icon: <IconPlus size={13} />, run: chat, disabled: !hasProject },
                { label: "New terminal here", icon: <IconTerminal size={13} />, run: () => openHostTerminal(host.id) },
                { label: "Files", icon: <IconFolder size={13} />, run: files, disabled: !hasProject },
                { sep: true },
                { ...(await approvalItem(host.id)), icon: <IconLock size={13} /> },
                { label: "Rename", icon: <IconPencil size={13} />, run: () => void rename() },
                { label: "Remove from account", icon: <IconTrash size={13} />, danger: true, run: () => void remove() },
            ]}
            onMenu={onMenu}
        />
    );
}

const SSH_TONE = { connected: "ok", connecting: "pending", failed: "failed", away: "idle" } as const;
const SSH_LABEL = { connected: "Connected", connecting: "Connecting", failed: "Failed", away: "Not connected" } as const;

function SshHostRow({ row, onMenu }: { row: CoreHostRow; onMenu: OpenMenu }) {
    const alias = row.name;
    const choice = useMemo<HostChoice>(() => ({ kind: "ssh", alias }), [alias]);
    const account = useStore((s) => s.sshProbes[alias]?.account);
    const off = account === "signedOut" || account === "other" ? OFF_ACCOUNT[account] : null;
    const { busy, install, signIn } = useOverSsh(alias);
    const setUp = account
        ? account === "signedOut"
            ? [{ label: "Sign in", icon: <IconLogIn size={13} />, run: signIn, disabled: !!busy }]
            : []
        : [{ label: "Install Sikemux here", icon: <IconDownload size={13} />, run: install, disabled: !!busy }];
    return (
        <HostRow
            kind="ssh"
            name={alias}
            detail={busy ?? off}
            status={<StatusDot tone={busy ? "pending" : SSH_TONE[row.reach]} label={busy ?? SSH_LABEL[row.reach]} />}
            choice={choice}
            menu={async () => [
                {
                    label: "New SSH terminal",
                    icon: <IconTerminal size={13} />,
                    run: () => {
                        cmd.createSshSession(alias);
                        cmd.newSshTerminal(alias);
                    },
                },
                ...setUp,
                { sep: true },
                { label: "Edit in ~/.ssh/config", icon: <IconPencil size={13} />, run: () => void cmd.openSshConfigEditor() },
            ]}
            onMenu={onMenu}
        />
    );
}

/** Every host in one list: this one, every host on the account, and the SSH hosts open now (ADR 0018). */
export function HostRows({ onMenu }: { onMenu: OpenMenu }) {
    const hosts = useStore((s) => s.hosts);
    const sshHosts = useStore((s) => s.openSshHosts);
    return (
        <>
            <ThisHostRow onMenu={onMenu} />
            {hosts.map((host) => (
                <AccountHostRow key={host.id} host={host} onMenu={onMenu} />
            ))}
            {sshHosts.map((row) => (
                <SshHostRow key={row.name} row={row} onMenu={onMenu} />
            ))}
        </>
    );
}
