// The editors the client knows (MIT reference, see LICENSE-T3: packages/contracts/src/editor.ts
// EDITORS, REMOTE_CAPABLE_EDITOR_IDS, remoteSchemeForEditor, at 9c161462c9; buildRemoteOpenUrl at 74541bab17).
// `remoteScheme` marks VS Code and the forks that ship Remote-SSH, plus Zed with its own link shape.
// `jetbrainsProductCode` (`IU`, `PY`, …) marks the JetBrains IDEs: they open remote projects through
// the Toolbox App's `jetbrains://gateway/ssh/…` link, which uses the code to choose the IDE backend.

export type EditorDefinition = { id: string; label: string; commands: string[] | null; launchStyle: 'direct-path' | 'goto' | 'line-column'; remoteScheme?: string; jetbrainsProductCode?: string };

/** EDITORS in the reference's order; `file-manager` is labelled Finder on macOS (editorLabelForPlatform). */
export const EDITOR_DEFINITIONS: EditorDefinition[] = [
  { id: 'cursor', label: 'Cursor', commands: ['cursor'], launchStyle: 'goto', remoteScheme: 'cursor' },
  { id: 'trae', label: 'Trae', commands: ['trae'], launchStyle: 'goto' },
  { id: 'kiro', label: 'Kiro', commands: ['kiro'], launchStyle: 'goto' },
  { id: 'vscode', label: 'VS Code', commands: ['code'], launchStyle: 'goto', remoteScheme: 'vscode' },
  { id: 'vscode-insiders', label: 'VS Code Insiders', commands: ['code-insiders'], launchStyle: 'goto', remoteScheme: 'vscode-insiders' },
  { id: 'vscodium', label: 'VSCodium', commands: ['codium'], launchStyle: 'goto', remoteScheme: 'vscodium' },
  { id: 'zed', label: 'Zed', commands: ['zed', 'zeditor'], launchStyle: 'direct-path', remoteScheme: 'zed' },
  { id: 'antigravity', label: 'Antigravity', commands: ['antigravity-ide', 'agy-ide'], launchStyle: 'goto' },
  { id: 'idea', label: 'IntelliJ IDEA', commands: ['idea'], launchStyle: 'line-column', jetbrainsProductCode: 'IU' },
  { id: 'aqua', label: 'Aqua', commands: ['aqua'], launchStyle: 'line-column', jetbrainsProductCode: 'QA' },
  { id: 'clion', label: 'CLion', commands: ['clion'], launchStyle: 'line-column', jetbrainsProductCode: 'CL' },
  { id: 'datagrip', label: 'DataGrip', commands: ['datagrip'], launchStyle: 'line-column', jetbrainsProductCode: 'DB' },
  { id: 'dataspell', label: 'DataSpell', commands: ['dataspell'], launchStyle: 'line-column', jetbrainsProductCode: 'DS' },
  { id: 'goland', label: 'GoLand', commands: ['goland'], launchStyle: 'line-column', jetbrainsProductCode: 'GO' },
  { id: 'phpstorm', label: 'PhpStorm', commands: ['phpstorm'], launchStyle: 'line-column', jetbrainsProductCode: 'PS' },
  { id: 'pycharm', label: 'PyCharm', commands: ['pycharm'], launchStyle: 'line-column', jetbrainsProductCode: 'PY' },
  { id: 'rider', label: 'Rider', commands: ['rider'], launchStyle: 'line-column', jetbrainsProductCode: 'RD' },
  { id: 'rubymine', label: 'RubyMine', commands: ['rubymine'], launchStyle: 'line-column', jetbrainsProductCode: 'RM' },
  { id: 'rustrover', label: 'RustRover', commands: ['rustrover'], launchStyle: 'line-column', jetbrainsProductCode: 'RR' },
  { id: 'webstorm', label: 'WebStorm', commands: ['webstorm'], launchStyle: 'line-column', jetbrainsProductCode: 'WS' },
  { id: 'file-manager', label: 'Finder', commands: null, launchStyle: 'direct-path' },
];

/** Editors that can open a remote workspace through an SSH deep link. */
const remoteSchemeOf = (editor: EditorDefinition): string | undefined => editor.remoteScheme ?? (editor.jetbrainsProductCode === undefined ? undefined : 'jetbrains');
export const REMOTE_CAPABLE_EDITOR_IDS: string[] = EDITOR_DEFINITIONS.flatMap(editor => remoteSchemeOf(editor) ? [editor.id] : []);

export function remoteSchemeForEditor(id: string): string | undefined {
  const editor = EDITOR_DEFINITIONS.find(candidate => candidate.id === id);
  return editor ? remoteSchemeOf(editor) : undefined;
}

export function editorLabel(id: string): string {
  return EDITOR_DEFINITIONS.find(editor => editor.id === id)?.label ?? id;
}

/**
 * `<scheme>://vscode-remote/ssh-remote+[<user>@]<host><path>` (Zed: `zed://ssh/[<user>@]<host><path>`, JetBrains
 * IDEs a Toolbox App `jetbrains://gateway/ssh/environment?h=<host>&u=<user>…` link), which opens `absolutePath` on
 * `host` in the local editor over SSH, as `username` when one is known; undefined for editors without remote links.
 * VS Code opens a file only with a position after it (`isFile`).
 */
export function buildRemoteOpenUrl(input: { editor: string; host: string; username?: string; absolutePath: string; isFile?: boolean }): string | undefined {
  const scheme = remoteSchemeForEditor(input.editor);
  if (scheme === undefined) return undefined;
  const jetbrainsProductCode = EDITOR_DEFINITIONS.find(editor => editor.id === input.editor)?.jetbrainsProductCode;
  if (jetbrainsProductCode !== undefined) {
    // No port: the SSH config entry for `host` supplies it. A bare product code lets Toolbox pick the backend build.
    const params = new URLSearchParams({ h: input.host, launchIde: 'true', ideHint: jetbrainsProductCode, projectHint: input.absolutePath.replace(/\\/g, '/') });
    if (input.username !== undefined) params.set('u', input.username);
    return `${scheme}://gateway/ssh/environment?${params.toString()}`;
  }
  // Windows server paths (`C:\...`) appear as `/C:/...` in vscode-remote URIs.
  const posixPath = input.absolutePath.replace(/\\/g, '/');
  const rootedPath = posixPath.startsWith('/') ? posixPath : `/${posixPath}`;
  const encodedHost = encodeURIComponent(input.host);
  if (input.editor === 'zed') {
    // Zed resolves a rooted path on the system drive: `C:\Users\x` becomes `/Users/x`; other drives stay.
    const zedPath = /^[Cc]:[\\/]/.test(input.absolutePath) ? rootedPath.slice(3) : rootedPath;
    // Zed reads this segment as ssh URL userinfo, so the `@` stays literal.
    const destination = input.username === undefined ? encodedHost : `${encodeURIComponent(input.username)}@${encodedHost}`;
    return `${scheme}://ssh/${destination}${zedPath.split('/').map(encodeURIComponent).join('/')}`;
  }
  const destination = input.username === undefined ? input.host : `${input.username}@${input.host}`;
  // VS Code opens a bare vscode-remote path as a folder; a trailing position makes it open the file.
  return `${scheme}://vscode-remote/ssh-remote+${encodeURIComponent(destination)}${rootedPath.split('/').map(encodeURIComponent).join('/')}${input.isFile ? ':1:1' : ''}`;
}
