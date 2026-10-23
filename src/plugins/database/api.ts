import { createPluginBackend } from "../../plugin-api/backend";
import { DATABASE_PLUGIN } from "./kinds";
import type { DatabaseCopyFormat, DatabaseQueryOutcome, DatabaseResultSet } from "./types.generated";

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

export const databaseApi = {
    profiles: () => backend.call<DatabaseProfile[]>("profiles"),
    /** A password left out keeps the saved one; an empty one forgets it. */
    save: (profile: ProfileDraft, password?: string) => backend.call<DatabaseProfile>("save", { profile, password }),
    remove: (id: string) => backend.call<void>("remove", { id }),
    test: (profile: ProfileDraft, password?: string) => backend.call<Tested>("test", { profile, password }),
    connect: (id: string) => backend.call<void>("connect", { id }),
    disconnect: (id: string) => backend.call<void>("disconnect", { id }),
    /** `run` names this run, so stopping it never stops another console's query on the same connection. */
    query: (id: string, sql: string, limit?: number, run?: string) => backend.call<DatabaseQueryOutcome>("query", { id, sql, limit, run }),
    /** The result as text to paste elsewhere, with a header row. */
    copyResult: (result: DatabaseResultSet, format: DatabaseCopyFormat) =>
        backend.call<string>("copyResult", { result: { columns: result.columns, rows: result.rows }, format }),
    cancel: (id: string, run: string) => backend.call<void>("cancel", { id, run }),
    clearHistory: (id: string) => backend.call<void>("clearHistory", { id }),
};
