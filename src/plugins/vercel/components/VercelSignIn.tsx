import { useState } from "react";
import { openUrl, swallow } from "../../../plugin-api/host";
import { SignInError, SignInScreen, useSignInAttempt } from "../../../plugin-api/ui";
import { vercelApi, type VercelStatus } from "../api";
import { VercelMark } from "./VercelMark";

/** Where a person makes the access token they paste here. */
export const VERCEL_TOKENS_PAGE = "https://vercel.com/account/settings/tokens";

export function VercelSignIn({ status, onSignedIn }: { status: VercelStatus | undefined; onSignedIn: (status: VercelStatus) => void }) {
    const [token, setToken] = useState("");
    const attempt = useSignInAttempt(status);
    const canSubmit = !attempt.busy && !!token.trim();
    const submit = () => {
        if (!canSubmit) return;
        attempt.run(async () => onSignedIn(await vercelApi.signIn(token.trim())));
    };

    return (
        <SignInScreen
            mark={<VercelMark size={26} className="icon-vercel" />}
            title="Connect Vercel"
            lede="See your projects, deployments and build logs beside your code, and let agents read them, redeploy and promote."
            foot="Sikemux keeps your token in the macOS Keychain.">
            <div className="signin-form">
                <label className="signin-field">
                    <span>Access token</span>
                    <input
                        className="signin-input mono"
                        type="password"
                        value={token}
                        onChange={(event) => setToken(event.target.value)}
                        onKeyDown={(event) => {
                            if (event.key === "Enter") submit();
                        }}
                        placeholder="Paste a Vercel token"
                        autoFocus
                        spellCheck={false}
                    />
                    <small className="signin-hint">
                        In Vercel, open Account Settings → Tokens, create a token for the scopes you want, and paste it here.
                    </small>
                </label>
                <button type="button" className="signin-btn primary" disabled={!canSubmit} onClick={submit}>
                    {attempt.busy ? "Checking…" : "Sign in"}
                </button>
            </div>
            <SignInError error={attempt.error} />
            <div className="signin-alt">
                <button type="button" className="signin-link" onClick={() => void openUrl(VERCEL_TOKENS_PAGE).catch(swallow("open Vercel"))}>
                    Make a token
                </button>
            </div>
        </SignInScreen>
    );
}
