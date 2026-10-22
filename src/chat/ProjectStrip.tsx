import { useEffect, useId, useMemo, useRef, useState, type ReactNode } from "react";
import { useProjectRepo } from "../git/projectRepo";
import { fuzzyScore, isSubstringMatch } from "../lib/fuzzy";
import { leavingMenu } from "../lib/motion";
import { basename, prettyPath } from "../lib/paths";
import * as cmd from "../state/commands";
import { useResource, useResourceEnabled } from "../state/resources";
import { gitOverviewR, projectRootsScanR } from "../state/resources.defs";
import { useStore } from "../state/store";
import { IconChevron, IconFolder, IconGit } from "../ui/Icons";
import { THIS_HOST, hostName } from "../hosts/hosts";
import { hostMark, projectMark } from "../hosts/placeMarks";
import { WorktreeToggle } from "./ChatWorktree";
import type { WorktreeSwitchState } from "./worktreeSwitch";
import "../styles/chat/project-strip.css";
import { projectRootsOf } from "../state/preferences";

interface Choice {
    path: string;
    name: string;
    sub: string;
    open: boolean;
}

/** Keeps a long path's last folders on screen when it is cut short from the start. */
const ltr = (text: string) => `‎${text}‎`;

function ProjectSwitcher({ agentId, cwd, onClose }: { agentId: string; cwd: string; onClose: () => void }) {
    const home = useStore((s) => s.home);
    const sessionsById = useStore((s) => s.sessions);
    const sessionOrder = useStore((s) => s.sessionOrder);
    const projectRoots = useStore(projectRootsOf);
    const scanned = useResourceEnabled(projectRoots.length > 0, projectRootsScanR, projectRoots).data;
    const [query, setQuery] = useState("");
    const [selected, setSelected] = useState(0);
    const listId = useId();
    const search = useRef<HTMLInputElement>(null);

    useEffect(() => search.current?.focus(), []);

    const choices = useMemo(() => {
        const open: Choice[] = sessionOrder
            .map((id) => sessionsById[id])
            .filter((session) => session?.kind === "project")
            .map((session) => ({ path: session.cwd, name: basename(session.cwd) || session.name, sub: prettyPath(session.cwd, home), open: true }));
        const opened = new Set(open.map((choice) => choice.path));
        const found: Choice[] = (scanned ?? [])
            .filter((entry) => !opened.has(entry.path))
            .map((entry) => ({ path: entry.path, name: basename(entry.path), sub: prettyPath(entry.path, home), open: false }));
        const scored = [...open, ...found]
            .map((choice) => ({ choice, score: fuzzyScore(query, `${choice.name} ${choice.sub}`) }))
            .filter((row) => row.score >= 0);
        const exact = scored.some((row) => isSubstringMatch(row.score));
        const kept = exact ? scored.filter((row) => isSubstringMatch(row.score)) : scored;
        if (query.trim()) kept.sort((a, b) => a.score - b.score);
        return kept.map((row) => row.choice);
    }, [sessionOrder, sessionsById, scanned, query, home]);

    const pick = (choice: Choice | undefined) => {
        if (!choice) return;
        onClose();
        if (choice.path !== cwd) void cmd.moveAgentToProject(agentId, choice.path);
    };

    const grouped = !query.trim();
    return (
        <div ref={leavingMenu} className="chat-picker-menu compact project-switcher">
            <input
                ref={search}
                value={query}
                aria-label="Search projects"
                placeholder="Search projects…"
                role="combobox"
                aria-controls={listId}
                aria-expanded
                aria-autocomplete="list"
                aria-activedescendant={choices[selected] ? `${listId}-${selected}` : undefined}
                onChange={(event) => {
                    setQuery(event.target.value);
                    setSelected(0);
                }}
                onKeyDown={(event) => {
                    if (event.key === "Escape") {
                        event.preventDefault();
                        event.stopPropagation();
                        onClose();
                    }
                    if (event.key === "ArrowDown" || event.key === "ArrowUp") {
                        event.preventDefault();
                        setSelected((index) =>
                            choices.length ? (index + (event.key === "ArrowDown" ? 1 : choices.length - 1)) % choices.length : 0,
                        );
                    }
                    if (event.key === "Enter" && !event.nativeEvent.isComposing) {
                        event.preventDefault();
                        pick(choices[selected]);
                    }
                }}
            />
            <div id={listId} role="listbox" aria-label="Projects" className="chat-picker-options">
                {choices.map((choice, index) => (
                    <div key={choice.path} role="presentation">
                        {grouped && (index === 0 || choices[index - 1].open !== choice.open) && (
                            <div className="project-switcher-group" role="presentation">
                                {choice.open ? "Open" : "Found on disk"}
                            </div>
                        )}
                        <button
                            type="button"
                            id={`${listId}-${index}`}
                            role="option"
                            aria-selected={choice.path === cwd}
                            className={`${index === selected ? "highlighted" : ""}${choice.open ? " open" : ""}`}
                            onMouseEnter={() => setSelected(index)}
                            onMouseDown={(event) => event.preventDefault()}
                            onClick={() => pick(choice)}>
                            <IconFolder size={12} />
                            <strong>{choice.name}</strong>
                            <small>{ltr(choice.sub)}</small>
                        </button>
                    </div>
                ))}
                {choices.length === 0 && <div className="chat-picker-hint">No matches</div>}
            </div>
        </div>
    );
}

/**
 * Where the chat runs, docked on its composer: the host, the project and the branch or worktree on the left, the
 * account on the right. Until the first message the project opens a switcher that moves the chat elsewhere.
 */
export function ProjectStrip({
    agentId,
    host,
    project,
    cwd,
    started,
    worktree,
    pull,
    account,
}: {
    agentId: string;
    host: string | undefined;
    /** The project's own folder, which a chat in a worktree does not run in. */
    project: string;
    cwd: string;
    started: boolean;
    worktree: { state: WorktreeSwitchState; toggle: () => void };
    pull: ReactNode;
    account: ReactNode;
}) {
    const home = useStore((s) => s.home);
    const hostLabel = useStore((s) => hostName(s, host) ?? "This host");
    const repo = useProjectRepo(cwd).repo ?? cwd;
    const branch = useResource(gitOverviewR, repo).data?.status?.branch;
    const [open, setOpen] = useState(false);
    const root = useRef<HTMLDivElement>(null);
    const trigger = useRef<HTMLButtonElement>(null);

    useEffect(() => {
        if (!open) return;
        const outside = (event: PointerEvent) => {
            if (event.target instanceof Node && !root.current?.contains(event.target)) setOpen(false);
        };
        document.addEventListener("pointerdown", outside);
        return () => document.removeEventListener("pointerdown", outside);
    }, [open]);

    const name = basename(project) || project;
    const where = worktree.state.kind === "hidden" || worktree.state.kind === "choosing" ? null : worktree.state;
    return (
        <div className="chat-project-strip">
            <span className="chat-project-host" title={hostLabel}>
                {hostMark(host ? { kind: "core", host } : THIS_HOST, 12)}
                <span>{hostLabel}</span>
            </span>
            <IconChevron size={10} className="chat-project-sep" />
            {started ? (
                <span className="chat-project-trigger static" title={prettyPath(project, home)}>
                    {projectMark(12)}
                    <span>{name}</span>
                </span>
            ) : (
                <div
                    className="chat-project"
                    ref={root}
                    onBlur={(event) => {
                        // WebKit hands focus to nobody when a button is clicked, so a blur
                        // with no new target is a click on our own menu, not a click away.
                        if (event.relatedTarget && !event.currentTarget.contains(event.relatedTarget)) setOpen(false);
                    }}>
                    <button
                        ref={trigger}
                        type="button"
                        className="chat-project-trigger"
                        aria-label="Project"
                        aria-haspopup="listbox"
                        aria-expanded={open}
                        title={prettyPath(project, home)}
                        onClick={() => setOpen(!open)}>
                        {projectMark(12)}
                        <span>{name}</span>
                        <IconChevron size={10} />
                    </button>
                    {open && (
                        <ProjectSwitcher
                            agentId={agentId}
                            cwd={cwd}
                            onClose={() => {
                                setOpen(false);
                                trigger.current?.focus();
                            }}
                        />
                    )}
                </div>
            )}
            {where ? (
                <WorktreeToggle state={where} onToggle={worktree.toggle} />
            ) : (
                branch && (
                    <span className="chat-project-branch" title={branch}>
                        <IconGit size={11} />
                        <span>{branch}</span>
                    </span>
                )
            )}
            {worktree.state.kind === "choosing" && <WorktreeToggle state={worktree.state} onToggle={worktree.toggle} />}
            {pull}
            <span className="chat-project-spacer" />
            {account}
        </div>
    );
}
