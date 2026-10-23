import { invokeCommand as invoke } from "./invoke";

/** A page an agent showed, as `page_show` hands it back. */
export interface PageRef {
    id: string;
    title: string;
    /** The page's height at the reply width, when the agent measured it. */
    height?: number;
}

export const pagesApi = {
    theme: (dark: boolean, variables: Record<string, string>) => invoke<void>("page_theme", { dark, variables }),
    /** Closes the tab the agent checked its draft in, and opens the kept page on its desk when `desk` is set. */
    shown: (agentId: string, id: string, desk: boolean) => invoke<void>("page_shown", { agentId, id, desk }),
};
