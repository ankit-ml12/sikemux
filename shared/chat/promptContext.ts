import type { SentContext } from "./types";

export interface ContextChip {
    kind: "issue" | "pull" | null;
    number: string | null;
    title: string;
}

export function contextChip(item: SentContext): ContextChip {
    const tracked = /\/(issues|pull|pulls|pull-requests|merge_requests)\/(\d+)(?:[/?#]|$)/.exec(item.uri);
    const number = tracked?.[2] ?? null;
    const title = number ? item.title.replace(new RegExp(`^(?:(?:Issue|Pull request) )?#${number}:?\\s*`), "") : item.title;
    return { kind: tracked ? (tracked[1] === "issues" ? "issue" : "pull") : null, number, title: title || item.uri };
}
