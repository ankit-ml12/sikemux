import { invokeCommand as invoke } from "../api/invoke";

export const notchApi = {
    configure: (enabled: boolean) => invoke<void>("notch_configure", { enabled }),
};
