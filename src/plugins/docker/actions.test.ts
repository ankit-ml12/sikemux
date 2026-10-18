import { beforeEach, describe, expect, it, vi } from "vitest";

const api = vi.hoisted(() => ({ act: vi.fn(), remove: vi.fn() }));
vi.mock("./api", async (importOriginal) => ({ ...(await importOriginal<object>()), dockerApi: api }));
const host = vi.hoisted(() => ({
    confirmDialog: vi.fn(),
    notify: vi.fn(),
    runInTerminal: vi.fn(() => Promise.resolve()),
}));
vi.mock("../../plugin-api/host", async (importOriginal) => ({ ...(await importOriginal<object>()), ...host }));

import { act, openShell, remove, shellArgv } from "./actions";

beforeEach(() => {
    Object.values(api).forEach((mock) => mock.mockReset());
    Object.values(host).forEach((mock) => mock.mockClear());
    host.confirmDialog.mockResolvedValue(true);
});

describe("acting on a container", () => {
    it("says what it did, or what the engine said when it refused", async () => {
        api.act.mockResolvedValueOnce({});
        expect(await act("web", "restart")).toBe(true);
        expect(host.notify).toHaveBeenLastCalledWith("success", "Restarted web");
        api.act.mockRejectedValueOnce({ category: "conflict", message: "docker: container is paused" });
        expect(await act("web", "stop")).toBe(false);
        expect(host.notify).toHaveBeenLastCalledWith("error", "Stop web: docker: container is paused");
        expect(host.confirmDialog).not.toHaveBeenCalled();
    });

    it("asks before killing, and does nothing when the person says no", async () => {
        host.confirmDialog.mockResolvedValueOnce(false);
        expect(await act("web", "kill")).toBe(false);
        expect(api.act).not.toHaveBeenCalled();
        expect(host.confirmDialog).toHaveBeenCalledWith(expect.objectContaining({ title: "Kill web?", destructive: true }));
    });

    it("removes a running container only after asking to stop it too", async () => {
        api.remove.mockResolvedValueOnce(undefined);
        expect(await remove("web", true)).toBe(true);
        expect(host.confirmDialog).toHaveBeenCalledWith(expect.objectContaining({ title: "Stop and remove web?" }));
        expect(api.remove).toHaveBeenCalledWith("web", true);
    });

    it("opens a shell in a terminal, with bash where the image has it", () => {
        openShell("web");
        expect(host.runInTerminal).toHaveBeenCalledWith("Shell in web", shellArgv("web"));
        expect(shellArgv("web").slice(0, 4)).toEqual(["docker", "exec", "-it", "web"]);
    });
});
