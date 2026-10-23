import type { AgentBuddy } from "../state/types";

function name(value: unknown): string | undefined {
    return typeof value === "string" && value.trim() && !/[\0\r\n]/.test(value) ? value.slice(0, 40) : undefined;
}

/** An agent's buddy read from anywhere outside the app, or null when it is not one. */
export function toAgentBuddy(value: unknown): AgentBuddy | null {
    if (typeof value !== "object" || value === null) return null;
    const fields = value as Record<string, unknown>;
    if (!Number.isInteger(fields.color) || (fields.color as number) < 0 || (fields.color as number) > 63) return null;
    const buddy: AgentBuddy = { color: fields.color as number };
    const pack = name(fields.pack);
    const shape = name(fields.shape);
    if (pack && shape) Object.assign(buddy, { pack, shape });
    return buddy;
}
