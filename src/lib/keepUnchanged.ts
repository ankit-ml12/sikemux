const isPlain = (value: unknown): value is Record<string, unknown> =>
    typeof value === "object" && value !== null && Object.getPrototypeOf(value) === Object.prototype;

/** `next`, with every array and object in it that equals its place in `previous` kept as the one `previous` holds. */
export function keepUnchanged<T>(previous: T, next: T): T {
    if (previous === next) return previous;
    if (Array.isArray(previous) && Array.isArray(next)) {
        let same = previous.length === next.length;
        const kept = next.map((item: unknown, index) => {
            const one = keepUnchanged(previous[index], item);
            if (one !== previous[index]) same = false;
            return one;
        });
        return (same ? previous : kept) as T;
    }
    if (!isPlain(previous) || !isPlain(next)) return next;
    let same = Object.keys(previous).length === Object.keys(next).length;
    const kept: Record<string, unknown> = {};
    for (const [key, value] of Object.entries(next)) {
        const one = keepUnchanged(previous[key], value);
        if (one !== previous[key] || !(key in previous)) same = false;
        kept[key] = one;
    }
    return (same ? previous : kept) as T;
}
