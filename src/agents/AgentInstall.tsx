import { useEffect, useState } from "react";
import { create } from "zustand";
import { agentApi, type AgentInfo } from "../api/agents";
import { getIpcTransport } from "../api/transport";
import { fetchResource, invalidate } from "../state/resources";
import { agentCatalogR } from "../state/resources.defs";
import { getState } from "../state/store";
import { TerminalPane } from "../terminal/TerminalPane";
import { selectedAgentRuntimeProfiles } from "./agentProfiles";
import { agentStatusLabel } from "./agentStatus";
import { agentLabel, installCommand, type AgentType } from "./catalog";

const useInstalling = create<{ type: AgentType | null; run: number }>(() => ({ type: null, run: 0 }));

/** Opens the sheet that runs the agent's own installer where the person can watch it. */
export function installAgent(type: AgentType): void {
    if (!installCommand(type)) return;
    useInstalling.setState((now) => ({ type, run: now.run + 1 }));
}

function closeInstall(): void {
    useInstalling.setState({ type: null });
}

interface PtyExited {
    readonly id: number;
    readonly code: number | null;
}

type Phase =
    { kind: "running" } | { kind: "checking" } | { kind: "installed"; agent: AgentInfo | undefined } | { kind: "failed"; code: number | null };

/** Forgets what was known about every agent and reads the list again, so a new install is found at once. */
export async function checkAgentsAgain(type: AgentType): Promise<AgentInfo | undefined> {
    await agentApi.refreshStatuses();
    invalidate((kind) => kind === "agents.catalog" || kind === "agents.account");
    const { providerProfiles, selectedProviderProfileIds } = getState();
    const agents = await fetchResource(agentCatalogR, selectedAgentRuntimeProfiles(providerProfiles, selectedProviderProfileIds));
    return agents.find((agent) => agent.type === type && agent.available !== false);
}

export function AgentInstallHost() {
    const { type, run } = useInstalling();
    if (!type) return null;
    return <AgentInstallSheet key={run} type={type} />;
}

function AgentInstallSheet({ type }: { type: AgentType }) {
    const command = installCommand(type)!;
    const label = agentLabel(type);
    const [listening, setListening] = useState(false);
    const [ptyId, setPtyId] = useState<number | null>(null);
    const [exit, setExit] = useState<PtyExited | null>(null);
    const [phase, setPhase] = useState<Phase>({ kind: "running" });
    const [attempt, setAttempt] = useState(0);

    useEffect(() => {
        const controller = new AbortController();
        void getIpcTransport()
            .subscribe<PtyExited>("pty_exited", ({ payload }) => setExit(payload), { signal: controller.signal })
            .then(() => setListening(true))
            .catch(() => setListening(true));
        return () => controller.abort();
    }, []);

    useEffect(() => {
        if (!exit || exit.id !== ptyId || phase.kind !== "running") return;
        if (exit.code !== 0) {
            setPhase({ kind: "failed", code: exit.code });
            return;
        }
        setPhase({ kind: "checking" });
        void checkAgentsAgain(type)
            .then((agent) => setPhase({ kind: "installed", agent }))
            .catch(() => setPhase({ kind: "installed", agent: undefined }));
    }, [exit, ptyId, phase.kind, type]);

    const again = () => {
        setExit(null);
        setPtyId(null);
        setPhase({ kind: "running" });
        setAttempt((count) => count + 1);
    };

    return (
        <div className="experience-backdrop command-popup-backdrop" role="presentation">
            <section className="command-popup agent-install" role="dialog" aria-modal="true" aria-label={`Install ${label}`}>
                <header>
                    <span>Install {label}</span>
                    <code className="agent-install-command">{command}</code>
                </header>
                {listening ? (
                    <TerminalPane
                        key={attempt}
                        directCommand={{ program: "/bin/bash", args: ["-c", command] }}
                        active
                        visible
                        onPtySession={setPtyId}
                    />
                ) : (
                    <div />
                )}
                <footer>
                    <span className="agent-install-state" role="status">
                        <InstallState phase={phase} label={label} />
                    </span>
                    {phase.kind === "failed" && (
                        <button type="button" onClick={again}>
                            Try again
                        </button>
                    )}
                    <button type="button" onClick={closeInstall}>
                        {phase.kind === "running" ? "Cancel" : "Close"}
                    </button>
                </footer>
            </section>
        </div>
    );
}

function InstallState({ phase, label }: { phase: Phase; label: string }) {
    switch (phase.kind) {
        case "running":
            return <>Running {label}'s own installer…</>;
        case "checking":
            return <>Installed. Looking for {label}…</>;
        case "failed":
            return <>The installer stopped{phase.code === null ? "" : ` with exit code ${phase.code}`}. Its output above says why.</>;
        case "installed":
            if (!phase.agent) return <>The installer finished, but Sikemux can't find {label} yet. Open a new terminal or restart Sikemux.</>;
            return (
                <>
                    {label} is installed: {agentStatusLabel(phase.agent.status) ?? "found"}.
                </>
            );
    }
}
