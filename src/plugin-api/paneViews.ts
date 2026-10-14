import { create } from "zustand";
import { onPaneClosed } from "./host";

export interface PaneViews<V> {
    use(paneId: string): V;
    get(paneId: string): V;
    update(paneId: string, change: Partial<V>): void;
}

/**
 * What each pane of a plugin's surface shows, starting from `initial` and forgotten when the pane closes.
 * `initial` may be a function, asked once per pane, for a start that depends on what was saved.
 * `onClose` sees a pane's last view as it goes.
 */
export function definePaneViews<V extends object>(initial: V | (() => V), onClose?: (paneId: string, view: V) => void): PaneViews<V> {
    const useViews = create<{ views: Record<string, V> }>()(() => ({ views: {} }));
    const starts = new Map<string, V>();
    const start = (paneId: string): V => {
        if (typeof initial !== "function") return initial;
        let view = starts.get(paneId);
        if (!view) {
            view = initial();
            starts.set(paneId, view);
        }
        return view;
    };

    onPaneClosed((paneId) => {
        const { views } = useViews.getState();
        if (!(paneId in views)) return;
        onClose?.(paneId, views[paneId]);
        starts.delete(paneId);
        const rest = { ...views };
        delete rest[paneId];
        useViews.setState({ views: rest });
    });

    return {
        use: (paneId) => useViews((state) => state.views[paneId] ?? start(paneId)),
        get: (paneId) => useViews.getState().views[paneId] ?? start(paneId),
        update: (paneId, change) =>
            useViews.setState((state) => ({ views: { ...state.views, [paneId]: { ...(state.views[paneId] ?? start(paneId)), ...change } } })),
    };
}
