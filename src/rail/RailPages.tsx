import type { ReactNode } from "react";
import * as cmd from "../state/commands";
import { useStore } from "../state/store";
import type { AgentRailPage } from "../state/types";
import { IconAgents, IconFile, IconTerminal } from "../ui/Icons";
import { SwipePages } from "../ui/SwipePages";

/** The agent rail's pages: the project's agents, its terminals and its files. */
export function RailPages({ pages }: { pages: Record<AgentRailPage, ReactNode> }) {
    const page = useStore((s) => s.agentRailPage);
    return (
        <SwipePages
            label="Rail pages"
            current={page}
            onChange={(id) => cmd.setAgentRailPage(id as AgentRailPage)}
            pages={[
                { id: "agents", label: "Agents", icon: <IconAgents size={15} />, content: pages.agents },
                { id: "terminals", label: "Terminals", icon: <IconTerminal size={15} />, content: pages.terminals },
                { id: "files", label: "Files", icon: <IconFile size={15} />, content: pages.files },
            ]}
        />
    );
}
