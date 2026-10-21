import { useEffect } from "react";
import { IS_MACOS } from "../lib/platform";
import { usePreference } from "../state/preferences";
import { swallow } from "../state/toast";
import { notchApi } from "./notchSettings";

/** Starts the notch helper while the person's Notch preferences have it on, and stops it otherwise. */
export function NotchBridge() {
    const enabled = usePreference("notch")?.enabled;
    useEffect(() => {
        if (!IS_MACOS || enabled === undefined) return;
        notchApi.configure(enabled).catch(swallow("start the notch"));
    }, [enabled]);
    return null;
}
