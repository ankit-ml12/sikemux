import { describe, expect, it } from "vitest";
import { wallpaperColours } from "./wallpaper";

function paint(size: number, pick: (x: number, y: number) => [number, number, number]): Uint8ClampedArray {
    const pixels = new Uint8ClampedArray(size * size * 4);
    for (let y = 0; y < size; y++)
        for (let x = 0; x < size; x++) {
            const at = (y * size + x) * 4;
            pixels.set([...pick(x, y), 255], at);
        }
    return pixels;
}

const channels = (colour: string) => [1, 3, 5].map((at) => parseInt(colour.slice(at, at + 2), 16));
const lightness = (colour: string) => channels(colour).reduce((sum, channel) => sum + channel, 0) / 765;

describe("wallpaperColours", () => {
    it("takes a dark ground and its loudest colour from a night wallpaper", () => {
        const { colours, accent } = wallpaperColours(
            paint(64, (x, y) => (x > 40 && y > 40 ? [255, 60, 170] : [8, 10, 20])),
            "Night",
        );
        expect(lightness(colours[0])).toBeLessThan(0.25);
        expect(lightness(colours[1])).toBeGreaterThan(0.8);
        const [r, g, b] = channels(accent);
        expect(r).toBeGreaterThan(g);
        expect(b).toBeGreaterThan(g);
    });

    it("turns a pale wallpaper into a light ground with dark text", () => {
        const { colours } = wallpaperColours(
            paint(64, (x) => (x < 8 ? [40, 120, 200] : [238, 232, 220])),
            "Paper",
        );
        expect(lightness(colours[0])).toBeGreaterThan(0.85);
        expect(lightness(colours[1])).toBeLessThan(0.4);
    });

    it("still builds a full palette from a wallpaper with no colour in it", () => {
        const { colours } = wallpaperColours(
            paint(32, (x) => [x * 4, x * 4, x * 4]),
            "Grey",
        );
        expect(colours).toHaveLength(21);
        expect(colours.every((colour) => /^#[0-9a-f]{6}$/.test(colour))).toBe(true);
    });
});
