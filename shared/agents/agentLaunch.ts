import { canBypassPermissions, isAgentType } from "./catalog";

/** Whether a mode lets the agent act without asking. */
export const isYolo = (mode: string | null | undefined): boolean => mode === "bypass";

/* The core switches a running chat between safe and YOLO for every agent that
   has a YOLO mode: in the CLI's own modes where it has them, or else by
   answering the agent's permission requests itself. */
export const canSwitchMode = (type: string): boolean => isAgentType(type) && canBypassPermissions(type);
