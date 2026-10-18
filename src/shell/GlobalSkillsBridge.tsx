import { useEffect } from "react";
import { onCoreChanged } from "../api/coreChanged";
import { skillsApi } from "../api/skills";
import { notify, reportError, swallow } from "../state/toast";

/** Offers to move a skill an agent saved into its own global folder into Sikemux, where every agent gets it. */
export function GlobalSkillsBridge() {
    useEffect(() => {
        const offered = new Set<string>();
        const offer = () =>
            skillsApi
                .added()
                .then((added) => {
                    for (const { agent, name, path } of added) {
                        if (offered.has(path)) continue;
                        offered.add(path);
                        notify("info", `${agent} saved the skill ${name} in its own folder, so only ${agent} has it, outside Sikemux too.`, {
                            action: {
                                label: "Move into Sikemux",
                                run: () =>
                                    skillsApi
                                        .adopt(path)
                                        .then(() => notify("success", `${name} is in Sikemux now, for every agent's next session.`))
                                        .catch(reportError("Move skill")),
                            },
                        });
                    }
                })
                .catch(swallow("skills the agents saved"));
        return onCoreChanged("skills", () => void offer());
    }, []);
    return null;
}
