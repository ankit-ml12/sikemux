/* Small marks in Jira's own colours, so a card reads at a glance the way it does in Jira. */

const TYPE_COLOURS: Record<string, string> = {
    bug: "#e5493a",
    task: "#4bade8",
    story: "#63ba3c",
    epic: "#904ee2",
    subtask: "#4bade8",
    "sub-task": "#4bade8",
    feature: "#63ba3c",
};

/** The issue type's mark: a coloured tile with Jira's glyph for bugs, tasks, stories and epics. */
export function TypeMark({ type, size = 14 }: { type: string | null; size?: number }) {
    const name = (type ?? "").toLowerCase();
    const colour = TYPE_COLOURS[name] ?? "#8590a2";
    return (
        <svg width={size} height={size} viewBox="0 0 16 16" role="img" aria-label={type ?? "Issue"}>
            <title>{type ?? "Issue"}</title>
            <rect width="16" height="16" rx="3" fill={colour} />
            {name === "bug" ? (
                <circle cx="8" cy="8" r="3.4" fill="#fff" />
            ) : name === "story" || name === "feature" ? (
                <path d="M5 3.5h6v9L8 10.4 5 12.5z" fill="#fff" />
            ) : name === "epic" ? (
                <path d="M9.4 2.8 4.6 9h3l-1 4.2L11.4 7h-3z" fill="#fff" />
            ) : name === "subtask" || name === "sub-task" ? (
                <path d="M4.5 4.5h4v4h-4zM7.5 7.5h4v4h-4z" fill="none" stroke="#fff" strokeWidth="1.4" />
            ) : (
                <path d="m4.4 8.2 2.3 2.3 4.9-5" fill="none" stroke="#fff" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" />
            )}
        </svg>
    );
}

/** Priority as Jira draws it: arrows up in red for urgent, an orange equals for medium, arrows down in blue for low. */
export function PriorityMark({ priority, size = 14 }: { priority: string | null; size?: number }) {
    if (!priority) return null;
    const name = priority.toLowerCase();
    const shape =
        name === "highest" ? (
            <path d="M3 9.5 8 5l5 4.5M3 13 8 8.5l5 4.5" stroke="#e5493a" />
        ) : name === "high" ? (
            <path d="M3 10.5 8 6l5 4.5" stroke="#e5493a" />
        ) : name === "low" ? (
            <path d="M3 5.5 8 10l5-4.5" stroke="#2a7fff" />
        ) : name === "lowest" ? (
            <path d="M3 3 8 7.5 13 3M3 6.5 8 11l5-4.5" stroke="#2a7fff" />
        ) : (
            <path d="M3 6h10M3 10h10" stroke="#f79232" />
        );
    return (
        <svg
            width={size}
            height={size}
            viewBox="0 0 16 16"
            role="img"
            aria-label={`${priority} priority`}
            fill="none"
            strokeWidth="1.8"
            strokeLinecap="round"
            strokeLinejoin="round">
            <title>{`${priority} priority`}</title>
            {shape}
        </svg>
    );
}

export function CalendarMark({ size = 12 }: { size?: number }) {
    return (
        <svg width={size} height={size} viewBox="0 0 16 16" aria-hidden fill="none" stroke="currentColor" strokeWidth="1.5">
            <rect x="2" y="3" width="12" height="11" rx="2" />
            <path d="M2 6.5h12M5.5 1.5v3M10.5 1.5v3" strokeLinecap="round" />
        </svg>
    );
}
