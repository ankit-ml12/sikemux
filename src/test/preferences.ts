import type { Preferences } from "../api/generated/wire";

/** Preferences as a fresh core answers them, with `changes` laid over. */
export function testPreferences(changes: Partial<Preferences> = {}): Preferences {
    return {
        midTurnSend: "steer",
        uiTextScale: 1,
        terminalFontSize: 13,
        chatTextScale: 1,
        editorTextScale: 1,
        themeId: "aura-noir",
        customThemes: [],
        lastAgentType: null,
        agentWorktreeDefaults: {},
        restoreAgentTabs: true,
        agentNotifications: true,
        projectRoots: [],
        defaultGitRepos: {},
        languageServerTrust: {},
        customCommands: [],
        pluginSettings: {},
        cloudBrowser: "",
        cloudBrowserShortcut: "",
        iosSimulator: true,
        notch: {
            enabled: true,
            displays: "all",
            openWith: "hover",
            fullScreen: "needsYou",
            peeks: "all",
            answerInNotch: true,
            sound: true,
            haptics: true,
            yieldToDev: true,
        },
        voiceDictation: false,
        ...changes,
    };
}
