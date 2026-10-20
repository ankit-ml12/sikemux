import { invokeCommand as invoke } from "./invoke";

/** One of the core's methods, which every client reaches by name and the core alone decides who may call. */
export function coreCall<T>(method: string, params: unknown = null): Promise<T> {
    return invoke<T>("core_call", { method, params });
}

/** One of the core's requests: `op` names it and the rest are its fields. */
export type CoreRequest = { readonly op: string } & Readonly<Record<string, unknown>>;

/** Sends `request` to the core as is and answers with the core's reply. */
export function coreRequest<Reply = void>(request: CoreRequest): Promise<Reply> {
    return invoke<Reply>("core_request", { request });
}
