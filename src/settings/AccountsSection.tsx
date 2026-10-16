import { useMemo } from "react";
import { AccountSignIn } from "../agents/AccountSignIn";
import { accountName, accountsOf, openAccountDialog, signOut, useSignIn, type AccountProvider } from "../agents/accounts";
import { selectedProviderProfile } from "../agents/agentProfiles";
import { ACCOUNT_AGENT_TYPES, agentLabel } from "../agents/catalog";
import * as cmd from "../state/commands";
import { useResource } from "../state/resources";
import { agentAccountR } from "../state/resources.defs";
import { useStore } from "../state/store";
import { reportError } from "../state/toast";
import type { ProviderProfile } from "../state/types";
import { Switch } from "../ui/Controls";
import { IconPlus, IconTrash } from "../ui/Icons";
import { Tooltip } from "../ui/Tooltip";
import { planLabel } from "../rail/usageFormat";
import { SettingsRow, SettingsRows, SettingsSection } from "./SettingsLayout";

const PROVIDERS = ACCOUNT_AGENT_TYPES.map((id) => ({ id, label: agentLabel(id) }));

/** Where the person keeps their Claude and Codex accounts: who each is signed in as, and everything done to them. */
export function AccountsSections() {
    return (
        <>
            {PROVIDERS.map((provider) => (
                <ProviderAccounts key={provider.id} provider={provider.id} label={provider.label} />
            ))}
        </>
    );
}

export function accountsSectionTitle(provider: AccountProvider): string {
    return `${agentLabel(provider)} accounts`;
}

function ProviderAccounts({ provider, label }: { provider: AccountProvider; label: string }) {
    const profiles = useStore((s) => s.providerProfiles);
    const selections = useStore((s) => s.selectedProviderProfileIds);
    const autoSwitch = useStore((s) => s.accountAutoSwitch[provider] === true);
    const accounts = useMemo(() => accountsOf(provider, profiles), [provider, profiles]);
    const current = selectedProviderProfile(provider, profiles, selections) ?? accounts[0];

    return (
        <SettingsSection
            title={accountsSectionTitle(provider)}
            meta={accounts.length === 1 ? "1 account" : `${accounts.length} accounts`}
            sub={`Each ${label} account signs in through ${label}'s own login and keeps the sign-in in its own folder. Chats and settings are shared, so a chat can move between them.`}>
            <SettingsRows>
                {accounts.map((account) => (
                    <AccountRow key={account.id} provider={provider} account={account} inUse={account.id === current?.id} />
                ))}
                <SettingsRow
                    icon={<IconPlus size={14} />}
                    label="Another account"
                    desc={`Sign in to a second ${label} account, such as one for work.`}>
                    <button className="settings-btn" type="button" onClick={() => openAccountDialog({ provider })}>
                        Add account
                    </button>
                </SettingsRow>
                {accounts.length > 1 && (
                    <SettingsRow
                        label="Move chats when one runs out"
                        desc={`A chat that hits a usage limit carries on with your next signed-in ${label} account. Off by default: ${label === "Claude" ? "Anthropic's" : "OpenAI's"} terms expect one person's ordinary use per plan.`}
                        asLabel
                        control={
                            <Switch
                                checked={autoSwitch}
                                onChange={(on) => cmd.setAccountAutoSwitch(provider, on)}
                                label={`Move ${label} chats to another account at a usage limit`}
                            />
                        }
                    />
                )}
            </SettingsRows>
        </SettingsSection>
    );
}

function AccountRow({ provider, account, inUse }: { provider: AccountProvider; account: ProviderProfile; inUse: boolean }) {
    const status = useResource(agentAccountR, provider, account.configPath);
    const signing = useSignIn(account.id);
    const signedIn = status.data?.signedIn === true;
    const known = status.data !== undefined;
    const plan = status.data?.plan ? planLabel(status.data.plan) : null;
    const name = accountName(account, status.data);
    const who = !known
        ? status.status === "error"
            ? "Could not ask who is signed in"
            : "Checking…"
        : signedIn
          ? [status.data?.email ?? status.data?.organization, plan].filter(Boolean).join(" · ") || "Signed in"
          : "Signed out";

    return (
        <SettingsRow
            icon={
                <span className="account-row-avatar" style={{ background: account.accent }} aria-hidden="true">
                    {name.trim().charAt(0).toUpperCase() || "?"}
                </span>
            }
            label={
                <span className="account-row-label">
                    {name}
                    {inUse && <span className="account-row-badge">New chats</span>}
                </span>
            }
            desc={
                <span className={known && !signedIn ? "account-row-out" : undefined} title={account.configPath ?? undefined}>
                    {who}
                </span>
            }>
            {signing ? (
                <AccountSignIn profile={account} buttonClass="settings-btn" />
            ) : known && !signedIn ? (
                <button className="settings-btn" type="button" onClick={() => openAccountDialog({ provider, profileId: account.id })}>
                    Sign in
                </button>
            ) : (
                <>
                    {!inUse && (
                        <button className="settings-btn" type="button" onClick={() => cmd.selectProviderProfile(provider, account.id)}>
                            Use for new chats
                        </button>
                    )}
                    {signedIn && (
                        <button className="settings-btn" type="button" onClick={() => void signOut(account).catch(reportError("Sign out"))}>
                            Sign out
                        </button>
                    )}
                </>
            )}
            {!account.id.startsWith("builtin-") && (
                <Tooltip label={`Remove ${name} from Sikemux`}>
                    <button
                        className="settings-btn danger account-row-remove"
                        type="button"
                        aria-label={`Remove ${name}`}
                        onClick={() => cmd.deleteProviderProfile(account.id)}>
                        <IconTrash size={12} />
                    </button>
                </Tooltip>
            )}
        </SettingsRow>
    );
}
