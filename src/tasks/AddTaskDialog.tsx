import { useId, useState } from "react";
import { notify, reportError } from "../state/toast";
import { Segmented } from "../ui/Controls";
import { IconPlus } from "../ui/Icons";
import { Modal } from "../ui/Modal";
import { tasksApi } from "./coreTasks";

type Scope = "team" | "personal";

/** A person's own start command, saved for the team in sikemux.json or for themselves only. */
export function AddTaskDialog({ project, onDone }: { project: string; onDone: (taskId: string | null) => void }) {
    const formId = useId();
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
        <Modal
            title="Add a task"
            glyph={<IconPlus size={15} />}
            onClose={() => onDone(null)}
            foot={
                <>
                    <button type="button" className="dlg-btn" onClick={() => onDone(null)}>
                        Cancel
                    </button>
                    <button type="submit" form={formId} className="dlg-btn primary" disabled={!ready}>
                        {saving ? "Adding…" : "Add task"}
                    </button>
                </>
            }>
            <form
                id={formId}
                className="dlg-fields"
                onSubmit={(event) => {
                    event.preventDefault();
                    save();
                }}>
                <label className="dlg-field">
                    <span>Name</span>
                    <input
                        className="dlg-input"
                        autoFocus
                        value={label}
                        onChange={(event) => setLabel(event.target.value)}
                        placeholder="Backend"
                        spellCheck={false}
                    />
                </label>
                <label className="dlg-field">
                    <span>Command</span>
                    <input
                        className="dlg-input mono"
                        value={command}
                        onChange={(event) => setCommand(event.target.value)}
                        placeholder="bash tool/run.sh"
                        spellCheck={false}
                    />
                </label>
                <label className="dlg-field">
                    <span>Folder</span>
                    <input
                        className="dlg-input"
                        value={cwd}
                        onChange={(event) => setCwd(event.target.value)}
                        placeholder="The project root"
                        spellCheck={false}
                    />
                </label>
                <div className="dlg-field">
                    <span>Save it</span>
                    <Segmented
                        fill
                        label="Who gets this task"
                        value={scope}
                        options={[
                            { value: "team", label: "For the team" },
                            { value: "personal", label: "Only me" },
                        ]}
                        onChange={setScope}
                    />
                    <span className="dlg-field-note">
                        {scope === "team" ? "Goes in sikemux.json, which you commit." : "Kept in .sikemux/, which Git ignores."}
                    </span>
                </div>
            </form>
        </Modal>
    );
}
