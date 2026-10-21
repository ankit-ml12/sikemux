import { describe, expect, it } from "vitest";
import { reviewVerdict } from "./PullsView";

describe("reviewVerdict", () => {
    const review = (author: string, state: string) => ({ author, state });

    it("says nothing until somebody has reviewed", () => {
        expect(reviewVerdict([])).toBeNull();
        expect(reviewVerdict([review("a", "COMMENTED")])).toBeNull();
    });

    it("reports an approval", () => {
        expect(reviewVerdict([review("a", "APPROVED")])).toBe("Approved");
    });

    it("lets requested changes outweigh an approval", () => {
        expect(reviewVerdict([review("a", "APPROVED"), review("b", "CHANGES_REQUESTED")])).toBe("Changes requested");
    });

    it("counts only a person's latest review", () => {
        expect(reviewVerdict([review("a", "CHANGES_REQUESTED"), review("a", "APPROVED")])).toBe("Approved");
    });

    it("ignores a review that was dismissed", () => {
        expect(reviewVerdict([review("a", "APPROVED"), review("b", "DISMISSED")])).toBe("Approved");
    });
});
