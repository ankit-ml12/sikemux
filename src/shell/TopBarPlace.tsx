import { useMemo } from "react";
import { useShallow } from "zustand/react/shallow";
import { chooseHost, THIS_HOST, type HostChoice } from "../hosts/hosts";
import * as cmd from "../state/commands";
import { useStore } from "../state/store";
import { Dropdown, type DropdownOption } from "../ui/Dropdown";
import { IconFolder, IconLaptop, IconServer } from "../ui/Icons";
import { LEAVES_SETTINGS } from "../settings/leaveSettings";

const keyOf = (choice: HostChoice) => (choice.kind === "core" ? `core:${choice.host ?? ""}` : `ssh:${choice.alias}`);

function choiceOf(key: string): HostChoice {
    const [kind, rest] = [key.slice(0, key.indexOf(":")), key.slice(key.indexOf(":") + 1)];
    return kind === "ssh" ? { kind: "ssh", alias: rest } : rest ? { kind: "core", host: rest } : THIS_HOST;
}

/** Where the window is: the chosen host, then the project in front, each a picker, in the middle of the top bar. */
export function TopBarPlace() {
    const chosen = useStore((s) => s.chosenHost);
    const hosts = useStore((s) => s.hosts);
    const sshAliases = useStore(
        useShallow((s) => [
            ...new Set(s.sessionOrder.flatMap((id) => (s.sessions[id]?.kind === "ssh" ? [s.sessions[id].name] : []))),
        ]),
    );
    const projects = useStore(
        useShallow((s) => s.sessionOrder.flatMap((id) => (s.sessions[id]?.kind === "project" ? [`${id}\n${s.sessions[id].name}`] : []))),
    );
    const activeId = useStore((s) => s.activeSessionId);

    const hostOptions = useMemo<DropdownOption[]>(() => {
        const installed = new Set(hosts.flatMap((host) => host.ssh ?? []));
        return [
            { value: keyOf(THIS_HOST), label: "This host", icon: <IconLaptop size={13} /> },
            ...hosts.map((host) => ({ value: keyOf({ kind: "core", host: host.id }), label: host.name, icon: <IconServer size={13} /> })),
            ...sshAliases
                .filter((alias) => !installed.has(alias))
                .map((alias) => ({ value: keyOf({ kind: "ssh", alias }), label: alias, icon: <IconServer size={13} /> })),
        ];
    }, [hosts, sshAliases]);
    const projectOptions = useMemo<DropdownOption[]>(
        () =>
            projects.map((entry) => {
                const [id, name] = entry.split("\n");
                return { value: id, label: name, icon: <IconFolder size={13} /> };
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
                icon={chosen.kind === "core" && !chosen.host ? <IconLaptop size={13} /> : <IconServer size={13} />}
                onChange={(key) => chooseHost(choiceOf(key))}
            />
            {inProject && (
                <>
                    <span className="tb-place-sep" aria-hidden="true">
                        /
                    </span>
                    <Dropdown
                        className="tb-place-btn"
                        label="Project"
                        value={activeId}
                        options={projectOptions}
                        icon={<IconFolder size={13} />}
                        search={projectOptions.length > 8 ? "Find a project" : undefined}
                        onChange={cmd.selectSession}
                    />
                </>
            )}
        </div>
    );
}
