export type OptionValues = Record<string, string>;

export function splitMulti(value: string, delimiter: string): string[] {
    return value
        .split(delimiter)
        .map((part) => part.trim())
        .filter(Boolean);
}

/** Options from a past run that may be shown and reused: never the secure ones. */
export function reusableOptions(options: OptionValues | null | undefined, secureNames: Set<string>): OptionValues {
    const out: OptionValues = {};
    for (const [name, value] of Object.entries(options ?? {})) {
        if (!secureNames.has(name)) out[name] = value;
    }
    return out;
}
