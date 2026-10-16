import { useEffect, useRef, useState } from "react";
import { portsApi } from "../api/ports";
import { useModalFocus } from "../hooks/useModalFocus";
import { planLabel } from "../rail/usageFormat";
import * as cmd from "../state/commands";
import { useOccludeNativeViews } from "../state/nativeViews";
import { useResource } from "../state/resources";
import { agentAccountR } from "../state/resources.defs";
import { useStore } from "../state/store";
import { reportError } from "../state/toast";
import type { ProviderProfile } from "../state/types";
import { Checkbox } from "../ui/Controls";
import { AgentIcon, IconCheck } from "../ui/Icons";
import { Kbd } from "../ui/Kbd";
import {
    addAccount,
    cancelSignIn,
    closeAccountDialog,
    dismissSignIn,
    signIn,
    submitSignInCode,
    useAccountDialog,
    useSignIn,
    type AccountDialogRequest,
    type AccountProvider,
} from "./accounts";
import { providerAccent, selectedProviderProfile } from "./agentProfiles";
import { agentLabel } from "./catalog";

export function AccountDialogHost() {
    const request = useAccountDialog();
    useOccludeNativeViews(request !== null);
    if (!request) return null;
    return <AccountDialog key={request.id} request={request} />;
}

/**
 * Adds an account, or signs one in again, through the provider's own login in
 * the browser: a name, then the browser, then who it signed in as.
 */
function AccountDialog({ request }: { request: AccountDialogRequest }) {
    const { provider } = request;
    const label = agentLabel(provider);
    const modalRef = useRef<HTMLDivElement>(null);
    useModalFocus(modalRef);
    const [profileId, setProfileId] = useState(request.profileId ?? null);
    const profile = useStore((s) => s.providerProfiles.find((candidate) => candidate.id === profileId));
    const signing = useSignIn(profileId ?? "");
    const [name, setName] = useState("");
    const [busy, setBusy] = useState(false);
    const [started, setStarted] = useState(false);

    const start = (account: ProviderProfile) => {
        dismissSignIn(account.id);
        setStarted(true);
        void signIn(account);
    };

    useEffect(() => {
        if (profile && request.profileId) start(profile);
        // Only the account the dialog was opened for signs in on its own.
    }, []); // eslint-disable-line react-hooks/exhaustive-deps

    const step = !profile ? "name" : signing?.phase === "failed" ? "failed" : signing || !started ? "waiting" : "done";

    const close = () => {
        if (profile && signing?.phase === "waiting") cancelSignIn(profile);
        if (profile && !request.profileId && step !== "done") cmd.deleteProviderProfile(profile.id);
        closeAccountDialog();
    };

    useEffect(() => {
        const onKey = (event: KeyboardEvent) => {
            if (event.key !== "Escape") return;
            event.preventDefault();
            event.stopPropagation();
            close();
        };
        window.addEventListener("keydown", onKey, true);
        return () => window.removeEventListener("keydown", onKey, true);
    });

    const create = async () => {
        const trimmed = name.trim();
        if (!trimmed || busy) return;
        setBusy(true);
        try {
            const account = await addAccount(provider, trimmed, providerAccent(provider));
            setProfileId(account.id);
            start(account);
        } catch (error) {
            reportError("Add account")(error);
        } finally {
            setBusy(false);
        }
    };

    return (
        <div className="dlg-scrim" onMouseDown={close}>
            <div
                ref={modalRef}
                tabIndex={-1}
                className={`dlg account-dlg ${provider}`}
                role="dialog"
                aria-modal="true"
                aria-label={request.profileId ? `Sign in to ${label}` : `Add a ${label} account`}
                onMouseDown={(event) => event.stopPropagation()}>
                <div className="account-dlg-hero" data-step={step}>
                    <span className={`account-dlg-mark agent-glyph ${provider}`} aria-hidden="true">
                        <AgentIcon type={provider} size={26} />
                        {step === "done" && (
                            <span className="account-dlg-done">
                                <IconCheck size={10} />
                            </span>
                        )}
                    </span>
                    {step === "name" && (
                        <>
                            <h2 className="dlg-title">Add a {label} account</h2>
                            <p>Sign in with {label}'s own login in your browser. Chats and settings are shared between your accounts.</p>
                        </>
                    )}
                    {step === "waiting" && (
                        <>
                            <h2 className="dlg-title">Finish in your browser</h2>
                            <p>{label}'s login is open in your browser. This updates on its own once you are signed in.</p>
                        </>
                    )}
                    {step === "failed" && signing?.phase === "failed" && (
                        <>
                            <h2 className="dlg-title">Sign-in did not finish</h2>
                            <p>{signing.message}</p>
                        </>
                    )}
                    {step === "done" && profile && <SignedIn provider={provider} account={profile} />}
                </div>

                {step === "name" && (
                    <form
                        id="account-dlg-name"
                        className="account-dlg-field"
                        onSubmit={(event) => {
                            event.preventDefault();
                            void create();
                        }}>
                        <label htmlFor="account-dlg-input">Name</label>
                        <input
                            id="account-dlg-input"
                            className="dlg-input"
                            autoFocus
                            placeholder="Work"
                            maxLength={40}
                            spellCheck={false}
                            autoComplete="off"
                            value={name}
                            disabled={busy}
                            onChange={(event) => setName(event.target.value)}
                        />
                    </form>
                )}
                {step === "waiting" && signing?.phase === "waiting" && profile && <BrowserHelp url={signing.url} account={profile} />}

                <div className="dlg-foot">
                    <span className="dlg-hint">
                        <Kbd>esc</Kbd> {step === "done" ? "close" : "cancel"}
                    </span>
                    {step === "name" && (
                        <>
                            <button type="button" className="dlg-btn" onClick={close}>
                                Cancel
                            </button>
                            <button type="submit" form="account-dlg-name" className="dlg-btn primary" disabled={busy || !name.trim()}>
                                Continue in browser
                            </button>
                        </>
                    )}
                    {step === "waiting" && (
                        <button type="button" className="dlg-btn" onClick={close}>
                            Cancel
                        </button>
                    )}
                    {step === "failed" && profile && (
                        <>
                            <button type="button" className="dlg-btn" onClick={close}>
                                Cancel
                            </button>
                            <button type="button" className="dlg-btn primary" onClick={() => start(profile)}>
                                Try again
                            </button>
                        </>
                    )}
                    {step === "done" && (
                        <button type="button" className="dlg-btn primary" autoFocus onClick={closeAccountDialog}>
                            Done
                        </button>
                    )}
                </div>
            </div>
        </div>
    );
}

/** The page again, or a code, for when the browser did not come up or asks for one. */
function BrowserHelp({ url, account }: { url: string | null; account: ProviderProfile }) {
    const [code, setCode] = useState<string | null>(null);
    if (!url) return <div className="account-dlg-wait" aria-live="polite" />;
    return (
        <div className="account-dlg-help">
            {code === null ? (
                <>
                    <button type="button" className="dlg-btn" onClick={() => void portsApi.openExternal(url).catch(reportError("Open sign-in page"))}>
                        Open the page again
                    </button>
                    <button type="button" className="dlg-btn" onClick={() => setCode("")}>
                        Paste a code
                    </button>
                </>
            ) : (
                <form
                    className="account-dlg-code"
                    onSubmit={(event) => {
                        event.preventDefault();
                        if (!code.trim()) return;
                        void submitSignInCode(account, code)
                            .then(() => setCode(null))
                            .catch(reportError("Sign-in code"));
                    }}>
                    <input
                        className="dlg-input"
                        autoFocus
                        aria-label="Sign-in code"
                        placeholder="Code from the page"
                        spellCheck={false}
                        autoComplete="off"
                        value={code}
                        onChange={(event) => setCode(event.target.value)}
                    />
                    <button type="submit" className="dlg-btn primary" disabled={!code.trim()}>
                        Send
                    </button>
                </form>
            )}
        </div>
    );
}

function SignedIn({ provider, account }: { provider: AccountProvider; account: ProviderProfile }) {
    const status = useResource(agentAccountR, provider, account.configPath);
    const inUse = useStore((s) => selectedProviderProfile(provider, s.providerProfiles, s.selectedProviderProfileIds)?.id === account.id);
    const plan = status.data?.plan ? planLabel(status.data.plan) : null;
    const who = [status.data?.email ?? status.data?.organization, plan].filter(Boolean).join(" · ");
    return (
        <>
            <h2 className="dlg-title">{account.name} is signed in</h2>
            <p>{status.data?.signedIn === false ? "It is not signed in yet." : who || "Checking who signed in…"}</p>
            <div className="account-dlg-use">
                <Checkbox checked={inUse} disabled={inUse} onChange={(on) => on && cmd.selectProviderProfile(provider, account.id)}>
                    Start new chats on this account
                </Checkbox>
            </div>
        </>
    );
}
