import { describe, expect, it } from "vitest";
import { effectiveAuth } from "./run";
import type { BruAuth, BruRequest, BruScope } from "./types";

const auth = (patch: Partial<BruAuth>): BruAuth => ({
    mode: "inherit",
    bearer: { token: "" },
    basic: { username: "", password: "" },
    apikey: { key: "", value: "", placement: "header" },
    ...patch,
});
const scope = (name: string, given: BruAuth): BruScope => ({
    meta: { name, type: "", seq: null },
    auth: given,
    headers: [],
    vars: { req: [], res: [] },
    scripts: { pre: "", post: "" },
    extra: [],
});

describe("Bruno run helpers", () => {
    it("resolves auth inheritance request -> leaf folder -> root -> collection", () => {
        const request = { auth: auth({}) } as Pick<BruRequest, "auth"> as BruRequest;
        const collection = scope("collection", auth({ mode: "bearer", bearer: { token: "collection" } }));
        const rootFolder = scope("root", auth({ mode: "basic", basic: { username: "root", password: "pw" } }));
        const leafFolder = scope("leaf", auth({}));

        expect(effectiveAuth(request, [collection, rootFolder, leafFolder])).toEqual(rootFolder.auth);

        request.auth = auth({ mode: "bearer", bearer: { token: "request" } });
        expect(effectiveAuth(request, [collection, rootFolder])).toEqual(request.auth);
    });
});
