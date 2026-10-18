import { invokeCommand as invoke } from "./invoke";

export const portsApi = {
    openExternal: (url: string) => invoke<void>("open_url", { url, app: null, shortcut: null }),
};
