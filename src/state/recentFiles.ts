import { deskEditorId } from "./desks";
import { setState, useStore, type StoreState } from "./store";

export const RECENT_FILES_LIMIT = 50;
const DESK_PREFIX = deskEditorId("");

/** The files `previous` had open in an editor that `next` no longer has, an agent's desk aside. */
function closedFiles(previous: StoreState["editorViews"], next: StoreState["editorViews"]): string[] {
    const closed: string[] = [];
    for (const [paneId, view] of Object.entries(previous)) {
        if (paneId.startsWith(DESK_PREFIX) || next[paneId] === view) continue;
        const still = new Set(next[paneId]?.openTabs);
        for (const path of view.openTabs) if (!still.has(path)) closed.push(path);
    }
    return closed;
}

/** Every file closed in an editor goes to the front of the recent files. */
export function recordRecentFiles(): () => void {
    return useStore.subscribe((state, previous) => {
        if (state.editorViews === previous.editorViews) return;
        const closed = closedFiles(previous.editorViews, state.editorViews);
        if (closed.length === 0) return;
        setState((s) => ({ recentFiles: [...new Set([...closed.reverse(), ...s.recentFiles])].slice(0, RECENT_FILES_LIMIT) }));
    });
}

export function forgetRecentFile(path: string): void {
    setState((s) => ({ recentFiles: s.recentFiles.filter((recent) => recent !== path) }));
}
