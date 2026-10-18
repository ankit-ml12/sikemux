import { confirmDialog, notify, runInTerminal, swallow } from "../../plugin-api/host";
import { dockerApi, failureMessage, refreshDocker, type ContainerAction } from "./api";

const DONE: Record<ContainerAction, string> = {
    start: "Started",
    stop: "Stopped",
    restart: "Restarted",
    pause: "Paused",
    unpause: "Resumed",
    kill: "Killed",
};

const DOING: Record<ContainerAction, string> = {
    start: "Start",
    stop: "Stop",
    restart: "Restart",
    pause: "Pause",
    unpause: "Resume",
    kill: "Kill",
};

export async function act(name: string, action: ContainerAction): Promise<boolean> {
    if (action === "kill") {
        const sure = await confirmDialog({
            title: `Kill ${name}?`,
            body: "It stops at once, without the chance to finish what it is doing.",
            confirmLabel: "Kill",
            destructive: true,
        });
        if (!sure) return false;
    }
    try {
        await dockerApi.act(name, action);
        notify("success", `${DONE[action]} ${name}`);
        return true;
    } catch (error) {
        notify("error", `${DOING[action]} ${name}: ${failureMessage(error)}`);
        return false;
    } finally {
        refreshDocker();
    }
}

/** Asks first, and stops a running container on the way, since the engine won't remove one that runs. */
export async function remove(name: string, running: boolean): Promise<boolean> {
    const sure = await confirmDialog({
        title: running ? `Stop and remove ${name}?` : `Remove ${name}?`,
        body: "Its volumes stay. Its own changes to the files inside it are lost.",
        confirmLabel: "Remove",
        destructive: true,
    });
    if (!sure) return false;
    try {
        await dockerApi.remove(name, running);
        notify("success", `Removed ${name}`);
        return true;
    } catch (error) {
        notify("error", `Remove ${name}: ${failureMessage(error)}`);
        return false;
    } finally {
        refreshDocker();
    }
}

/** A shell inside the container, in a terminal, with bash where the image has it. */
export function shellArgv(name: string): string[] {
    return ["docker", "exec", "-it", name, "sh", "-c", "command -v bash >/dev/null && exec bash || exec sh"];
}

export function openShell(name: string): void {
    void runInTerminal(`Shell in ${name}`, shellArgv(name)).catch(swallow("open a shell in the container"));
}
