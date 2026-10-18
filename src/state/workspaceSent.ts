/** Settles once what this window changed so far has reached the core. */
let sent: Promise<void> = Promise.resolve();
let settle: (() => void) | null = null;

/** Waits until every change made here so far is in the core, such as a new agent before its chat starts. */
export function workspaceSent(): Promise<void> {
    return sent;
}

/** A change is on its way to the core; `workspaceSent` waits for it. */
export function changeOnItsWay(): void {
    if (!settle) sent = new Promise((resolve) => (settle = resolve));
}

/** Everything changed so far has reached the core. */
export function allSent(): void {
    settle?.();
    settle = null;
}
