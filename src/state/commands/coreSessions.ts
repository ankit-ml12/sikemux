import { mutate } from "../store";
import { collectPanes } from "../layout";

/** Records the core terminal a pane shows, so the next launch can take it back. */
export function setPanePty(paneId: string, ptyId: number): void {
    mutate((d) => {
        for (const window of Object.values(d.windows)) {
            const pane = collectPanes(window.root).find((candidate) => candidate.id === paneId);
            if (!pane) continue;
            if (pane.ptyId !== ptyId) pane.ptyId = ptyId;
            return;
        }
    });
}

/** Shows live the terminal agents whose terminal still runs. */
export function showAgentsLive(ids: readonly string[]): void {
    mutate((d) => {
        for (const id of ids) {
            const agent = d.agents[id];
            if (agent) agent.launchState = "live";
        }
    });
}
