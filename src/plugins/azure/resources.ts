import { resource } from "../../plugin-api/resources";
import {
    azureApi,
    type ActivityEvent,
    type App,
    type AppDetail,
    type AzureStatus,
    type Group,
    type Inventory,
    type Machine,
    type Scope,
    type StorageAccount,
    type Vault,
} from "./api";

/** One key per subscription and group, so switching back shows what was read there. */
const keyOf = ([scope]: unknown[]) => `${(scope as Scope).subscription ?? ""}|${(scope as Scope).group ?? ""}`;

export const azureStatusR = resource({
    kind: "azure.status",
    fetch: (): Promise<AzureStatus> => azureApi.status(),
    staleAfterMs: 60_000,
});

export const azureGroupsR = resource({
    kind: "azure.groups",
    fetch: (subscription: string): Promise<Group[]> => azureApi.groups(subscription),
    staleAfterMs: 120_000,
});

export const azureResourcesR = resource({
    kind: "azure.resources",
    fetch: (scope: Scope, find: string): Promise<Inventory> => azureApi.resources(scope, find),
    staleAfterMs: 30_000,
    keyFn: (args) => `${keyOf(args)}|${args[1] as string}`,
});

export const azureVmsR = resource({
    kind: "azure.vms",
    fetch: (scope: Scope): Promise<Machine[]> => azureApi.vms(scope),
    staleAfterMs: 15_000,
    keyFn: keyOf,
});

export const azureVmR = resource({
    kind: "azure.vm",
    fetch: (scope: Scope, name: string): Promise<Machine> => azureApi.vm(scope, name),
    staleAfterMs: 15_000,
    keyFn: (args) => `${keyOf(args)}|${args[1] as string}`,
});

export const azureAppsR = resource({
    kind: "azure.apps",
    fetch: (scope: Scope): Promise<App[]> => azureApi.apps(scope),
    staleAfterMs: 30_000,
    keyFn: keyOf,
});

export const azureAppR = resource({
    kind: "azure.app",
    fetch: (scope: Scope, name: string): Promise<AppDetail> => azureApi.app(scope, name),
    staleAfterMs: 30_000,
    keyFn: (args) => `${keyOf(args)}|${args[1] as string}`,
});

export const azureStorageR = resource({
    kind: "azure.storage",
    fetch: (scope: Scope): Promise<StorageAccount[]> => azureApi.storage(scope),
    staleAfterMs: 60_000,
    keyFn: keyOf,
});

export const azureVaultsR = resource({
    kind: "azure.vaults",
    fetch: (scope: Scope): Promise<Vault[]> => azureApi.vaults(scope),
    staleAfterMs: 60_000,
    keyFn: keyOf,
});

export const azureActivityR = resource({
    kind: "azure.activity",
    fetch: (scope: Scope, hours: number, failedOnly: boolean): Promise<ActivityEvent[]> => azureApi.activity(scope, hours, failedOnly),
    staleAfterMs: 30_000,
    keyFn: (args) => `${keyOf(args)}|${args[1] as number}|${args[2] as boolean}`,
});
