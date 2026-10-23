import { describe, expect, it } from "vitest";
import { reusableOptions } from "./options";

describe("reuse", () => {
    it("drops secure options from a past run", () => {
        expect(reusableOptions({ A: "1", SECRET: "s" }, new Set(["SECRET"]))).toEqual({ A: "1" });
    });
});
