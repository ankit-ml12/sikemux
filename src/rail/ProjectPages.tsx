import { type MouseEvent, type ReactNode, useState } from "react";
import { useShortcutLabel } from "../commands/useShortcutLabel";
import { basename, dirname, relativePath } from "../lib/paths";
import * as cmd from "../state/commands";
import { collectPanes } from "../state/layout";
import { activeTabRef, documentsOf, terminalLabel } from "../state/selectors";
import { getState, useStore } from "../state/store";
import type { Session, TabRef } from "../state/types";
import { FileIcon } from "../ui/FileIcon";
import { IconClose, IconPlus, IconTerminal } from "../ui/Icons";
import { Panel } from "../ui/Panel";
import { Tooltip } from "../ui/Tooltip";
import { fileTabMenu, windowTabMenu } from "../workspace/tabMenus";
import { type CtxItem, FileTree, TreeContextMenu } from "./FileTree";
import { NewRow } from "./RailNewRow";
import { RailToggle } from "./RailToggle";

const EMPTY_IDS: readonly string[] = [];
const openEntry = (entry: { path: string }) => cmd.requestOpenFile(entry.path);

type Menu = { x: number; y: number; items: CtxItem[] };

function PageHeader({ label }: { label: string }) {
    return (
        <div className="agent-header">
            <div className="agent-header-top">
                <span className="agent-header-label">{label}</span>
                <div className="agent-header-actions">
                    <RailToggle edge="end" />
                </div>
            </div>
        </div>
    );
}

function useProjectSession(): Session | undefined {
    const session = useStore((s) => s.sessions[s.activeSessionId]);
    return session?.kind === "project" ? session : undefined;
}

/** A tab of the session's strip, as a rail row: it selects the tab, closes it, and opens the strip's menu for it. */
function TabRow({
    tab,
    icon,
    label,
    detail,
    title,
    active,
    onMenu,
}: {
    tab: TabRef;
    icon: ReactNode;
    label: string;
    detail?: string | null;
    title?: string;
    active: boolean;
    onMenu: (event: MouseEvent) => void;
}) {
    return (
        <div className="agent-row-wrap">
            <button
                type="button"
                className={`agent-row rail-tab${active ? " active" : ""}`}
                title={title}
                onClick={() => cmd.selectTab(tab)}
                onAuxClick={(event) => {
                    if (event.button === 1) cmd.closeTab(tab);
                }}
                onContextMenu={onMenu}>
                <span className="agent-glyph">{icon}</span>
                <span className="agent-title">{label}</span>
                {detail && <span className="rail-file-folder">{detail}</span>}
            </button>
            <Tooltip label={`Close ${label}`}>
                <button type="button" className="row-x" aria-label={`Close ${label}`} onClick={() => cmd.closeTab(tab)}>
                    <IconClose size={11} />
                </button>
            </Tooltip>
        </div>
    );
}

function useTabMenu() {
    const [menu, setMenu] = useState<Menu | null>(null);
    const open = (event: MouseEvent, items: CtxItem[]) => {
        event.preventDefault();
        event.stopPropagation();
        setMenu({ x: event.clientX, y: event.clientY, items });
    };
    const shown = menu && <TreeContextMenu x={menu.x} y={menu.y} items={menu.items} onClose={() => setMenu(null)} />;
    return { open, shown };
}

/** The project's terminal tabs. */
export function TerminalsPage() {
    const session = useProjectSession();
    const windowIds = useStore((s) => (session ? s.windowsBySession[session.id] : undefined) ?? EMPTY_IDS);
    const windows = useStore((s) => s.windows);
    const titles = useStore((s) => s.terminalTitles);
    const closeShortcut = useShortcutLabel("pane.close");
    const menu = useTabMenu();

    const tabs = windowIds.flatMap((id) => {
        const win = windows[id];
        const home = win?.role === "term" ? collectPanes(win.root).find((pane) => pane.kind === "terminal") : undefined;
        return win && home ? [{ win, label: terminalLabel(win, home, titles) }] : [];
    });

    return (
        <>
            <PageHeader label="Terminals" />
            {!session ? (
                <div className="agent-empty">terminals are project-scoped</div>
            ) : (
                <>
                    <div className="agent-rail-top">
                        <div className="agent-actions">
                            <NewRow icon={<IconPlus size={15} />} title="New terminal" shortcut="terminal.new" onClick={cmd.newTerminal} />
                        </div>
                    </div>
                    <div className="rail-scroll">
                        {tabs.length === 0 ? (
                            <div className="agent-empty">no terminals open</div>
                        ) : (
                            <Panel variant="group" className="agent-group">
                                {tabs.map(({ win, label }) => (
                                    <TabRow
                                        key={win.id}
                                        tab={{ id: win.id }}
                                        icon={<IconTerminal size={15} />}
                                        label={label}
                                        active={session.activeWindowId === win.id}
                                        onMenu={(event) =>
                                            menu.open(event, windowTabMenu(session.id, getState().windows[win.id] ?? win, closeShortcut))
                                        }
                                    />
                                ))}
                            </Panel>
                        )}
                    </div>
                </>
            )}
            {menu.shown}
        </>
    );
}

/** The files open in the project's editor, then the project's tree, as one list. */
export function FilesPage() {
    const session = useProjectSession();
    const showing = useStore((s) => s.agentRailPage === "files" && s.agentRailOpen);
    const windowIds = useStore((s) => (session ? s.windowsBySession[session.id] : undefined) ?? EMPTY_IDS);
    const windows = useStore((s) => s.windows);
    const editorViews = useStore((s) => s.editorViews);
    const closeShortcut = useShortcutLabel("pane.close");
    const menu = useTabMenu();

    if (!session) {
        return (
            <>
                <PageHeader label="Files" />
                <div className="agent-empty">files are project-scoped</div>
            </>
        );
    }

    const cwd = session.cwd;
    const live = activeTabRef(session, windows, editorViews);
    const open: { id: string; path: string }[] = [];
    let activePath: string | null = null;
    for (const id of windowIds) {
        const win = windows[id];
        if (win?.role !== "files") continue;
        const documents = documentsOf(win, editorViews);
        activePath ??= documents?.activeId ?? null;
        for (const path of documents?.ids ?? EMPTY_IDS) open.push({ id, path });
    }

    const lead =
        open.length > 0 ? (
            <Panel variant="group" className="agent-group rail-open-files">
                {open.map(({ id, path }) => (
                    <TabRow
                        key={`${id}:${path}`}
                        tab={{ id, doc: path }}
                        icon={<FileIcon name={basename(path)} size={15} />}
                        label={basename(path)}
                        detail={relativePath(dirname(path), cwd)}
                        title={relativePath(path, cwd) ?? path}
                        active={live?.id === id && live.doc === path}
                        onMenu={(event) => {
                            const win = getState().windows[id];
                            if (win) menu.open(event, fileTabMenu(cwd, win, path, closeShortcut));
                        }}
                    />
                ))}
            </Panel>
        ) : undefined;

    return (
        <>
            <PageHeader label="Files" />
            <FileTree cwd={cwd} activePath={activePath} onOpenFile={openEntry} onKeepFile={openEntry} active={showing} lead={lead} />
            {menu.shown}
        </>
    );
}
