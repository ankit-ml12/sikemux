import { useEffect } from "react";
import { agentBoardApi } from "../api/agentBoard";
import { onCoreChanged } from "../api/coreChanged";
import * as cmd from "../state/commands";
import { swallow } from "../state/toast";

/** Keeps the app's copy of each project's board, read again whenever the core says one changed. */
export function AgentBoardBridge() {
    useEffect(() => {
        let stopped = false;
        const keep = (read: Promise<Parameters<typeof cmd.setAgentBoards>[0]>) =>
            read.then((boards) => {
                if (!stopped) cmd.setAgentBoards(boards);
            }, swallow("read the boards"));
        const stop = onCoreChanged("boards", ({ project }) =>
            keep(project ? agentBoardApi.get(project).then((board) => [board]) : agentBoardApi.all()),
        );
        void keep(agentBoardApi.all());
        return () => {
            stopped = true;
            stop();
        };
    }, []);
    return null;
}
