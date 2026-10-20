import type { PluginKind } from "../plugins/kinds";

/** Who a plugin is, as its manifest says: the id the core knows it by and the name people see. */
export interface PluginIdentity {
    readonly id: string;
    readonly name: string;
}

export const surfaceKind = (plugin: PluginIdentity, surface: string): PluginKind => `${plugin.id}:${surface}` as PluginKind;
