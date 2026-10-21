import { invokeCommand as invoke } from "./invoke";
import { getState } from "../state/store";
import { swallow } from "../state/toast";

export function reportActive(): Promise<void> {
    const preferences = getState().preferences;
    if (!preferences?.shareUsageData) return Promise.resolve();
    return invoke<void>("usage_report_active", { channel: preferences.updateChannel }).catch(swallow("usage report"));
}
