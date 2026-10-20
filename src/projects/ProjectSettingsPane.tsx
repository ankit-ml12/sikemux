import { useEffect, useState } from "react";
import { fsapi } from "../api/fs";
import { lsp } from "../api/lsp";
import { useProjectRepo } from "../git/projectRepo";
import { basename, prettyPath } from "../lib/paths";
import { SpaceGlyph } from "../rail/SpaceSwitch";
import { SettingsPage, SettingsRow, SettingsRows, SettingsSection } from "../settings/SettingsLayout";
import * as cmd from "../state/commands";
import { confirmDialog } from "../state/dialog";
import { useStore } from "../state/store";
import { notify, reportError } from "../state/toast";
import { AddTaskDialog } from "../tasks/AddTaskDialog";
import { tasksApi, useProjectTasks, type ProjectTask, type TaskSourceKind } from "../tasks/coreTasks";
import { runProjectTask } from "../tasks/projectTaskActions";
import { Switch } from "../ui/Controls";
import { Dropdown } from "../ui/Dropdown";
import { IconFolder, IconGit, IconPencil, IconPlus, IconRun, IconTrash } from "../ui/Icons";
import { Tooltip } from "../ui/Tooltip";
import { loadProjectConfig, type ProjectConfigLoadResult } from "./projectConfig";
import { trustProjectConfig } from "./projectConfigRuntime";
import "../styles/settings.css";
import "../styles/project-settings.css";

const SOURCE_NAMES: Record<TaskSourceKind, string> = {
    project: "sikemux.json",
    personal: "Only you",
    detected: "Build files",
    rerun: "Rerun",
    command: "Command",
};

const editable = (task: ProjectTask) => task.source === "project" || task.source === "personal";

/** One project's own settings, kept in its repository or in Sikemux. */
export function ProjectSettingsPane({ project, visible }: { project: string; visible: boolean }) {
    const home = useStore((s) => s.home);
    const tasks = useProjectTasks(visible ? project : null);
    const [config, setConfig] = useState<ProjectConfigLoadResult | null>(null);
    const [dialog, setDialog] = useState<{ editing?: ProjectTask } | null>(null);

    useEffect(() => {
        if (!visible) return;
        let cancelled = false;
        void loadProjectConfig(project)
            .then((next) => {
                if (!cancelled) setConfig(next);
            })
            .catch(() => {});
        return () => {
            cancelled = true;
        };
    }, [project, visible, tasks]);

    const own = tasks?.tasks.filter(editable) ?? [];
    const found = tasks?.tasks.filter((task) => task.source === "detected") ?? [];

    return (
        <div className="project-settings-pane">
            <SettingsPage>
                <header className="project-settings-head">
                    <span className="project-settings-glyph">
                        <IconFolder size={16} />
                    </span>
                    <span className="project-settings-title">
                        <h1>{basename(project)}</h1>
                        <span className="project-settings-path">{prettyPath(project, home)}</span>
                    </span>
                    <button
                        type="button"
                        className="settings-btn"
                        onClick={() => void fsapi.revealInFinder(project).catch(reportError("reveal the project"))}>
                        Reveal in Finder
                    </button>
                </header>

                <SettingsSection
                    title="Tasks"
                    meta={tasks ? `${own.length} yours · ${found.length} found` : undefined}
                    sub="Commands the run button and agents start. For the team goes in sikemux.json, which you commit; Only me stays in .sikemux/, which Git ignores.">
                    <div className="settings-list">
                        {own.map((task) => (
                            <TaskRow key={task.id} task={task} project={project} onEdit={() => setDialog({ editing: task })} />
                        ))}
                        {tasks && own.length === 0 && <div className="settings-empty">No tasks of your own yet.</div>}
                        <div className="settings-list-add">
                            <button type="button" className="settings-btn" onClick={() => setDialog({})}>
                                <IconPlus size={11} />
                                Add a task
                            </button>
                        </div>
                    </div>
                    {found.length > 0 && (
                        <div className="settings-list">
                            <div className="project-task-group">Found in the build files</div>
                            {found.map((task) => (
                                <TaskRow key={task.id} task={task} project={project} />
                            ))}
                        </div>
                    )}
                </SettingsSection>

                <ConfigSection project={project} config={config} onChanged={setConfig} />

                <GitSection project={project} visible={visible} />

                <AgentsSection project={project} />
            </SettingsPage>
            {dialog && <AddTaskDialog project={project} editing={dialog.editing} onDone={() => setDialog(null)} />}
        </div>
    );
}

function TaskRow({ task, project, onEdit }: { task: ProjectTask; project: string; onEdit?: () => void }) {
    const remove = async () => {
        const ok = await confirmDialog({
            title: `Remove ${task.label}?`,
            body:
                task.source === "project"
                    ? "It comes out of sikemux.json; commit that to remove it for the team."
                    : "It comes out of .sikemux/tasks.json.",
            confirmLabel: "Remove",
            destructive: true,
        });
        if (!ok) return;
        void tasksApi
            .remove(project, task.id)
            .then(() => notify("success", `Removed ${task.label}`))
            .catch(reportError("remove task"));
    };
    return (
        <div className="project-task-row">
            <span className="project-task-copy">
                <span className="project-task-label">{task.label}</span>
                <span className="project-task-command">{task.command}</span>
            </span>
            <span className="project-task-meta">
                {task.cwd !== "." && <span className="project-task-cwd">{task.cwd}</span>}
                <span>{SOURCE_NAMES[task.source]}</span>
            </span>
            <span className="project-task-actions">
                <Tooltip label={`Run ${task.label}`}>
                    <button
                        type="button"
                        className="settings-row-x"
                        aria-label={`Run ${task.label}`}
                        onClick={() => void runProjectTask(project, task)}>
                        <IconRun size={11} />
                    </button>
                </Tooltip>
                {onEdit && (
                    <Tooltip label="Edit">
                        <button type="button" className="settings-row-x" aria-label={`Edit ${task.label}`} onClick={onEdit}>
                            <IconPencil size={11} />
                        </button>
                    </Tooltip>
                )}
                {onEdit && (
                    <Tooltip label="Remove">
                        <button type="button" className="settings-row-x" aria-label={`Remove ${task.label}`} onClick={() => void remove()}>
                            <IconTrash size={11} />
                        </button>
                    </Tooltip>
                )}
            </span>
        </div>
    );
}

function ConfigSection({
    project,
    config,
    onChanged,
}: {
    project: string;
    config: ProjectConfigLoadResult | null;
    onChanged: (next: ProjectConfigLoadResult) => void;
}) {
    if (!config) return null;
    const trust = async () => {
        if (config.status !== "valid") return;
        if (await trustProjectConfig(config).catch(reportError("trust sikemux.json"))) onChanged(await loadProjectConfig(project));
    };
    return (
        <SettingsSection title="sikemux.json" sub="The project's actions, tasks and preview, checked into the repository for everyone who opens it.">
            <SettingsRows>
                <SettingsRow
                    label={
                        config.status === "absent"
                            ? "Not made yet"
                            : config.status === "invalid"
                              ? "Has mistakes"
                              : config.trusted
                                ? "Trusted"
                                : "Not trusted yet"
                    }
                    desc={
                        config.status === "absent"
                            ? "Adding a task for the team makes it."
                            : config.status === "invalid"
                              ? `${config.errors.length} ${config.errors.length === 1 ? "problem" : "problems"}; Sikemux ignores the file until they are fixed.`
                              : config.trusted
                                ? "Sikemux runs its commands without asking until the file changes."
                                : "Sikemux asks before it runs any of its commands."
                    }>
                    {config.status === "valid" && !config.trusted && (
                        <button type="button" className="settings-btn" onClick={() => void trust()}>
                            Trust
                        </button>
                    )}
                    {config.status !== "absent" && (
                        <button type="button" className="settings-btn" onClick={() => cmd.requestOpenFile(config.path)}>
                            Open
                        </button>
                    )}
                </SettingsRow>
            </SettingsRows>
            {config.status === "invalid" && (
                <ul className="project-config-errors">
                    {config.errors.map((error) => (
                        <li key={`${error.path}:${error.code}`}>
                            <code>{error.path}</code> {error.message}
                        </li>
                    ))}
                </ul>
            )}
        </SettingsSection>
    );
}

function GitSection({ project, visible }: { project: string; visible: boolean }) {
    const { repo, repos } = useProjectRepo(project, visible);
    const chosen = useStore((s) => s.defaultGitRepos[project]);
    return (
        <SettingsSection
            title="Default git repository"
            sub="When the project folder holds several repositories, its Git pane, the branch in the top bar, the diff pane and new agent worktrees use this one.">
            {repos === null ? (
                <p className="settings-repo-note">This project folder is a repository itself, so its Git views use it.</p>
            ) : repos.length === 0 ? (
                <p className="settings-repo-note">There is no repository in this folder or the folders directly inside it.</p>
            ) : (
                <div className="settings-repo-choices" role="radiogroup" aria-label="Default git repository">
                    <button
                        type="button"
                        role="radio"
                        aria-checked={!chosen}
                        className={`settings-repo-choice${!chosen ? " on" : ""}`}
                        onClick={() => cmd.setDefaultGitRepo(project, null)}>
                        <span className="settings-repo-name">{repos.length === 1 ? "The only one inside" : "Ask each time"}</span>
                        <span className="settings-repo-branch">{repos.length === 1 ? repos[0].name : "the Git pane lists them"}</span>
                    </button>
                    {repos.map((entry) => (
                        <button
                            key={entry.path}
                            type="button"
                            role="radio"
                            aria-checked={chosen === entry.path}
                            className={`settings-repo-choice${chosen === entry.path ? " on" : ""}`}
                            onClick={() => cmd.setDefaultGitRepo(project, entry.path)}>
                            <IconGit size={12} />
                            <span className="settings-repo-name">{entry.name}</span>
                            <span className="settings-repo-branch">{entry.branch || "no commits"}</span>
                            {entry.changes > 0 && <span className="settings-repo-changes">{entry.changes} changed</span>}
                        </button>
                    ))}
                    {repo && <p className="settings-repo-note">Git views open {basename(repo)}.</p>}
                </div>
            )}
        </SettingsSection>
    );
}

function AgentsSection({ project }: { project: string }) {
    const worktree = useStore((s) => s.agentWorktreeDefaults[project] === true);
    const languageServers = useStore((s) => s.languageServerTrust[project] === true);
    const spaces = useStore((s) => s.spaces);
    const space = useStore((s) => s.projectSpaces[project] ?? "");
    return (
        <SettingsSection title="Agents and editor" sub="Kept in Sikemux, not in the repository.">
            <SettingsRows>
                <SettingsRow
                    label="New chats start in a worktree"
                    desc="An agent gets its own checkout, so its edits stay off your branch until you merge them."
                    asLabel>
                    <Switch checked={worktree} onChange={(on) => cmd.setAgentWorktreeDefault(project, on)} />
                </SettingsRow>
                <SettingsRow
                    label="Language servers"
                    desc="They can run code from the project, such as its build scripts. Only allow them for projects you trust."
                    asLabel>
                    <Switch
                        checked={languageServers}
                        onChange={(on) => {
                            cmd.setLanguageServerTrust(project, on);
                            if (!on) void lsp.stop(project).catch(reportError("stop language servers"));
                        }}
                    />
                </SettingsRow>
                <SettingsRow label="Space" desc="The rail shows the project under this space." wide>
                    <Dropdown
                        className="settings-dd"
                        label="Space"
                        value={space}
                        options={[
                            { value: "", label: "No Space" },
                            ...spaces.map((entry) => ({ value: entry.id, label: entry.name, icon: <SpaceGlyph icon={entry.icon} /> })),
                        ]}
                        onChange={(value) => cmd.setProjectSpace(project, value || null)}
                    />
                </SettingsRow>
            </SettingsRows>
        </SettingsSection>
    );
}
