import { coreCall } from "./core";
import type { ChatInfo } from "./generated/wire";
import { invokeCommand as invoke } from "./invoke";
import { getIpcTransport, type IpcUnsubscribe } from "./transport";
import type { AgentPermissionMode, AgentType } from "../state/types";

export interface AcpStartOptions {
    agentId: string;
    provider: AgentType;
    cwd: string;
    resumeId?: string;
    permissionMode: AgentPermissionMode;
    configPath?: string;
    executablePath?: string;
    model?: string;
    effort?: string;
    environmentKeys?: string[];
    /** The account the agent signs in as. */
    account?: AcpAccount;
    /** The model provider the chat runs on, by catalogue id, in place of its maker's account. */
    modelProvider?: string;
}

/** One of the person's accounts with the chat's provider: a profile, by its directory. */
export interface AcpAccount {
    id: string;
    label: string;
    configPath?: string;
}

export interface AcpStartResponse {
    sessionId: string;
    capabilities: Record<string, unknown>;
    setup: Record<string, unknown>;
}

/** Whether the background core still runs a chat for the agent. A live one
    replays what it said so far as `acp_event`s before any new event. */
export type AcpAttachment =
    | {
          status: "live";
          start: AcpStartResponse;
          permissionMode: AgentPermissionMode;
          running: boolean;
          /** A turn ran in this session, so the provider keeps it. */
          turned: boolean;
          /** The core's run of this chat, which paging back names. */
          feed: string;
          /** Set when the replay holds only the chat's last turns: `history` pages back from it. */
          olderBefore: number | null;
      }
    | { status: "missing" };

/** Turns from before what the page shows, and where the page before them starts. */
export interface AcpHistory {
    events: { kind: string; payload: Record<string, unknown> }[];
    olderBefore: number | null;
}

export interface AcpAttachOptions {
    agentId: string;
    provider: AgentType;
    cwd: string;
    configPath?: string;
}

/** Something read elsewhere and handed to the agent whole, such as an issue and its comments. */
export interface PromptContext {
    uri: string;
    title: string;
    text: string;
}

export interface AcpEvent {
    agentId: string;
    /** `reattach`: the core came back with this chat still running, so take it up again. */
    /** `prompt`: what someone on another device sent the agent. */
    kind: "status" | "ready" | "session_update" | "prompt" | "turn_started" | "turn_completed" | "permission_request" | "error" | "reattach";
    /** A `session_update` carries `updates`: a frame's worth of them at once. */
    payload: Record<string, unknown>;
}

export const acpApi = {
    start: (options: AcpStartOptions): Promise<AcpStartResponse> =>
        invoke<AcpStartResponse>("acp_start", {
            agentId: options.agentId,
            provider: options.provider,
            cwd: options.cwd,
            resumeId: options.resumeId ?? null,
            permissionMode: options.permissionMode,
            configPath: options.configPath ?? null,
            executablePath: options.executablePath ?? null,
            model: options.model ?? null,
            effort: options.effort ?? null,
            environmentKeys: options.environmentKeys ?? [],
            account: options.account ?? null,
            modelProvider: options.modelProvider ?? null,
        }),
    attach: (options: AcpAttachOptions): Promise<AcpAttachment> =>
        invoke<AcpAttachment>("acp_attach", {
            agentId: options.agentId,
            provider: options.provider,
            cwd: options.cwd,
            configPath: options.configPath ?? null,
        }),
    history: (agentId: string, feed: string, before: number, turns: number): Promise<AcpHistory> =>
        invoke<AcpHistory>("acp_history", { agentId, feed, before, turns }),
    list: () => invoke<ChatInfo[]>("acp_list"),
    setPermissionMode: (agentId: string, permissionMode: AgentPermissionMode): Promise<void> =>
        invoke<void>("acp_set_permission_mode", { agentId, permissionMode }),
    setConfig: (agentId: string, configId: string, value: string): Promise<Record<string, unknown>> =>
        invoke("acp_set_config", { agentId, configId, value }),
    prompt: (agentId: string, text: string, paths: string[], context: PromptContext[] = [], messageId?: string): Promise<void> =>
        invoke<void>("acp_prompt", { agentId, messageId: messageId ?? null, text, paths, context }),
    edit: (agentId: string, messageId: string, text: string, paths: string[], context: PromptContext[]): Promise<void> =>
        invoke<void>("acp_edit", { agentId, messageId, text, paths, context }),
    steer: (agentId: string, text: string, paths: string[], context: PromptContext[] = []): Promise<string> =>
        invoke<string>("acp_steer", { agentId, text, paths, context }),
    /** Puts the waiting messages `ids`, then `message` when given, into the running turn as one; a turn that just ended starts again with them. */
    steerQueued: (agentId: string, ids: string[], message?: { text: string; paths: string[]; context: PromptContext[] }): Promise<void> =>
        coreCall<void>("steerQueued", { agentId, ids, message: message ?? null }),
    /** Takes back waiting messages before they go out. */
    dropQueued: (agentId: string, ids: string[]): Promise<void> => coreCall<void>("dropQueued", { agentId, ids }),
    /** Stops the running turn; the messages waiting behind it are taken back. */
    cancel: (agentId: string): Promise<void> => invoke<void>("acp_cancel", { agentId }),
    stopTask: (agentId: string, taskId: string): Promise<void> => invoke<void>("acp_stop_task", { agentId, taskId }),
    permissionReply: (agentId: string, requestId: string, optionId?: string): Promise<void> =>
        invoke<void>("acp_permission_reply", { agentId, requestId, optionId: optionId ?? null }),
    stop: (agentId: string): Promise<void> => invoke<void>("acp_stop", { agentId }),
    /** Stops the chat unless its agent is still at work, which only the person stops. Answers whether it stopped. */
    stopIfIdle: (agentId: string): Promise<boolean> => coreCall<{ stopped: boolean }>("stopChatIfIdle", { agentId }).then(({ stopped }) => stopped),
    switchAccount: (agentId: string, provider: AgentType, account: AcpAccount): Promise<void> =>
        invoke<void>("acp_switch_account", { agentId, provider, account }),
    subscribe: (listener: (event: AcpEvent) => void, signal?: AbortSignal): Promise<IpcUnsubscribe> =>
        getIpcTransport().subscribe<AcpEvent>("acp_event", (event) => listener(event.payload), { signal }),
};
