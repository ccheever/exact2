// Mobile365aa87982 new-task-project-selection and NewTaskFlowProvider environment filter.
// @ref llp/1109.005-composer-and-transcript.decision.md#new-task-ownership
import { obj, str, type Obj } from './shared/domain';
import type { HomeSource } from './home';
const repositoryKey = (project: Obj | null): string | null => {
  const identity = obj(project?.repositoryIdentity);
  return str(obj(identity.origin).canonicalKey) || str(identity.canonicalKey) || null;
};
export function mobileNewTaskEnvironmentMatch(projects: Obj[], selected: Obj | null): Obj | null {
  const repository = repositoryKey(selected), basename = str(selected?.workspaceRoot).split('/').at(-1) || null;
  const mismatch = (project: Obj) => repository !== null && repositoryKey(project) !== null && repositoryKey(project) !== repository;
  return (repository !== null ? projects.find(project => repositoryKey(project) === repository) : undefined)
    ?? (basename !== null ? projects.find(project => !mismatch(project) && str(project.workspaceRoot).split('/').at(-1) === basename) : undefined)
    ?? (selected !== null ? projects.find(project => !mismatch(project) && project.title === selected.title) : undefined)
    ?? projects[0] ?? null;
}
export function mobileNewTaskEnvironmentSources(sources: HomeSource[], selected: Obj | null): HomeSource[] {
  const repository = repositoryKey(selected), basename = str(selected?.workspaceRoot).split('/').at(-1) || null;
  return sources.filter(source => source.shell.projects.some(project => {
    if (project.archivedAt != null) return false;
    if (repository === null && basename === null) return true;
    const key = repositoryKey(project);
    if (repository !== null && key !== null) return repository === key;
    return str(project.workspaceRoot).split('/').at(-1) === basename || selected !== null && project.title === selected.title;
  }));
}
