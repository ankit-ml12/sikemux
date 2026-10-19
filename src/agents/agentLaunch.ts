import type { AgentEffort, AgentPermissionMode, AgentType } from "../state/types";
import { agentEfforts, canBypassPermissions, isAgentType } from "./catalog";

export const MAX_AGENT_MODEL_LENGTH = 256;

const SESSION_TOKEN = /^[A-Za-z0-9][A-Za-z0-9._:-]{0,199}$/;

/** Whether `id` has the shape the agent's own sessions take, so it can never pass for a flag. Pi and OMP also name a session by its transcript's path. */
export function isResumeId(type: AgentType, id: string): boolean {
    if (SESSION_TOKEN.test(id)) return true;
    return (
        (type === "pi" || type === "omp") && id.startsWith("/") && id.endsWith(".jsonl") && id.length <= 4_096 && ![...id].some((char) => char < " ")
    );
}

export const AGENT_PERMISSION_MODES: readonly AgentPermissionMode[] = ["workspace-write", "bypass"];

export function supportedPermissionModes(type: AgentType): readonly AgentPermissionMode[] {
    return canBypassPermissions(type) ? AGENT_PERMISSION_MODES : ["workspace-write"];
}

/** Agents the built-in chat can drive over ACP. Pi has no ACP mode of its own. */
export type ChatAgentType = Exclude<AgentType, "pi">;

export const CHAT_AGENT_TYPES: readonly ChatAgentType[] = ["codex", "claude", "opencode", "omp", "grok", "hermes", "muse"];

export function agentSupportsChat(type: AgentType): type is ChatAgentType {
    return (CHAT_AGENT_TYPES as readonly AgentType[]).includes(type);
}

export function normalizePermissionMode(type: AgentType, mode: AgentPermissionMode): AgentPermissionMode {
    return supportedPermissionModes(type).includes(mode) ? mode : "workspace-write";
}

/** The permission mode an agent runs in. */
export function effectiveMode(agent: { type: AgentType; permissionMode?: AgentPermissionMode; skipPermissions?: boolean }): AgentPermissionMode {
    return agent.permissionMode ?? (agent.skipPermissions ? "bypass" : normalizePermissionMode(agent.type, "workspace-write"));
}

export const AGENT_PERMISSION_COPY: Record<AgentPermissionMode, { label: string; detail: string; tone: "safe" | "balanced" | "open" | "danger" }> = {
    "read-only": {
        label: "Observe",
        detail: "Inspect and plan. Changes require a relaunch with a wider boundary.",
        tone: "safe",
    },
    "workspace-write": {
        label: "Normal",
        detail: "Use the provider's normal approval and sandbox behavior.",
        tone: "balanced",
    },
    "full-access": {
        label: "Operate",
        detail: "Use the whole machine, with the provider's approval flow intact.",
        tone: "open",
    },
    bypass: {
        label: "YOLO",
        detail: "Bypass approvals and sandboxing.",
        tone: "danger",
    },
};

/** Whether a mode lets the agent act without asking. "full-access" widens the sandbox but keeps the approvals. */
export const isYolo = (mode: string | null | undefined): boolean => mode === "bypass";

/* The core switches a running chat between safe and YOLO for every agent that
   has a YOLO mode: in the CLI's own modes where it has them, or else by
   answering the agent's permission requests itself. */
export const canSwitchMode = (type: string): boolean => isAgentType(type) && canBypassPermissions(type);

export function supportedEfforts(type: AgentType): readonly AgentEffort[] {
    return agentEfforts(type) as readonly AgentEffort[];
}
