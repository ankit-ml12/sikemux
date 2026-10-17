import { type MouseEvent, type ReactNode, useState } from "react";
import { basename, dirname, relativePath } from "../lib/paths";
import * as cmd from "../state/commands";
import { collectPanes } from "../state/layout";
import { forgetRecentFile } from "../state/recentFiles";
import { activeTabRef, documentsOf, terminalLabel } from "../state/selectors";
import { useStore } from "../state/store";
import type { Session } from "../state/types";
import { FileIcon } from "../ui/FileIcon";
import { IconClose, IconPlus, IconSearch, IconTerminal } from "../ui/Icons";
import { Panel, PanelHeader } from "../ui/Panel";
import { TreeContextMenu } from "./FileTree";
import { NewRow } from "./RailNewRow";
import { RailToggle } from "./RailToggle";

const RECENT_SHOWN = 15;
const EMPTY_IDS: readonly string[] = [];

type Menu = { x: number; y: number; items: { label: string; icon: ReactNode; run: () => void }[] };

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

/** The project's open terminals, one row for each terminal in each terminal tab. */
export function TerminalsPage() {
    const session = useProjectSession();
    const windowIds = useStore((s) => (session ? s.windowsBySession[session.id] : undefined) ?? EMPTY_IDS);
    const windows = useStore((s) => s.windows);
    const titles = useStore((s) => s.terminalTitles);
    const [menu, setMenu] = useState<Menu | null>(null);

    const rows = windowIds.flatMap((id) => {
        const win = windows[id];
        if (win?.role !== "term") return [];
        const panes = collectPanes(win.root).filter((pane) => pane.kind === "terminal");
        return panes.map((pane) => ({
            win,
            pane,
            active: session?.activeWindowId === win.id && (panes.length === 1 || win.activePaneId === pane.id),
        }));
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
                        {rows.length === 0 ? (
                            <div className="agent-empty">no terminals open</div>
                        ) : (
                            <Panel variant="group" className="agent-group">
                                {rows.map(({ win, pane, active }) => (
                                    <button
                                        key={pane.id}
                                        type="button"
                                        className={`agent-row${active ? " active" : ""}`}
                                        onClick={() => {
                                            cmd.selectWindowId(win.id);
                                            cmd.focusPane(pane.id);
                                        }}
                                        onContextMenu={(event) => {
                                            event.preventDefault();
                                            setMenu({
                                                x: event.clientX,
                                                y: event.clientY,
                                                items: [
                                                    { label: "Close tab", icon: <IconClose size={13} />, run: () => cmd.closeTab({ id: win.id }) },
                                                ],
                                            });
                                        }}>
                                        <span className="agent-glyph term">
                                            <IconTerminal size={15} />
                                        </span>
                                        <span className="agent-title">{terminalLabel(win, pane, titles)}</span>
                                    </button>
                                ))}
                            </Panel>
                        )}
                    </div>
                </>
            )}
            {menu && <TreeContextMenu x={menu.x} y={menu.y} items={menu.items} onClose={() => setMenu(null)} />}
        </>
    );
}

function FileRow({
    path,
    cwd,
    active,
    onClick,
    onContextMenu,
}: {
    path: string;
    cwd: string;
    active?: boolean;
    onClick: () => void;
    onContextMenu: (event: MouseEvent) => void;
}) {
    const folder = relativePath(dirname(path), cwd);
    return (
        <button
            type="button"
            className={`agent-row rail-file${active ? " active" : ""}`}
            title={relativePath(path, cwd) ?? path}
            onClick={onClick}
            onContextMenu={onContextMenu}>
            <span className="agent-glyph">
                <FileIcon name={basename(path)} size={15} />
            </span>
            <span className="agent-title">{basename(path)}</span>
            {folder && <span className="rail-file-folder">{folder}</span>}
        </button>
    );
}

/** The files open in the project's editor, then the ones closed there lately. */
export function FilesPage() {
    const session = useProjectSession();
    const windowIds = useStore((s) => (session ? s.windowsBySession[session.id] : undefined) ?? EMPTY_IDS);
    const windows = useStore((s) => s.windows);
    const editorViews = useStore((s) => s.editorViews);
    const recentFiles = useStore((s) => s.recentFiles);
    const [menu, setMenu] = useState<Menu | null>(null);

    const cwd = session?.cwd ?? "";
    const live = session ? activeTabRef(session, windows, editorViews) : null;
    const open = windowIds.flatMap((id) => {
        const win = windows[id];
        if (win?.role !== "files") return [];
        return (documentsOf(win, editorViews)?.ids ?? EMPTY_IDS).map((path) => ({ id, path }));
    });
    const openPaths = new Set(open.map((file) => file.path));
    const recent = cwd ? recentFiles.filter((path) => !openPaths.has(path) && relativePath(path, cwd)).slice(0, RECENT_SHOWN) : [];

    const menuAt = (event: MouseEvent, items: Menu["items"]) => {
        event.preventDefault();
        setMenu({ x: event.clientX, y: event.clientY, items });
    };

    return (
        <>
            <PageHeader label="Files" />
            {!session ? (
                <div className="agent-empty">files are project-scoped</div>
            ) : (
                <>
                    <div className="agent-rail-top">
                        <div className="agent-actions">
                            <NewRow icon={<IconSearch size={15} />} title="Open file" shortcut="palette.files" onClick={cmd.openFilePalette} />
                        </div>
                    </div>
                    <div className="rail-scroll">
                        {open.length === 0 && recent.length === 0 && <div className="agent-empty">no files open</div>}
                        {open.length > 0 && (
                            <Panel variant="group" className="agent-group">
                                {open.map(({ id, path }) => (
                                    <FileRow
                                        key={`${id}:${path}`}
                                        path={path}
                                        cwd={cwd}
                                        active={live?.id === id && live.doc === path && session.activeWindowId === id}
                                        onClick={() => cmd.selectTab({ id, doc: path })}
                                        onContextMenu={(event) =>
                                            menuAt(event, [
                                                { label: "Close", icon: <IconClose size={13} />, run: () => cmd.closeTab({ id, doc: path }) },
                                            ])
                                        }
                                    />
                                ))}
                            </Panel>
                        )}
                        {recent.length > 0 && (
                            <Panel variant="group" className="agent-group">
                                <PanelHeader label="Recent" rule />
                                {recent.map((path) => (
                                    <FileRow
                                        key={path}
                                        path={path}
                                        cwd={cwd}
                                        onClick={() => cmd.requestOpenFile(path)}
                                        onContextMenu={(event) =>
                                            menuAt(event, [
                                                { label: "Remove from recent", icon: <IconClose size={13} />, run: () => forgetRecentFile(path) },
                                            ])
                                        }
                                    />
                                ))}
                            </Panel>
                        )}
                    </div>
                </>
            )}
            {menu && <TreeContextMenu x={menu.x} y={menu.y} items={menu.items} onClose={() => setMenu(null)} />}
        </>
    );
}
