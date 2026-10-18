import { coreCall } from "./core";

export interface Skill {
    name: string;
    description: string;
    path: string;
}

/** Skills an agent keeps in its own global folder, which Sikemux only reads. */
export interface OwnSkills {
    agent: string;
    folder: string;
    skills: Skill[];
}

export interface SkillsOverview {
    library: string;
    skills: Skill[];
    own: OwnSkills[];
    clashes: string[];
}

/** A skill an agent saved into its own global folder while the host ran. */
export interface GlobalSkillAdded {
    agent: string;
    name: string;
    path: string;
}

export const skillsApi = {
    overview: () => coreCall<SkillsOverview>("skillsOverview"),
    create: (name: string, description: string, body: string) => coreCall<Skill>("createSkill", { name, description, body }),
    install: (source: string, subfolder?: string) => coreCall<Skill>("installSkill", { source, subfolder: subfolder || null }),
    remove: (name: string) => coreCall<void>("removeSkill", { name }),
    adopt: (path: string) => coreCall<Skill>("adoptSkill", { path }),
    added: () => coreCall<GlobalSkillAdded[]>("skillsAdded"),
};
