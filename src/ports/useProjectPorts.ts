import { useEffect, useMemo, useState } from "react";
import { coreCall } from "../api/core";
import { onCoreChanged } from "../api/coreChanged";
import { getState, useStore } from "../state/store";
import { swallow } from "../state/toast";
import { useTaskRuns } from "../tasks/coreTasks";
import { projectPorts, type CorePort, type ProjectPort } from "./projectPorts";

const NO_PORTS: CorePort[] = [];
const NO_SHOWN: ProjectPort[] = [];

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
    }, [cwd]);
    const ports = found && found.cwd === cwd ? found.ports : NO_PORTS;
    const runs = useTaskRuns();
    return useMemo(() => (sessionId && ports.length ? projectPorts(getState(), sessionId, ports, runs) : NO_SHOWN), [sessionId, ports, runs]);
}
