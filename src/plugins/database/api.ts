import { createPluginBackend } from "../../plugin-api/backend";
import { DATABASE_PLUGIN } from "./kinds";

const backend = createPluginBackend(DATABASE_PLUGIN.id);

/** PostgreSQL's `sslmode` names: whether to encrypt, and whether to check the server's certificate. */
export type TlsMode = "disable" | "prefer" | "require" | "verify-full";

/** A database server reached over the network; an empty port means the engine's usual one. */
export interface ServerFields {
    host: string;
    port: number | null;
    database: string;
    user: string;
    tls: TlsMode;
}

export type PostgresTarget = ServerFields & { engine: "postgres" };
export type MysqlTarget = ServerFields & { engine: "mysql" };

export interface SqliteTarget {
    engine: "sqlite";
    path: string;
}

export type Target = PostgresTarget | MysqlTarget | SqliteTarget;
export type ServerTarget = PostgresTarget | MysqlTarget;
export type Engine = Target["engine"];

export type DatabaseProfile = Target & {
    id: string;
    name: string;
    readOnly: boolean;
    /** Agents may run statements that change data; otherwise their connection is read-only. */
    agentWrites: boolean;
    hasPassword: boolean;
};

/** A profile as the form holds it: no id until it is first saved. */
export type ProfileDraft = Target & {
    id?: string;
    name: string;
    readOnly: boolean;
    agentWrites: boolean;
};

export interface Tested {
    version: string;
    millis: number;
}

export interface ResultColumn {
    name: string;
    /** The engine's own name for the type; empty when it does not say. */
    type: string;
    numeric: boolean;
}

/** A cell: null, a boolean, a number, or text. Huge integers and decimals arrive as text so no digit is lost. */
export type Cell = string | number | boolean | null;

export interface ResultSet {
    columns: ResultColumn[];
    rows: Cell[][];
    truncated: boolean;
    /** Rows a change touched; null for statements that return rows. */
    affected: number | null;
}

export interface QueryOutcome {
    results: ResultSet[];
    millis: number;
}

export const databaseApi = {
    profiles: () => backend.call<DatabaseProfile[]>("profiles"),
    /** A password left out keeps the saved one; an empty one forgets it. */
    save: (profile: ProfileDraft, password?: string) => backend.call<DatabaseProfile>("save", { profile, password }),
    remove: (id: string) => backend.call<void>("remove", { id }),
    test: (profile: ProfileDraft, password?: string) => backend.call<Tested>("test", { profile, password }),
    connect: (id: string) => backend.call<void>("connect", { id }),
    disconnect: (id: string) => backend.call<void>("disconnect", { id }),
    /** `run` names this run, so stopping it never stops another console's query on the same connection. */
    query: (id: string, sql: string, limit?: number, run?: string) => backend.call<QueryOutcome>("query", { id, sql, limit, run }),
    cancel: (id: string, run: string) => backend.call<void>("cancel", { id, run }),
    clearHistory: (id: string) => backend.call<void>("clearHistory", { id }),
};
