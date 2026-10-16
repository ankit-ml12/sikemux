import { useState } from "react";
import { openUrl, swallow } from "../../../plugin-api/host";
import { SignInError, SignInScreen, useSignInAttempt } from "../../../plugin-api/ui";
import { linearApi, type LinearStatus } from "../api";
import { LinearMark } from "./LinearMark";

/** Where a person makes the personal API key they paste here. */
export const LINEAR_KEYS_PAGE = "https://linear.app/settings/account/security";

/** A personal API key; it reads and writes Linear as the person who made it. */
export function LinearSignIn({ status, onSignedIn }: { status: LinearStatus | undefined; onSignedIn: (status: LinearStatus) => void }) {
    const [key, setKey] = useState("");
    const attempt = useSignInAttempt(status);
    const busy = attempt.busy;

    const canSubmit = !busy && !!key.trim();
    const submit = () => {
        if (!canSubmit) return;
        attempt.run(async () => onSignedIn(await linearApi.signIn(key.trim())));
    };

    return (
        <SignInScreen
            mark={<LinearMark size={26} />}
            title="Connect Linear"
            lede="See the issues you are meant to be working on beside your code, and let agents read, comment on and move them."
            foot="Sikemux keeps the key in the macOS Keychain.">
            <div className="signin-form">
                <label className="signin-field">
                    <span>Personal API key</span>
                    <input
                        className="signin-input mono"
                        type="password"
                        value={key}
                        onChange={(event) => setKey(event.target.value)}
                        onKeyDown={(event) => {
                            if (event.key === "Enter") submit();
                        }}
                        placeholder="lin_api_…"
                        autoFocus
                        spellCheck={false}
                    />
                    <small className="signin-hint">
                        In Linear, open Settings → Security &amp; access, make a personal API key, and paste it here.
                    </small>
                </label>
                <button type="button" className="signin-btn primary" disabled={!canSubmit} onClick={submit}>
                    {busy ? "Checking…" : "Sign in"}
                </button>
            </div>

            <SignInError error={attempt.error} />

            <div className="signin-alt">
                <button type="button" className="signin-link" onClick={() => void openUrl(LINEAR_KEYS_PAGE).catch(swallow("open Linear"))}>
                    Make an API key
                </button>
            </div>
        </SignInScreen>
    );
}
