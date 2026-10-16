import { useEffect, useRef, useState, type ReactNode } from "react";
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
import { AgentIcon } from "../ui/Icons";
import { SignInError, SignInScreen, SignInWaiting } from "../plugin-api/SignIn";
import {
    accountName,
    addAccount,
    cancelSignIn,
    closeAccountDialog,
    dismissSignIn,
    signIn,
    submitSignInCode,
    switchAccount,
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

    const mark = (
        <span className={`agent-glyph ${provider}`}>
            <AgentIcon type={provider} size={34} />
        </span>
    );
    const foot = "Each account keeps its sign-in in a folder of its own. Chats and settings are shared between them.";

    return (
        <div className="dlg-scrim" onMouseDown={close}>
            <div
                ref={modalRef}
                tabIndex={-1}
                className="dlg account-dlg"
                role="dialog"
                aria-modal="true"
                aria-label={request.profileId ? `Sign in to ${label}` : `Add a ${label} account`}
                onMouseDown={(event) => event.stopPropagation()}>
                {step === "done" && profile ? (
                    <SignedIn provider={provider} account={profile} mark={mark} />
                ) : (
                    <SignInScreen
                        mark={mark}
                        title={step === "name" ? `Add a ${label} account` : `Sign in to ${profile?.name ?? label}`}
                        lede={`Sign in with ${label}'s own login in your browser.`}
                        foot={foot}>
                        {step === "name" && (
                            <form
                                className="signin-form"
                                onSubmit={(event) => {
                                    event.preventDefault();
                                    void create();
                                }}>
                                <label className="signin-field">
                                    <span>Name</span>
                                    <input
                                        className="signin-input"
                                        autoFocus
                                        placeholder="Work"
                                        maxLength={40}
                                        spellCheck={false}
                                        autoComplete="off"
                                        value={name}
                                        disabled={busy}
                                        onChange={(event) => setName(event.target.value)}
                                    />
                                </label>
                                <button type="submit" className="signin-btn primary" disabled={busy || !name.trim()}>
                                    Continue with {label}
                                </button>
                            </form>
                        )}
                        {step === "waiting" && profile && (
                            <>
                                <SignInWaiting onCancel={close}>Finish signing in in your browser</SignInWaiting>
                                {signing?.phase === "waiting" && signing.url && <BrowserHelp url={signing.url} account={profile} />}
                            </>
                        )}
                        {step === "failed" && profile && (
                            <>
                                <SignInError error={signing?.phase === "failed" ? signing.message : null} />
                                <button type="button" className="signin-btn primary" onClick={() => start(profile)}>
                                    Try again
                                </button>
                            </>
                        )}
                        {step !== "waiting" && (
                            <div className="signin-alt">
                                <button type="button" className="signin-link" onClick={close}>
                                    Cancel
                                </button>
                            </div>
                        )}
                    </SignInScreen>
                )}
            </div>
        </div>
    );
}

/** The page again, or a code, for when the browser did not come up or asks for one. */
function BrowserHelp({ url, account }: { url: string; account: ProviderProfile }) {
    const [code, setCode] = useState<string | null>(null);
    if (code !== null) {
        return (
            <form
                className="signin-form"
                onSubmit={(event) => {
                    event.preventDefault();
                    if (!code.trim()) return;
                    void submitSignInCode(account, code)
                        .then(() => setCode(null))
                        .catch(reportError("Sign-in code"));
                }}>
                <label className="signin-field">
                    <span>Code from the page</span>
                    <input
                        className="signin-input mono"
                        autoFocus
                        spellCheck={false}
                        autoComplete="off"
                        value={code}
                        onChange={(event) => setCode(event.target.value)}
                    />
                </label>
                <button type="submit" className="signin-btn" disabled={!code.trim()}>
                    Send code
                </button>
            </form>
        );
    }
    return (
        <div className="signin-alt">
            <button type="button" className="signin-link" onClick={() => void portsApi.openExternal(url).catch(reportError("Open sign-in page"))}>
                Open the page again
            </button>
            <button type="button" className="signin-link" onClick={() => setCode("")}>
                Paste a code
            </button>
        </div>
    );
}

function SignedIn({ provider, account, mark }: { provider: AccountProvider; account: ProviderProfile; mark: ReactNode }) {
    const status = useResource(agentAccountR, provider, account.configPath);
    const current = useStore((s) => selectedProviderProfile(provider, s.providerProfiles, s.selectedProviderProfileIds));
    const currentStatus = useResource(agentAccountR, provider, current?.configPath);
    const plan = status.data?.plan ? planLabel(status.data.plan) : null;
    const who = [status.data?.email ?? status.data?.organization, plan].filter(Boolean).join(" · ");
    const name = accountName(account, status.data);
    const switchTo = () => {
        void switchAccount(provider, account).catch(reportError("Switch account"));
        closeAccountDialog();
    };
    return (
        <SignInScreen
            mark={mark}
            title={`${name} is signed in`}
            lede={status.data?.signedIn === false ? "It is not signed in yet." : who || "Checking who signed in…"}>
            {current && current.id !== account.id ? (
                <>
                    <button type="button" className="signin-btn primary" autoFocus onClick={switchTo}>
                        Switch to {name}
                    </button>
                    <button type="button" className="signin-btn" onClick={closeAccountDialog}>
                        Keep using {accountName(current, currentStatus.data)}
                    </button>
                </>
            ) : (
                <button type="button" className="signin-btn primary" autoFocus onClick={closeAccountDialog}>
                    Done
                </button>
            )}
        </SignInScreen>
    );
}
