import { coreCall } from "./core";
import { invokeCommand as invoke } from "./invoke";

/** A host in the person's SSH config, as the core reads it. */
export interface SshHost {
    alias: string;
    hostname: string | null;
    user: string | null;
    port: number | null;
    /** How its terminals reach it, the best of them. */
    status: "idle" | "failed" | "connecting" | "ok";
}

export const sshApi = {
    hosts: (): Promise<SshHost[]> => coreCall<SshHost[]>("sshHosts"),
    configEnsure: (): Promise<string> => invoke<string>("ssh_config_ensure"),
};
