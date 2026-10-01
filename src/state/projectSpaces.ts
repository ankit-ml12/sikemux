import type { ProjectSpace, Session, SpaceView } from "./types";

export const PROJECT_SPACES: readonly { space: ProjectSpace; label: string }[] = [
    { space: "work", label: "Work" },
    { space: "personal", label: "Personal" },
];

export const isProjectSpace = (value: unknown): value is ProjectSpace => value === "work" || value === "personal";

/** A project in no space shows in every view; a tagged one only under All and its own space. */
export function isProjectShown(cwd: string, spaces: Readonly<Record<string, ProjectSpace>>, view: SpaceView): boolean {
    const space = spaces[cwd];
    return view === "all" || space === undefined || space === view;
}

export function shownProjects<T extends Pick<Session, "cwd">>(
    projects: readonly T[],
    spaces: Readonly<Record<string, ProjectSpace>>,
    view: SpaceView,
): T[] {
    return projects.filter((project) => isProjectShown(project.cwd, spaces, view));
}
