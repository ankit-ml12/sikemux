import { useEffect, useMemo, useState } from "react";
import { coreCall } from "../api/core";
import { onCoreChanged } from "../api/coreChanged";
import { getState, useStore } from "../state/store";
import { swallow } from "../state/toast";
import { useTaskRuns } from "../tasks/coreTasks";
import { projectPorts, type CorePort, type ProjectPort } from "./projectPorts";

export const PORT_REFRESH_MS = 5_000;
/** A server binds its port a moment after its task starts. */
export const PORT_SETTLE_MS = 1_500;

/** The ports the project's terminals, tasks and agents listen on, read again whenever the core says they changed. */
export function useProjectPorts(sessionId: string | null): ProjectPort[] {
    const cwd = useStore((state) => (sessionId ? (state.sessions[sessionId]?.cwd ?? null) : null));
    const [found, setFound] = useState<{ cwd: string; ports: CorePort[] } | null>(null);
    useEffect(() => {
        if (!cwd) return;
        let cancelled = false;
        const read = () =>
            void coreCall<CorePort[]>("ports", { project: cwd })
                .then((ports) => {
                    if (!cancelled) setFound({ cwd, ports });
                })
                .catch(swallow("listening ports"));
        read();
        const unsubscribe = onCoreChanged("ports", read);
        return () => {
            cancelled = true;
            unsubscribe();
        };
    }, [sessionId, live, visible, previewUrl]);

    return found && found.sessionId === sessionId ? found.ports : NO_PORTS;
}
