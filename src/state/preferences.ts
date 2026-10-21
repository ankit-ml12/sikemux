import { coreCall } from "../api/core";
import { onCoreChanged } from "../api/coreChanged";
import type { Preferences } from "../api/generated/wire";
import { setState, useStore } from "./store";
import { swallow } from "./toast";

/** Keeps the person's preferences as the core keeps them, for as long as the page runs. */
export function watchPreferences(): () => void {
    let live = true;
    const read = () =>
        coreCall<Preferences>("preferences")
            .then((preferences) => {
                if (live) setState({ preferences });
            })
            .catch(swallow("read your preferences"));
    const stop = onCoreChanged("preferences", () => void read());
    void read();
    return () => {
        live = false;
        stop();
    };
}

/** A preference's value, or undefined until the core has said. */
export function usePreference<K extends keyof Preferences>(key: K): Preferences[K] | undefined {
    return useStore((s) => s.preferences?.[key]);
}

export function setPreference<K extends keyof Preferences>(key: K, value: Preferences[K]): void {
    coreCall<Preferences>("setPreference", { key, value })
        .then((preferences) => setState({ preferences }))
        .catch(swallow("save your preference"));
}
