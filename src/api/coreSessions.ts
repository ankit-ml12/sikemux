import { invokeCommand as invoke } from "./invoke";
import type { SessionInfo } from "./generated/wire";

export const coreSessionsApi = {
    list: () => invoke<SessionInfo[]>("pty_sessions"),
    kill: (id: number) => invoke<void>("pty_kill", { id }),
    quitAndStopEverything: () => invoke<void>("app_quit_and_stop_everything"),
};
