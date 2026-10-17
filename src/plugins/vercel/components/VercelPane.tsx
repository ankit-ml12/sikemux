import { useEffect, useMemo, useState } from "react";
import { confirmDialog, copyWithNotice, notify, openUrl, swallow } from "../../../plugin-api/host";
import { formatAgo } from "../../../plugin-api/format";
import { invalidate, useResourceEnabled } from "../../../plugin-api/resources";
import { Dropdown, EmptyState, Seg, SendToAgentMenu, SkeletonRows, StaleNotice } from "../../../plugin-api/ui";
import { failureMessage, refreshVercel, vercelApi, type VercelDeployment, type VercelProject, type VercelStatus } from "../api";
import { vercelDeploymentsR, vercelDomainsR, vercelEnvR, vercelLogR, vercelProjectsR, vercelStatusR } from "../resources";
import { deploymentForAgent, isUnderway, updateVercelView, useVercelView, type VercelTab } from "../state";
import { VercelSignIn } from "./VercelSignIn";
import "../vercel.css";

const UNDERWAY_POLL_MS = 4000;

export function VercelPane({ paneId, active }: { paneId: string; active: boolean }) {
    const status = useResourceEnabled(active, vercelStatusR);
    if (status.data === undefined && status.status !== "error") return <SkeletonRows rows={5} label="Connecting to Vercel" />;
    if (!status.data?.configured || status.data.authFailed) return <VercelSignIn status={status.data} onSignedIn={() => refreshVercel()} />;
    return <VercelAccountView paneId={paneId} active={active} status={status.data} />;
}

function StateDot({ state }: { state: string }) {
    return <span className={`vercel-dot ${state.toLowerCase()}`} title={stateLabel(state)} aria-hidden="true" />;
}

function stateLabel(state: string): string {
    const known: Record<string, string> = {
        READY: "Ready",
        ERROR: "Error",
        BUILDING: "Building",
        QUEUED: "Queued",
        INITIALIZING: "Initializing",
        CANCELED: "Canceled",
    };
    return known[state] ?? state;
}

const visit = (url: string) => void openUrl(url).catch(swallow("open the page"));

function VercelAccountView({ paneId, active, status }: { paneId: string; active: boolean; status: VercelStatus }) {
    const view = useVercelView(paneId);
    const team = status.team ?? "";
    const scope = status.teams.find((each) => each.id === team)?.slug ?? status.account?.username ?? "";
    const projects = useResourceEnabled(active, vercelProjectsR, team);
    const [filter, setFilter] = useState("");
    const shown = useMemo(() => {
        const words = filter.trim().toLowerCase();
        return (projects.data ?? []).filter((project) => !words || project.name.toLowerCase().includes(words));
    }, [projects.data, filter]);
    const project = projects.data?.find((each) => each.name === view.project) ?? null;

    const chooseTeam = (next: string) => {
        if (next === team) return;
        updateVercelView(paneId, { project: null, deployment: null });
        void vercelApi
            .setTeam(next)
            .then(refreshVercel)
            .catch((error: unknown) => notify("error", `Switch team: ${failureMessage(error)}`));
    };

    return (
        <div className="vercel-pane">
            <nav className="vercel-list" aria-label="Vercel projects">
                <div className="vercel-list-head">
                    <Dropdown
                        label="Team"
                        value={team}
                        options={[
                            { value: "", label: status.account?.name || status.account?.username || "Personal", detail: "Personal account" },
                            ...status.teams.map((each) => ({ value: each.id, label: each.name, detail: each.slug })),
                        ]}
                        onChange={chooseTeam}
                    />
                    <button type="button" className="vercel-chip" onClick={() => refreshVercel()}>
                        Refresh
                    </button>
                </div>
                <input
                    className="vercel-input"
                    value={filter}
                    onChange={(event) => setFilter(event.target.value)}
                    onKeyDown={(event) => {
                        if (event.key === "Escape") setFilter("");
                    }}
                    placeholder="Filter projects"
                    aria-label="Filter Vercel projects"
                    spellCheck={false}
                />
                <StaleNotice handle={projects} />
                {projects.status === "error" && !projects.data ? (
                    <p className="vercel-note">{projects.error}</p>
                ) : !projects.data ? (
                    <SkeletonRows rows={6} label="Loading projects" />
                ) : shown.length === 0 ? (
                    <EmptyState message={filter ? "No project matches." : "No projects here yet."} />
                ) : (
                    <div className="vercel-rows">
                        {shown.map((each) => (
                            <button
                                key={each.id}
                                type="button"
                                className={`vercel-row project${each.name === view.project ? " active" : ""}`}
                                onClick={() => updateVercelView(paneId, { project: each.name, deployment: null })}>
                                <StateDot state={each.production?.state ?? "NONE"} />
                                <span className="vercel-row-main">
                                    <span className="vercel-row-title">{each.name}</span>
                                    <span className="vercel-row-sub">
                                        {each.production?.url?.replace(/^https:\/\//, "") ?? each.repo ?? "No production deployment"}
                                    </span>
                                </span>
                                <span className="vercel-row-age">{formatAgo(each.updatedAt)}</span>
                            </button>
                        ))}
                    </div>
                )}
                <button
                    type="button"
                    className="vercel-signout"
                    onClick={() => void vercelApi.signOut().then(refreshVercel).catch(swallow("sign out of Vercel"))}>
                    Sign out of {status.account?.username ?? "Vercel"}
                </button>
            </nav>
            {project ? (
                <ProjectView paneId={paneId} active={active} team={team} scope={scope} project={project} />
            ) : (
                <div className="vercel-detail">
                    <EmptyState message="Pick a project to see its deployments, domains and environment." />
                </div>
            )}
        </div>
    );
}

const TABS: { value: VercelTab; label: string }[] = [
    { value: "deployments", label: "Deployments" },
    { value: "domains", label: "Domains" },
    { value: "env", label: "Environment" },
];

function ProjectView({
    paneId,
    active,
    team,
    scope,
    project,
}: {
    paneId: string;
    active: boolean;
    team: string;
    scope: string;
    project: VercelProject;
}) {
    const view = useVercelView(paneId);
    if (view.deployment) {
        return (
            <DeploymentView
                active={active}
                team={team}
                id={view.deployment}
                production={project.production?.id ?? null}
                onBack={() => updateVercelView(paneId, { deployment: null })}
            />
        );
    }
    return (
        <section className="vercel-detail" aria-label={project.name}>
            <header className="vercel-detail-head">
                <div className="vercel-heading-block">
                    <h2 className="vercel-title">{project.name}</h2>
                    <div className="vercel-meta">
                        {project.framework && <span className="vercel-fact">{project.framework}</span>}
                        {project.repo && <span className="vercel-fact">{project.repo}</span>}
                        {project.productionBranch && <span className="vercel-fact">production: {project.productionBranch}</span>}
                    </div>
                </div>
                <span className="vercel-actions">
                    {project.production?.url && (
                        <button type="button" className="vercel-chip" onClick={() => visit(project.production!.url!)}>
                            Visit
                        </button>
                    )}
                    {scope && (
                        <button type="button" className="vercel-chip" onClick={() => visit(`https://vercel.com/${scope}/${project.name}`)}>
                            Open in Vercel
                        </button>
                    )}
                </span>
            </header>
            <div className="vercel-tabs">
                <Seg label="Project view" options={TABS} value={view.tab} onChange={(tab) => updateVercelView(paneId, { tab })} />
            </div>
            {view.tab === "deployments" && (
                <Deployments active={active} team={team} project={project} onOpen={(id) => updateVercelView(paneId, { deployment: id })} />
            )}
            {view.tab === "domains" && <Domains active={active} team={team} project={project.name} />}
            {view.tab === "env" && <Environment active={active} team={team} project={project.name} />}
        </section>
    );
}

function Deployments({ active, team, project, onOpen }: { active: boolean; team: string; project: VercelProject; onOpen: (id: string) => void }) {
    const deployments = useResourceEnabled(active, vercelDeploymentsR, team, project.name);
    const underway = deployments.data?.some((each) => isUnderway(each.state)) ?? false;
    const refresh = deployments.refresh;
    useEffect(() => {
        if (!active || !underway) return;
        const timer = window.setInterval(() => void refresh(), UNDERWAY_POLL_MS);
        return () => window.clearInterval(timer);
    }, [active, underway, refresh]);

    if (deployments.status === "error" && !deployments.data) return <p className="vercel-note">{deployments.error}</p>;
    if (!deployments.data) return <SkeletonRows rows={6} label="Loading deployments" />;
    if (deployments.data.length === 0) return <EmptyState message="No deployments yet." />;
    return (
        <div className="vercel-body">
            <StaleNotice handle={deployments} />
            <div className="vercel-rows">
                {deployments.data.map((each) => (
                    <button key={each.id} type="button" className="vercel-row deployment" onClick={() => onOpen(each.id)}>
                        <StateDot state={each.state} />
                        <span className="vercel-row-main">
                            <span className="vercel-row-title">{each.message || each.url?.replace(/^https:\/\//, "") || each.id}</span>
                            <span className="vercel-row-sub">{[each.branch, each.commit?.slice(0, 7), each.author].filter(Boolean).join(" · ")}</span>
                        </span>
                        <span className={`vercel-target ${each.target}`}>
                            {each.id === project.production?.id ? "Current" : each.target === "production" ? "Production" : "Preview"}
                        </span>
                        <span className="vercel-row-state">{stateLabel(each.state)}</span>
                        <span className="vercel-row-age">{formatAgo(each.createdAt)}</span>
                    </button>
                ))}
            </div>
        </div>
    );
}

function DeploymentView({
    active,
    team,
    id,
    production,
    onBack,
}: {
    active: boolean;
    team: string;
    id: string;
    production: string | null;
    onBack: () => void;
}) {
    const log = useResourceEnabled(active, vercelLogR, team, id);
    const [busy, setBusy] = useState(false);
    const [menu, setMenu] = useState<{ x: number; y: number } | null>(null);
    const deployment = log.data?.deployment;
    const underway = deployment ? isUnderway(deployment.state) : false;
    const refresh = log.refresh;
    useEffect(() => {
        if (!active || !underway) return;
        const timer = window.setInterval(() => void refresh(), UNDERWAY_POLL_MS);
        return () => window.clearInterval(timer);
    }, [active, underway, refresh]);

    if (log.status === "error" && !deployment) {
        return (
            <section className="vercel-detail">
                <EmptyState message={log.error ?? `Vercel could not open ${id}.`} tone="error" />
            </section>
        );
    }
    if (!deployment) {
        return (
            <section className="vercel-detail">
                <SkeletonRows rows={8} label={`Loading ${id}`} />
            </section>
        );
    }

    const act = async (what: string, confirm: { title: string; body: string; confirmLabel: string }, run: () => Promise<VercelDeployment>) => {
        if (busy || !(await confirmDialog(confirm))) return;
        setBusy(true);
        try {
            await run();
            notify("success", what);
            invalidate((kind) => kind.startsWith("vercel.") && kind !== "vercel.status");
        } catch (error) {
            notify("error", `${confirm.confirmLabel}: ${failureMessage(error)}`);
        } finally {
            setBusy(false);
        }
    };
    const isCurrent = deployment.id === production;
    const name = deployment.url?.replace(/^https:\/\//, "") ?? deployment.id;

    return (
        <section className="vercel-detail" aria-label={`Deployment ${name}`}>
            <header className="vercel-detail-head">
                <button type="button" className="vercel-chip" onClick={onBack}>
                    ← Deployments
                </button>
                <span className="vercel-actions">
                    {deployment.url && (
                        <button type="button" className="vercel-chip" onClick={() => visit(deployment.url!)}>
                            Visit
                        </button>
                    )}
                    {deployment.inspectorUrl && (
                        <button type="button" className="vercel-chip" onClick={() => visit(deployment.inspectorUrl!)}>
                            Inspect
                        </button>
                    )}
                    {underway ? (
                        <button
                            type="button"
                            className="vercel-chip"
                            disabled={busy}
                            onClick={() =>
                                void act(
                                    "Canceled the deployment",
                                    { title: "Cancel this deployment?", body: `${name} stops building.`, confirmLabel: "Cancel deployment" },
                                    () => vercelApi.cancel(team, deployment.id),
                                )
                            }>
                            Cancel
                        </button>
                    ) : (
                        <button
                            type="button"
                            className="vercel-chip"
                            disabled={busy}
                            onClick={() =>
                                void act(
                                    "Redeploying",
                                    {
                                        title: "Redeploy?",
                                        body: `Vercel builds ${name} again as a new ${deployment.target} deployment.`,
                                        confirmLabel: "Redeploy",
                                    },
                                    () => vercelApi.redeploy(team, deployment.id),
                                )
                            }>
                            Redeploy
                        </button>
                    )}
                    {deployment.state === "READY" && !isCurrent && (
                        <button
                            type="button"
                            className="vercel-chip primary"
                            disabled={busy}
                            onClick={() =>
                                void act(
                                    "Promoted to production",
                                    {
                                        title: "Promote to production?",
                                        body: `${name} starts serving the project's production domains, without building again.`,
                                        confirmLabel: "Promote",
                                    },
                                    () => vercelApi.promote(team, deployment.id),
                                )
                            }>
                            Promote to production
                        </button>
                    )}
                    <button type="button" className="vercel-chip" onClick={(event) => setMenu({ x: event.clientX, y: event.clientY })}>
                        Send to agent
                    </button>
                </span>
            </header>
            <div className="vercel-summary">
                <h2 className="vercel-title">
                    <StateDot state={deployment.state} />
                    {deployment.message || name}
                </h2>
                <div className="vercel-meta">
                    <span className={`vercel-target ${deployment.target}`}>
                        {isCurrent ? "Current production" : deployment.target === "production" ? "Production" : "Preview"}
                    </span>
                    <span className="vercel-fact">{stateLabel(deployment.state)}</span>
                    {deployment.branch && <span className="vercel-fact">{deployment.branch}</span>}
                    {deployment.commit && (
                        <button type="button" className="vercel-fact mono" onClick={() => copyWithNotice(deployment.commit!, "the commit")}>
                            {deployment.commit.slice(0, 7)}
                        </button>
                    )}
                    {deployment.author && <span className="vercel-fact">{deployment.author}</span>}
                    <span className="vercel-fact">{formatAgo(deployment.createdAt)}</span>
                </div>
                {deployment.error && <p className="vercel-error">{deployment.error}</p>}
            </div>
            <h3 className="vercel-heading">Build log{log.data && log.data.skipped > 0 ? ` (last ${log.data.lines.length} lines)` : ""}</h3>
            <div className="vercel-log" role="log" aria-label="Build log">
                {log.data && log.data.lines.length === 0 ? (
                    <p className="vercel-note">{underway ? "Waiting for the build to print…" : "This build printed nothing."}</p>
                ) : (
                    log.data?.lines.map((line, index) => (
                        <div key={index} className={`vercel-log-line ${line.kind}`}>
                            {line.text || " "}
                        </div>
                    ))
                )}
            </div>
            {menu && (
                <SendToAgentMenu
                    x={menu.x}
                    y={menu.y}
                    alignRight
                    delivery={() => ({ text: deploymentForAgent(deployment, log.data) })}
                    onClose={() => setMenu(null)}
                />
            )}
        </section>
    );
}

function Domains({ active, team, project }: { active: boolean; team: string; project: string }) {
    const domains = useResourceEnabled(active, vercelDomainsR, team, project);
    if (domains.status === "error" && !domains.data) return <p className="vercel-note">{domains.error}</p>;
    if (!domains.data) return <SkeletonRows rows={3} label="Loading domains" />;
    if (domains.data.length === 0) return <EmptyState message="No domains." />;
    return (
        <div className="vercel-body">
            <div className="vercel-rows">
                {domains.data.map((domain) => (
                    <button key={domain.name} type="button" className="vercel-row domain" onClick={() => visit(`https://${domain.name}`)}>
                        <span className={`vercel-dot ${domain.verified ? "ready" : "error"}`} aria-hidden="true" />
                        <span className="vercel-row-title">{domain.name}</span>
                        <span className="vercel-row-sub">
                            {domain.redirect ? `redirects to ${domain.redirect}` : domain.gitBranch ? `branch ${domain.gitBranch}` : "production"}
                        </span>
                        <span className="vercel-row-state">{domain.verified ? "Verified" : "Not verified"}</span>
                    </button>
                ))}
            </div>
        </div>
    );
}

function Environment({ active, team, project }: { active: boolean; team: string; project: string }) {
    const variables = useResourceEnabled(active, vercelEnvR, team, project);
    if (variables.status === "error" && !variables.data) return <p className="vercel-note">{variables.error}</p>;
    if (!variables.data) return <SkeletonRows rows={5} label="Loading environment variables" />;
    if (variables.data.length === 0) return <EmptyState message="No environment variables." />;
    return (
        <div className="vercel-body">
            <p className="vercel-note">Names only. Values stay in Vercel.</p>
            <div className="vercel-rows">
                {variables.data.map((variable) => (
                    <div key={variable.id} className="vercel-row env">
                        <span className="vercel-row-title mono">{variable.key}</span>
                        <span className="vercel-row-sub">
                            {variable.target.join(", ")}
                            {variable.gitBranch ? ` · ${variable.gitBranch}` : ""}
                        </span>
                        <span className="vercel-row-state">{variable.type}</span>
                    </div>
                ))}
            </div>
        </div>
    );
}
