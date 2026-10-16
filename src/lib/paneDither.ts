export const DITHER_PATTERNS = ["8x8", "4x4", "2x2", "random"] as const;
export const GRAIN_SHAPES = ["simplex", "warp", "dots", "wave", "ripple", "swirl", "sphere"] as const;

export type DitherPattern = (typeof DITHER_PATTERNS)[number];
export type GrainShape = (typeof GRAIN_SHAPES)[number];

/** How panes dither their grain, or the picture shown in its place. */
export interface PaneDither {
    strength: number;
    grainShape: GrainShape;
    grainPattern: DitherPattern;
    grainSize: number;
    grainScale: number;
    grainSpeed: number;
    imagePattern: DitherPattern;
    imageSize: number;
    imageSteps: number;
    imageOriginalColors: boolean;
}

export const DEFAULT_PANE_DITHER: PaneDither = {
    strength: 1,
    grainShape: "simplex",
    grainPattern: "8x8",
    grainSize: 3,
    grainScale: 2.4,
    grainSpeed: 0.5,
    imagePattern: "8x8",
    imageSize: 2,
    imageSteps: 4,
    imageOriginalColors: true,
};

/* The picture is decoded at one texel per two-pixel dot, so smaller dots would only blur it. */
export const PANE_DITHER_RANGES = {
    strength: { min: 0, max: 2, step: 0.05 },
    grainSize: { min: 1, max: 8, step: 1 },
    grainScale: { min: 0.5, max: 6, step: 0.1 },
    grainSpeed: { min: 0, max: 2, step: 0.05 },
    imageSize: { min: 2, max: 8, step: 1 },
    imageSteps: { min: 1, max: 7, step: 1 },
} as const;

type RangedKey = keyof typeof PANE_DITHER_RANGES;

export function clampPaneDither<K extends RangedKey>(key: K, value: number): number {
    const { min, max, step } = PANE_DITHER_RANGES[key];
    const stepped = Math.round(value / step) * step;
    return Math.min(max, Math.max(min, Number(stepped.toFixed(2))));
}

export function normalisePaneDither(value: unknown): PaneDither {
    const saved = value !== null && typeof value === "object" ? (value as Record<string, unknown>) : {};
    const number = (key: RangedKey) =>
        typeof saved[key] === "number" && Number.isFinite(saved[key]) ? clampPaneDither(key, saved[key]) : DEFAULT_PANE_DITHER[key];
    const choice = <T extends string>(key: keyof PaneDither, allowed: readonly T[]) =>
        allowed.includes(saved[key] as T) ? (saved[key] as T) : (DEFAULT_PANE_DITHER[key] as T);
    return {
        strength: number("strength"),
        grainShape: choice("grainShape", GRAIN_SHAPES),
        grainPattern: choice("grainPattern", DITHER_PATTERNS),
        grainSize: number("grainSize"),
        grainScale: number("grainScale"),
        grainSpeed: number("grainSpeed"),
        imagePattern: choice("imagePattern", DITHER_PATTERNS),
        imageSize: number("imageSize"),
        imageSteps: number("imageSteps"),
        imageOriginalColors: typeof saved.imageOriginalColors === "boolean" ? saved.imageOriginalColors : DEFAULT_PANE_DITHER.imageOriginalColors,
    };
}
