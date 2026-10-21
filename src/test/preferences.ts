import type { Preferences } from "../api/generated/wire";
import { PREFERENCES_BEFORE_CORE } from "../state/preferences";

/** Preferences as a core answers them for someone past the introduction, with `changes` laid over. */
export function testPreferences(changes: Partial<Preferences> = {}): Preferences {
    return { ...PREFERENCES_BEFORE_CORE, onboardingComplete: true, ...changes };
}
