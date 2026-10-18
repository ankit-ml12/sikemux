import { agentApi, type AgentAccountBook } from "../api/agents";
import { onCoreChanged } from "../api/coreChanged";
import { adoptAgentAccounts } from "../state/persist";
import { swallow } from "../state/toast";

/** How often a core that could not be asked at start is asked again. */
const RETRY_MS = 5_000;

function reread(): void {
    agentApi
        .accounts()
        .then((book) => book && adoptAgentAccounts(book))
        .catch(swallow("read the agent accounts from the core"));
}

/**
 * Keeps the person's agent accounts as the core has them, reading them again whenever any client changes them.
 * `known` is what the core said at start, or `undefined` when it could not be asked, so it is asked again.
 */
export function syncAgentAccounts(known: AgentAccountBook | null | undefined): () => void {
    const stop = onCoreChanged("agentAccounts", reread);
    if (known !== undefined) return stop;
    const retry = window.setInterval(() => {
        agentApi.accounts().then(
            (book) => {
                window.clearInterval(retry);
                if (book) adoptAgentAccounts(book);
            },
            () => {},
        );
    }, RETRY_MS);
    return () => {
        window.clearInterval(retry);
        stop();
    };
}
