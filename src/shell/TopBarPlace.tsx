import { useMemo } from "react";
import { useShallow } from "zustand/react/shallow";
import { chooseHost, THIS_HOST, type HostChoice } from "../hosts/hosts";
import * as cmd from "../state/commands";
import { useStore } from "../state/store";
import { Dropdown, type DropdownOption } from "../ui/Dropdown";
import { IconChevron, IconFolder, IconLaptop, IconServer } from "../ui/Icons";
import { LEAVES_SETTINGS } from "../settings/leaveSettings";

const keyOf = (choice: HostChoice) => (choice.kind === "core" ? `core:${choice.host ?? ""}` : `ssh:${choice.alias}`);

function choiceOf(key: string): HostChoice {
    const [kind, rest] = [key.slice(0, key.indexOf(":")), key.slice(key.indexOf(":") + 1)];
    return kind === "ssh" ? { kind: "ssh", alias: rest } : rest ? { kind: "core", host: rest } : THIS_HOST;
}

/** A host's mark in the side rail's colour for its kind: this host, one on the account, or an SSH host. */
function hostMark(choice: HostChoice) {
    if (choice.kind === "core" && !choice.host) return <IconLaptop size={13} />;
    return (
        <span className={`tb-place-mark ${choice.kind === "core" ? "host-sikemux" : "host-ssh"}`}>
            <IconServer size={13} />
        </span>
    );
}

const PROJECT_MARK = (
    <span className="tb-place-mark project">
        <IconFolder size={13} />
    </span>
);

/** Where the window is: the chosen host, then the project in front, each a picker, in the middle of the top bar. */
export function TopBarPlace() {
    const chosen = useStore((s) => s.chosenHost);
    const hosts = useStore((s) => s.hosts);
    const sshAliases = useStore(
        useShallow((s) => [...new Set(s.sessionOrder.flatMap((id) => (s.sessions[id]?.kind === "ssh" ? [s.sessions[id].name] : [])))]),
    );
    const projects = useStore(
        useShallow((s) =>
            s.sessionOrder.flatMap((id) => {
                const session = s.sessions[id];
                const onHost = session?.kind === "project" && s.chosenHost.kind === "core" && session.host === s.chosenHost.host;
                return onHost ? [`${id}\n${session.name}`] : [];
            }),
        ),
    );
    const activeId = useStore((s) => s.activeSessionId);

    const hostOptions = useMemo<DropdownOption[]>(() => {
        const installed = new Set(hosts.flatMap((host) => host.ssh ?? []));
        return [
            { value: keyOf(THIS_HOST), label: "This host", icon: hostMark(THIS_HOST) },
            ...hosts.map((host) => ({
                value: keyOf({ kind: "core", host: host.id }),
                label: host.name,
                icon: hostMark({ kind: "core", host: host.id }),
            })),
            ...sshAliases
                .filter((alias) => !installed.has(alias))
                .map((alias) => ({ value: keyOf({ kind: "ssh", alias }), label: alias, icon: hostMark({ kind: "ssh", alias }) })),
        ];
    }, [hosts, sshAliases]);
    const projectOptions = useMemo<DropdownOption[]>(
        () =>
            projects.map((entry) => {
                const [id, name] = entry.split("\n");
                return { value: id, label: name, icon: PROJECT_MARK };
            }),
        [projects],
    );
    const hostKey = keyOf(chosen);
    const inProject = projectOptions.some((option) => option.value === activeId);

    return (
        <div className="tb-place" data-no-window-drag {...LEAVES_SETTINGS}>
            <Dropdown
                className="tb-place-btn"
                label="Host"
                value={hostKey}
                options={hostOptions}
                icon={hostMark(chosen)}
                onChange={(key) => chooseHost(choiceOf(key))}
            />
            {inProject && (
                <>
                    <span className="tb-place-sep" aria-hidden="true">
                        <IconChevron size={11} />
                    </span>
                    <Dropdown
                        className="tb-place-btn"
                        label="Project"
                        value={activeId}
                        options={projectOptions}
                        icon={PROJECT_MARK}
                        search={projectOptions.length > 8 ? "Find a project" : undefined}
                        onChange={cmd.selectSession}
                    />
                </>
            )}
        </div>
    );
}
