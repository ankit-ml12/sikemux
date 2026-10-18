import { invokeCommand as invoke } from "./invoke";

/** One of the core's methods, which every client reaches by name and the core alone decides who may call. */
export function coreCall<T>(method: string, params: unknown = null): Promise<T> {
    return invoke<T>("core_call", { method, params });
}
