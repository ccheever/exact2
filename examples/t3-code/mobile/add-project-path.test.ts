import { describe, expect, test } from 'bun:test';
import * as paths from './add-project-path';

describe('mobile Add Project paths and sources', () => {
  test('an explicit unavailable machine never falls back to another server', () => {
    const options = [{ environmentId: 'offline', connectionState: 'reconnecting' }, { environmentId: 'first', connectionState: 'connected' }, { environmentId: 'second', connectionState: 'connected' }];
    expect(paths.resolveAddProjectEnvironment(options, null)).toBe(options[1]);
    expect(paths.resolveAddProjectEnvironment(options, 'second')).toBe(options[2]);
    expect(paths.resolveAddProjectEnvironment(options, 'offline')).toBeNull();
    expect(paths.resolveAddProjectEnvironment(options, 'missing')).toBeNull();
    expect(paths.resolveAddProjectEnvironment([], null)).toBeNull();
    expect(paths.stringParam(['', 'github'])).toBe('');
    expect(paths.sourceFromParam(['gitlab', 'github'])).toBe('gitlab');
    expect(paths.sourceFromParam(['wrong', 'github'])).toBe('url');
    expect(paths.sourceFromParam(undefined)).toBe('url');
  });

  test('mobile local paths refuse project-relative and foreign Windows paths', () => {
    for (const rawPath of ['.', '..', './app', '../app', '.\\app', '..\\app']) {
      expect(paths.resolveAddProjectPath({ rawPath, currentProjectCwd: null, platform: 'MacIntel' })).toEqual({ ok: false, error: 'Relative paths require an active project in this environment.' });
    }
    for (const rawPath of ['C:\\code', 'C:/code', '\\\\host\\share']) {
      expect(paths.resolveAddProjectPath({ rawPath, currentProjectCwd: null, platform: 'Linux' }).ok).toBe(false);
      expect(paths.resolveAddProjectPath({ rawPath, currentProjectCwd: null, platform: 'Win32' }).ok).toBe(true);
    }
    expect(paths.resolveAddProjectPath({ rawPath: ' ~/project/// ', platform: 'Linux' })).toEqual({ ok: true, path: '~/project' });
    expect(paths.resolveAddProjectPath({ rawPath: ' ', platform: 'Linux' }).ok).toBe(false);
  });

  test('duplicate identity preserves Unix backslashes and ignores Windows case', () => {
    const projects = [{ environmentId: 'other', workspaceRoot: '/same', id: 'other' }, { environmentId: 'here', workspaceRoot: 'C:\\Code\\Repo\\', id: 'win' }, { environmentId: 'here', workspaceRoot: '/same\\', id: 'unix' }];
    expect(paths.findExistingAddProject({ projects, environmentId: 'here', path: 'c:/code/repo' })?.id).toBe('win');
    expect(paths.findExistingAddProject({ projects, environmentId: 'here', path: '/same' })).toBeNull();
    expect(paths.findExistingAddProject({ projects, environmentId: 'here', path: '/same\\/' })?.id).toBe('unix');
    expect(paths.normalizeProjectPathForComparison('C:')).toBe(paths.normalizeProjectPathForComparison('c:/'));
    expect(paths.buildProjectCreateCommand({ commandId: 'cmd', projectId: 'project', workspaceRoot: 'C:\\Code\\Repo' })).toEqual({ type: 'project.create', commandId: 'cmd', projectId: 'project', title: 'Repo', workspaceRoot: 'C:\\Code\\Repo', createWorkspaceRootIfMissing: true, defaultModelSelection: null });
  });

  test('browse roots and directory separators follow the selected machine', () => {
    expect(paths.getFilesystemBrowsePath('C:\\Code\\Re', 'Win32')).toEqual({ isBrowsing: true, directoryPath: 'C:\\Code\\', filterQuery: 'Re', parentPath: 'C:\\', canBrowseUp: true });
    expect(paths.getFilesystemBrowsePath('C:\\Code\\Re', 'Linux').isBrowsing).toBe(false);
    expect(paths.getBrowseParentPath('\\\\server\\share\\')).toBeNull();
    expect(paths.getBrowseParentPath('\\\\server\\share\\code\\')).toBe('\\\\server\\share\\');
    expect(paths.getBrowseParentPath('/')).toBeNull();
    expect(paths.getFilesystemBrowsePath('~/').canBrowseUp).toBe(false);
    expect(paths.ensureBrowseDirectoryPath('C:\\code')).toBe('C:\\code\\');
    expect(paths.appendBrowsePathSegment('/home/a\\b/', 'repo')).toBe('/home/a\\b/repo/');
  });

  test('pinned clone folder does not hide browsing or double append itself', () => {
    expect(paths.getCloneDestinationBrowsePath({ browseDirectoryPath: 'C:\\code\\', selectedDirectoryName: 'REPO', cloneDirectoryName: 'repo', caseSensitive: false })).toBe('C:\\code\\REPO\\');
    expect(paths.getCloneDestinationBrowsePath({ browseDirectoryPath: '/code/', selectedDirectoryName: 'REPO', cloneDirectoryName: 'repo', caseSensitive: true })).toBe('/code/REPO/repo');
    expect(paths.getPinnedBrowseFilter('REPO', 'repo', 'Win32')).toBe('');
    expect(paths.getPinnedBrowseFilter('REPO', 'repo', 'Linux')).toBe('REPO');
    expect(paths.getPinnedBrowseFilter('rep', 'repo', 'Win32')).toBe('rep');
    const entries = [{ name: '.git' }, { name: 'Alpha' }, { name: 'alpha' }, { name: 'Beta' }];
    expect(paths.filterFilesystemBrowseEntries(entries, '').visibleEntries).toEqual(entries.slice(1));
    expect(paths.filterFilesystemBrowseEntries(entries, '.').visibleEntries).toEqual([entries[0]]);
    expect(paths.filterFilesystemBrowseEntries(entries, 'alpha')).toEqual({ visibleEntries: entries.slice(1, 3), exactEntry: entries[2] });
  });

  test('clone URL defaults and names preserve provider semantics', () => {
    expect(paths.normalizePastedCloneUrl(' owner/my.repo ')).toBe('https://github.com/owner/my.repo.git');
    expect(paths.normalizePastedCloneUrl(' git@host:owner/repo.git ')).toBe('git@host:owner/repo.git');
    for (const value of ['https://host', 'https://host:22', 'ssh://git@host:22']) expect(paths.getCloneDirectoryName(value)).toBe('');
    for (const value of ['org/project/repo', 'git@host:owner/repo.git', 'https://host/owner/repo.git/?a#b']) expect(paths.getCloneDirectoryName(value)).toBe('repo');
    expect(paths.getCloneDirectoryName('https://host/org/123')).toBe('123');
    for (const provider of ['github', 'forgejo', 'gitlab', 'bitbucket', 'azure-devops']) expect(paths.getDefaultCloneUrl({ provider, url: 'https', sshUrl: 'ssh' })).toBe(['github', 'forgejo'].includes(provider) ? 'https' : 'ssh');
  });

  test('new project preview matches server slug and platform reserved names', () => {
    expect(paths.newProjectFolderName(' Déjà Vu._ ')).toBe('deja-vu');
    expect(paths.newProjectFolderName('CON')).toBe('con-project');
    expect(paths.newProjectFolderName('LPT1')).toBe('lpt1-project');
    expect(paths.newProjectFolderName('🎉')).toBe('project');
    expect(paths.newProjectFolderName('a'.repeat(80))).toHaveLength(64);
    expect(paths.getNewProjectPathPreview('C:\\projects', 'My app')).toBe('C:\\projects\\my-app');
    expect(paths.getNewProjectGitHubRepository({ account: 'octocat' }, 'C:\\projects\\my-app')).toBe('octocat/my-app');
  });

  test('readiness keeps unavailable hints, null versus empty detail, and source order', () => {
    const discovery = { sourceControlProviders: [
      { kind: 'github', label: 'GitHub', status: 'available', auth: { status: 'authenticated', account: 'octocat' } },
      { kind: 'gitlab', label: 'GitLab', status: 'available', auth: { status: 'unauthenticated', detail: '' } },
      { kind: 'forgejo', label: 'Forgejo', status: 'missing', installHint: 'Install fj', auth: {} },
      { kind: 'bitbucket', label: 'Bitbucket', status: 'available', auth: { status: 'unauthenticated', detail: null } },
    ] };
    const ready = paths.buildAddProjectRemoteSourceReadiness(discovery);
    expect(ready.github).toEqual({ ready: true, hint: null });
    expect(ready.gitlab).toEqual({ ready: false, hint: '' });
    expect(ready.forgejo).toEqual({ ready: false, hint: 'Install fj' });
    expect(ready.bitbucket.hint).toBe('Bitbucket is not authenticated. Open Source Control settings for setup guidance.');
    expect(paths.sortAddProjectProviderSources(ready)).toEqual(['github', 'azure-devops', 'bitbucket', 'forgejo', 'gitlab']);
    expect(paths.getNewProjectGitHubTarget(discovery)).toEqual({ account: 'octocat' });
    expect(paths.getNewProjectGitHubTarget(null)).toBeNull();
    expect(paths.getNewProjectGitHubTarget({ sourceControlProviders: [{ kind: 'github', status: 'available', auth: { status: 'authenticated', account: null } }] })).toEqual({ account: null });
  });
});
