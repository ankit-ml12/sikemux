import { createCodeHostClient, type CodeHostClient } from "../codehost/client";
import { hostStatusR } from "../codehost/resources";
import { createPluginBackend, type PluginBackend } from "./backend";

export type * from "../codehost/types";
export {
    registerCodeHost,
    type CodeHost,
    type CodeHostApi,
    type HostAccount,
    type HostAccountEntry,
    type HostCapabilities,
    type ThreadOf,
} from "../codehost/registry";
export type { CodeHostClient } from "../codehost/client";
export { hostCiGlyph } from "../codehost/components/HostCiGlyph";

/** A code host plugin's backend. A call that finds the person signed out has the Git pane's read of the host's sign-in read again too. */
export function createCodeHostBackend(pluginId: string): CodeHostClient & { readonly backend: PluginBackend } {
    const backend = createPluginBackend(pluginId, { alsoStatus: [hostStatusR.kind] });
    return { backend, ...createCodeHostClient(backend) };
}
