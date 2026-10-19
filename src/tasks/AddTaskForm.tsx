import { useState } from "react";
import { notify, reportError } from "../state/toast";
import { Segmented } from "../ui/Controls";
import { tasksApi } from "./coreTasks";

type Scope = "team" | "personal";

/** A person's own start command, saved for the team in sikemux.json or for themselves only. */
export function AddTaskForm({ project, onDone }: { project: string; onDone: (taskId: string | null) => void }) {
    const [label, setLabel] = useState("");
    const [command, setCommand] = useState("");
    const [cwd, setCwd] = useState("");
    const [scope, setScope] = useState<Scope>("team");
    const [saving, setSaving] = useState(false);
    const ready = label.trim() !== "" && command.trim() !== "" && !saving;

    const save = () => {
        if (!ready) return;
        setSaving(true);
        const team = scope === "team";
        void tasksApi
            .add(project, { label, command, cwd, team })
            .then(({ taskId }) => {
                notify("success", team ? `Added ${label.trim()} to sikemux.json; commit it to share it` : `Added ${label.trim()} for you only`);
                onDone(taskId);
            })
            .catch(reportError("add task"))
            .finally(() => setSaving(false));
    };

    return (
        <form
            className="tb-run-add"
            onSubmit={(event) => {
                event.preventDefault();
                save();
            }}
            onKeyDown={(event) => {
                if (event.key === "Escape") {
                    event.stopPropagation();
                    onDone(null);
                }
            }}>
            <label className="tb-run-add-field">
                <span>Name</span>
                <input autoFocus value={label} onChange={(event) => setLabel(event.target.value)} placeholder="Backend" spellCheck={false} />
            </label>
            <label className="tb-run-add-field">
                <span>Command</span>
                <input
                    className="mono"
                    value={command}
                    onChange={(event) => setCommand(event.target.value)}
                    placeholder="bash tool/run.sh"
                    spellCheck={false}
                />
            </label>
            <label className="tb-run-add-field">
                <span>Folder</span>
                <input value={cwd} onChange={(event) => setCwd(event.target.value)} placeholder="The project root" spellCheck={false} />
            </label>
            <div className="tb-run-add-field">
                <Segmented
                    fill
                    small
                    label="Who gets this task"
                    value={scope}
                    options={[
                        { value: "team", label: "For the team" },
                        { value: "personal", label: "Only me" },
                    ]}
                    onChange={setScope}
                />
                <span className="tb-run-add-hint">
                    {scope === "team" ? "Goes in sikemux.json, which you commit." : "Kept in .sikemux/, which Git ignores."}
                </span>
            </div>
            <div className="tb-run-add-foot">
                <button type="button" className="tb-run-check-btn" onClick={() => onDone(null)}>
                    Cancel
                </button>
                <button type="submit" className="tb-run-check-btn primary" disabled={!ready}>
                    {saving ? "Adding…" : "Add task"}
                </button>
            </div>
        </form>
    );
}
