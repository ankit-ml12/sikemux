import { browserSignIn, createPluginBackend, type PluginStream, type PluginStreamHandlers } from "../../plugin-api/backend";
import { VERCEL_PLUGIN } from "./kinds";

const backend = createPluginBackend(VERCEL_PLUGIN.id);

export interface VercelAccount {
    id: string;
    username: string;
    name: string;
    email: string;
    /** A browser sign-in sees only the teams the Sikemux integration is installed on. */
    signIn: "token" | "browser";
}

export interface VercelTeam {
    id: string;
    slug: string;
    name: string;
}

export interface VercelStatus {
    configured: boolean;
    ok: boolean;
    authFailed: boolean;
    message: string | null;
    account: VercelAccount | null;
    teams: VercelTeam[];
    /** The team calls are for, by id; null for the account's own projects. */
    team: string | null;
    /** Whether the account has projects outside its teams; newer accounts keep them in a default team. */
    personal: boolean;
    /** Whether this build can sign in through the browser. */
    browserSignIn: boolean;
}

/** BUILDING, ERROR, INITIALIZING, QUEUED, READY or CANCELED. */
export type DeploymentState = string;

export interface VercelProject {
    id: string;
    name: string;
    framework: string | null;
    updatedAt: number;
    repo: string | null;
    gitProvider: string | null;
    productionBranch: string | null;
    /** The deployment serving production. */
    production: { id: string; url: string | null; state: DeploymentState; createdAt: number } | null;
    /** A newer production build that is still building or failed, while the one before it serves. */
    latestProduction: { id: string; state: DeploymentState; createdAt: number } | null;
}

export interface VercelDeployment {
    id: string;
    project: string;
    projectId: string | null;
    url: string | null;
    state: DeploymentState;
    target: "production" | "preview";
    createdAt: number;
    readyAt: number | null;
    /** How long the build ran, once it finished. */
    buildMs: number | null;
    branch: string | null;
    commit: string | null;
    message: string | null;
    author: string | null;
    /** The deployment this one built again, for a redeploy. */
    redeployOf: string | null;
    inspectorUrl: string | null;
    aliases: string[] | null;
    error: string | null;
}

export interface VercelLogLine {
    at: number;
    /** stdout, stderr, command or fatal. */
    kind: string;
    text: string;
}

export interface VercelLog {
    deployment: VercelDeployment;
    lines: VercelLogLine[];
    skipped: number;
}

export interface VercelDomain {
    name: string;
    verified: boolean;
    redirect: string | null;
    gitBranch: string | null;
}

export interface VercelVariable {
    id: string;
    key: string;
    target: string[];
    type: string;
    gitBranch: string | null;
    updatedAt: number | null;
}

export interface VercelSettings {
    id: string;
    name: string;
    framework: string | null;
    buildCommand: string | null;
    outputDirectory: string | null;
    installCommand: string | null;
    devCommand: string | null;
    rootDirectory: string | null;
    nodeVersion: string | null;
    /** The Node versions Vercel builds with. */
    nodeVersions: string[];
}

export type SettingsChange = Partial<
    Pick<VercelSettings, "framework" | "buildCommand" | "outputDirectory" | "installCommand" | "devCommand" | "rootDirectory" | "nodeVersion">
>;

export interface DnsRecord {
    type: string;
    name: string;
    value: string;
    /** What the record is for, in words. */
    why: string;
}

export interface DomainDetail {
    name: string;
    verified: boolean;
    misconfigured: boolean;
    redirect: string | null;
    redirectStatusCode: number | null;
    gitBranch: string | null;
    /** What is left to add at the domain's DNS provider; none once it is set up. */
    records: DnsRecord[];
}

export interface DomainRouting {
    /** Serve this branch instead of production; "" for production again. */
    branch?: string;
    /** Redirect visitors to this domain instead; "" to stop redirecting. */
    redirect?: string;
}

export interface RuntimeLogLine {
    at: number;
    level: "info" | "warning" | "error" | string;
    source: string;
    message: string;
    method: string | null;
    path: string | null;
    status: number | null;
    domain: string | null;
}

export type VercelTarget = "production" | "preview" | "development";

export interface VariableChange {
    /** The variable being changed; none adds a new one. */
    id?: string;
    key: string;
    /** Left out when changing a variable, its value stays as it is. */
    value?: string;
    targets: VercelTarget[];
    branch?: string;
    kind?: "encrypted" | "sensitive";
}

/** Every call names its team, so the pane never acts on another team than the one it shows. */
export const vercelApi = {
    status: () => backend.call<VercelStatus>("status"),
    signIn: (token: string) => backend.call<VercelStatus>("signIn", { token }),
    signInWithBrowser: (openPage: (url: string) => void) => browserSignIn<VercelStatus>(backend, "signInWithBrowser", openPage),
    signOut: () => backend.call<void>("signOut"),
    setTeam: (team: string) => backend.call<void>("setTeam", { team }),
    projects: (team: string) => backend.call<VercelProject[]>("projects", { team }),
    deployments: (team: string, project: string) => backend.call<VercelDeployment[]>("deployments", { team, project, limit: 50 }),
    deployment: (team: string, id: string) => backend.call<VercelDeployment>("deployment", { team, id }),
    logs: (team: string, id: string) => backend.call<VercelLog>("logs", { team, id, tail: 2000 }),
    domains: (team: string, project: string) => backend.call<VercelDomain[]>("domains", { team, project }),
    env: (team: string, project: string) => backend.call<VercelVariable[]>("env", { team, project }),
    redeploy: (team: string, id: string, target?: "production") => backend.call<VercelDeployment>("redeploy", { team, id, target }),
    promote: (team: string, id: string) => backend.call<VercelDeployment>("promote", { team, id }),
    cancel: (team: string, id: string) => backend.call<VercelDeployment>("cancel", { team, id }),
    domain: (team: string, project: string, domain: string) => backend.call<DomainDetail>("domain", { team, project, domain }),
    addDomain: (team: string, project: string, domain: string, routing: DomainRouting) =>
        backend.call<DomainDetail>("addDomain", { team, project, domain, ...routing }),
    updateDomain: (team: string, project: string, domain: string, routing: DomainRouting) =>
        backend.call<DomainDetail>("updateDomain", { team, project, domain, ...routing }),
    verifyDomain: (team: string, project: string, domain: string) => backend.call<DomainDetail>("verifyDomain", { team, project, domain }),
    removeDomain: (team: string, project: string, domain: string) => backend.call<void>("removeDomain", { team, project, domain }),
    followRuntimeLogs: (team: string, project: string, id: string, handlers: PluginStreamHandlers<{ lines: RuntimeLogLine[] }>): PluginStream =>
        backend.stream("followRuntimeLogs", { team, project, id }, handlers),
    setEnv: (team: string, project: string, change: VariableChange) => backend.call<VercelVariable>("setEnv", { team, project, ...change }),
    removeEnv: (team: string, project: string, id: string) => backend.call<void>("removeEnv", { team, project, id }),
    settings: (team: string, project: string) => backend.call<VercelSettings>("settings", { team, project }),
    updateSettings: (team: string, project: string, settings: SettingsChange) =>
        backend.call<VercelSettings>("updateSettings", { team, project, settings }),
};
