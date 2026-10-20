import { Fragment, useMemo } from "react";
import { agentHandle } from "../agents/AgentLabel";
import { agentLabel, isAgentType } from "../agents/catalog";
import type { ChatInfo, SessionInfo } from "../api/generated/wire";
import {
    chatsIn,
    hostName,
    newRemoteTerminal,
    openRemoteChat,
    openRemoteTerminal,
    remoteCopiesOf,
    startRemoteChat,
    terminalsIn,
    type RemoteCopy,
} from "../hosts/hosts";
import { useStore } from "../state/store";
import { swallow } from "../state/toast";
import type { AgentType, Session } from "../state/types";
import { AgentIcon, IconAgent, IconPlus, IconTerminal } from "../ui/Icons";

/** The quiet note on a chat or terminal that runs on another host. */
export function HostLabel({ host }: { host: string | undefined }) {
    const name = useStore((s) => hostName(s, host));
    return name ? <span className="agent-row-host">on {name}</span> : null;
}

/** The copies of the session's project on the other hosts the app reaches. */
export function useRemoteCopies(session: Session | undefined): RemoteCopy[] {
    const hosts = useStore((s) => s.hosts);
    const hostViews = useStore((s) => s.hostViews);
    const projectIdentities = useStore((s) => s.projectIdentities);
    return useMemo(
        () => (session ? remoteCopiesOf({ hosts, hostViews, projectIdentities }, session) : []),
        [session, hosts, hostViews, projectIdentities],
    );
}

/** New chats and terminals in the project's copies elsewhere, until a new chat picks its host. */
export function RemoteNewRows({ session, copies, type }: { session: Session; copies: readonly RemoteCopy[]; type: AgentType | null }) {
    if (session.host) return null;
    return copies.map((copy) => (
        <Fragment key={`${copy.host.id}:${copy.project.id}`}>
            {type && (
                <button
                    type="button"
                    className="agent-row agent-new"
                    onClick={() => void startRemoteChat(session.id, copy, type).catch(swallow(`start a chat on ${copy.host.name}`))}>
                    <span className="agent-glyph">
                        <IconPlus size={15} />
                    </span>
                    <span className="agent-title">New chat</span>
                    <span className="agent-row-host">on {copy.host.name}</span>
                </button>
            )}
            <button type="button" className="agent-row agent-new" onClick={() => newRemoteTerminal(session.id, copy.host.id, copy.project.path)}>
                <span className="agent-glyph term">
                    <IconTerminal size={15} />
                </span>
                <span className="agent-title">New terminal</span>
                <span className="agent-row-host">on {copy.host.name}</span>
            </button>
        </Fragment>
    ));
}

/** A chat running in a copy elsewhere that this app does not show yet. */
function RemoteChatRow({ sessionId, copy, chat }: { sessionId: string; copy: RemoteCopy; chat: ChatInfo }) {
    return (
        <div className="agent-row-wrap">
            <button type="button" className="agent-row" onClick={() => openRemoteChat(sessionId, copy.host.id, chat)}>
                <span className={`agent-glyph ${chat.provider}`}>
                    {isAgentType(chat.provider) ? <AgentIcon type={chat.provider} size={20} /> : <IconAgent size={20} />}
                </span>
                <span className="agent-row-text">
                    <span className="agent-title">{chat.title ?? agentLabel(chat.provider)}</span>
                </span>
                <span className="agent-row-host">on {copy.host.name}</span>
                {chat.name && <span className="agent-row-name">{agentHandle(chat.name)}</span>}
            </button>
        </div>
    );
}

/** The chats running in the project's copies elsewhere that are not open here. */
export function remoteChatRows({ sessionId, copies, open }: { sessionId: string; copies: readonly RemoteCopy[]; open: ReadonlySet<string> }) {
    return copies.flatMap((copy) =>
        chatsIn(copy)
            .filter((chat) => !open.has(chat.agentId))
            .map((chat) => <RemoteChatRow key={chat.agentId} sessionId={sessionId} copy={copy} chat={chat} />),
    );
}

function RemoteTerminalRow({ sessionId, copy, terminal }: { sessionId: string; copy: RemoteCopy; terminal: SessionInfo }) {
    return (
        <div className="agent-row-wrap">
            <button
                type="button"
                className="agent-row rail-tab"
                onClick={() => openRemoteTerminal(sessionId, copy.host.id, terminal, copy.project.path)}>
                <span className="agent-glyph term">
                    <IconTerminal size={15} />
                </span>
                <span className="agent-title">{terminal.windowTitle || terminal.title || "Terminal"}</span>
                <span className="agent-row-host">on {copy.host.name}</span>
            </button>
        </div>
    );
}

/** The terminals running in the project's copies elsewhere that are not open here. */
export function remoteTerminalRows({ sessionId, copies, open }: { sessionId: string; copies: readonly RemoteCopy[]; open: ReadonlySet<number> }) {
    return copies.flatMap((copy) =>
        terminalsIn(copy)
            .filter((terminal) => !open.has(terminal.id))
            .map((terminal) => <RemoteTerminalRow key={terminal.id} sessionId={sessionId} copy={copy} terminal={terminal} />),
    );
}
