import { useEffect, useRef, useState, type ReactNode } from "react";
import { IconChevron } from "../../../plugin-api/ui";

export interface MenuOption {
    value: string;
    label: string;
    count?: number;
    /** Drawn before the label, such as an avatar or a type's mark. */
    mark?: ReactNode;
}

const SEARCH_FROM = 8;

/**
 * A toolbar button that opens a list to pick from, as on Jira's board.
 * With `single` it picks one value and closes; otherwise it ticks any number.
 */
export function FilterMenu({
    label,
    options,
    picked,
    onToggle,
    onClear,
    single = false,
    current,
    empty = "Nothing to pick on this board",
}: {
    label: string;
    options: MenuOption[];
    picked: ReadonlySet<string>;
    onToggle: (value: string) => void;
    onClear?: () => void;
    single?: boolean;
    /** What the button shows after its label for a single pick, such as "Assignee". */
    current?: string;
    empty?: string;
}) {
    const [open, setOpen] = useState(false);
    const [words, setWords] = useState("");
    const holder = useRef<HTMLDivElement | null>(null);

    useEffect(() => {
        if (!open) return;
        const away = (event: PointerEvent) => {
            if (!holder.current?.contains(event.target as Node)) setOpen(false);
        };
        const escape = (event: KeyboardEvent) => {
            if (event.key === "Escape") setOpen(false);
        };
        window.addEventListener("pointerdown", away);
        window.addEventListener("keydown", escape);
        return () => {
            window.removeEventListener("pointerdown", away);
            window.removeEventListener("keydown", escape);
        };
    }, [open]);

    const count = single ? 0 : options.filter((option) => picked.has(option.value)).length;
    const needle = words.trim().toLowerCase();
    const shown = needle ? options.filter((option) => option.label.toLowerCase().includes(needle)) : options;

    return (
        <div className="jira-menu" ref={holder}>
            <button
                type="button"
                className={`jira-menu-button${count > 0 || (single && current) ? " picked" : ""}`}
                aria-haspopup="true"
                aria-expanded={open}
                onClick={() => {
                    setOpen(!open);
                    setWords("");
                }}>
                {label}
                {single && current && <span className="jira-menu-current">{current}</span>}
                {count > 0 && <span className="jira-menu-count">{count}</span>}
                <IconChevron size={9} />
            </button>
            {open && (
                <div className="jira-menu-list" role="listbox" aria-label={label} aria-multiselectable={!single}>
                    {options.length > SEARCH_FROM && (
                        <input
                            className="jira-menu-search"
                            value={words}
                            onChange={(event) => setWords(event.target.value)}
                            placeholder={`Search ${label.toLowerCase()}`}
                            aria-label={`Search ${label.toLowerCase()}`}
                            autoFocus
                            spellCheck={false}
                        />
                    )}
                    {shown.length === 0 && <div className="jira-menu-empty">{options.length ? "No match" : empty}</div>}
                    {shown.map((option) => {
                        const on = single ? option.value === current || picked.has(option.value) : picked.has(option.value);
                        return (
                            <button
                                key={option.value}
                                type="button"
                                role="option"
                                aria-selected={on}
                                className="jira-menu-option"
                                onClick={() => {
                                    onToggle(option.value);
                                    if (single) setOpen(false);
                                }}>
                                {!single && <span className={`jira-menu-tick${on ? " on" : ""}`} aria-hidden />}
                                {option.mark}
                                <span className="jira-menu-label">{option.label}</span>
                                {option.count !== undefined && <span className="jira-menu-option-count">{option.count}</span>}
                            </button>
                        );
                    })}
                    {!single && count > 0 && onClear && (
                        <button type="button" className="jira-menu-clear" onClick={onClear}>
                            Clear {label.toLowerCase()}
                        </button>
                    )}
                </div>
            )}
        </div>
    );
}
