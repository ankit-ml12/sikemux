import type { InboxItem, ProjectInfo } from '@/core/protocol';

export function folder(path: string): string {
  return path.split('/').filter(Boolean).pop() ?? path;
}

export function inProject(project: ProjectInfo, path: string): boolean {
  return path === project.path || path.startsWith(`${project.path}/`);
}

/** The item's card is on `project`'s board, or its agent works there. */
export function itemIn(item: InboxItem, project: ProjectInfo): boolean {
  const where = item.card?.project ?? item.project;
  return where !== undefined && (where === project.id || inProject(project, where));
}
