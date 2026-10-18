import { describe, expect, it } from "vitest";
import type { ProviderProfile } from "../state/types";
import { chatLaunchers } from "./workspace";

function profile(id: string, name: string, provider: ProviderProfile["provider"]): ProviderProfile {
    return { id, name, provider, accent: "#fff", configPath: `~/.${id}`, environmentKeys: ["ANTHROPIC_API_KEY"] };
}

describe("chatLaunchers", () => {
    it("names each chat agent once, or once per profile", () => {
        const profiles = [profile("work", "Work", "claude"), profile("home", "Home", "claude"), profile("cx", "Default", "codex")];
        const byId = Object.fromEntries(chatLaunchers(profiles).map((launcher) => [launcher.id, launcher.label]));
        expect(Object.keys(byId).sort()).toEqual(["claude:home", "claude:work", "codex:cx", "grok", "hermes", "muse", "omp", "opencode"]);
        expect(byId["claude:work"]).toBe("Claude · Work");
        expect(byId["codex:cx"]).toBe("Codex");
    });
});
