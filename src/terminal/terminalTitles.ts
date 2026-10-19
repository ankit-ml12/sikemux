import { coreSessionsApi } from "../api/coreSessions";
import { getIpcTransport } from "../api/transport";
import { setState } from "../state/store";
import { swallow } from "../state/toast";

function note(id: number, title: string | null): void {
    setState((s) => {
        const titles = { ...s.terminalTitles };
        if (title) titles[id] = title;
        else delete titles[id];
        return { terminalTitles: titles };
    });
}

/** Keeps every core terminal's title as the core reports it, for as long as the page runs. */
export function watchTerminalTitles(): () => void {
    const controller = new AbortController();
    const { signal } = controller;
    const transport = getIpcTransport();
    const fail = (error: unknown) => {
        if (!signal.aborted) swallow("terminal title listener")(error);
    };
    void transport
        .subscribe<{ id: number; title: string }>("terminal_title", (event) => note(event.payload.id, event.payload.title), { signal })
        .catch(fail);
    void transport.subscribe<{ id: number }>("pty_exited", (event) => note(event.payload.id, null), { signal }).catch(fail);
    void coreSessionsApi
        .list()
        .then((sessions) => {
            for (const session of sessions) if (session.windowTitle && !signal.aborted) note(session.id, session.windowTitle);
        })
        .catch(fail);
    return () => controller.abort();
}
