import { describe, expect, it } from "vitest";
import { elapsedMs, isUnfinished, watchIsNewer } from "./runStatus";

const NOW = Date.parse("2026-01-01T12:00:00Z");

describe("isUnfinished", () => {
    it("counts queued, running and waiting for approval as unfinished, and nothing that has completed", () => {
        expect(isUnfinished({ status: "in_progress", outcome: "running" })).toBe(true);
        expect(isUnfinished({ status: "queued", outcome: "queued" })).toBe(true);
        expect(isUnfinished({ status: "waiting", outcome: "blocked" })).toBe(true);
        expect(isUnfinished({ status: "completed", outcome: "success" })).toBe(false);
        expect(isUnfinished({ status: "completed", outcome: "blocked" })).toBe(false);
    });
});

describe("elapsedMs", () => {
    it("measures a finished span between its own two ends", () => {
        expect(elapsedMs("2026-01-01T11:58:00Z", "2026-01-01T11:59:30Z", NOW)).toBe(90_000);
    });

    it("measures an unfinished span up to now", () => {
        expect(elapsedMs("2026-01-01T11:59:00Z", null, NOW)).toBe(60_000);
    });

    it("has nothing to measure without a start, or from nonsense", () => {
        expect(elapsedMs(null, null, NOW)).toBeNull();
        expect(elapsedMs("not a time", null, NOW)).toBeNull();
        expect(elapsedMs("2026-01-01T11:59:00Z", "not a time", NOW)).toBeNull();
    });

    it("never reports a negative span from a clock that disagrees", () => {
        expect(elapsedMs("2026-01-01T12:05:00Z", null, NOW)).toBe(0);
    });
});

describe("watchIsNewer", () => {
    const at = (attempt: number, minute: number) => ({ attempt, updatedAt: `2026-09-28T10:${String(minute).padStart(2, "0")}:00Z` });

    it("lets a re-run replace the finished run a watch last saw", () => {
        expect(watchIsNewer(at(1, 30), at(2, 31))).toBe(false);
        expect(watchIsNewer(at(2, 31), at(1, 30))).toBe(true);
    });

    it("lets a refresh of the same attempt win unless the watch saw something later", () => {
        expect(watchIsNewer(at(1, 30), at(1, 30))).toBe(false);
        expect(watchIsNewer(at(1, 32), at(1, 30))).toBe(true);
    });

    it("uses whichever there is", () => {
        expect(watchIsNewer(null, at(1, 30))).toBe(false);
        expect(watchIsNewer(at(1, 30), null)).toBe(true);
    });
});
