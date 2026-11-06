// GAP 001: bake cannot capture parent imports. Remove this copy when ancestor mounts work.
// Unchanged body from examples/t3-code/editors.ts at 887b2491b182f851b11253655f6aa84fe2a26708.
// The editors the client knows (MIT reference, see LICENSE-T3: packages/contracts/src/editor.ts
// EDITORS, REMOTE_CAPABLE_EDITOR_IDS, remoteSchemeForEditor, buildRemoteOpenUrl, at 1e2ecbd975).
// `remoteScheme` marks VS Code and the forks that ship Remote-SSH, plus Zed with its own link shape.

export type EditorDefinition = { id: string; label: string; commands: string[] | null; launchStyle: 'direct-path' | 'goto' | 'line-column'; remoteScheme?: string };

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
  { id: 'idea', label: 'IntelliJ IDEA', commands: ['idea'], launchStyle: 'line-column' },
  { id: 'aqua', label: 'Aqua', commands: ['aqua'], launchStyle: 'line-column' },
  { id: 'clion', label: 'CLion', commands: ['clion'], launchStyle: 'line-column' },
  { id: 'datagrip', label: 'DataGrip', commands: ['datagrip'], launchStyle: 'line-column' },
  { id: 'dataspell', label: 'DataSpell', commands: ['dataspell'], launchStyle: 'line-column' },
  { id: 'goland', label: 'GoLand', commands: ['goland'], launchStyle: 'line-column' },
  { id: 'phpstorm', label: 'PhpStorm', commands: ['phpstorm'], launchStyle: 'line-column' },
  { id: 'pycharm', label: 'PyCharm', commands: ['pycharm'], launchStyle: 'line-column' },
  { id: 'rider', label: 'Rider', commands: ['rider'], launchStyle: 'line-column' },
  { id: 'rubymine', label: 'RubyMine', commands: ['rubymine'], launchStyle: 'line-column' },
  { id: 'rustrover', label: 'RustRover', commands: ['rustrover'], launchStyle: 'line-column' },
  { id: 'webstorm', label: 'WebStorm', commands: ['webstorm'], launchStyle: 'line-column' },
  { id: 'file-manager', label: 'Finder', commands: null, launchStyle: 'direct-path' },
];

/** Editors that can open a remote workspace through an SSH deep link. */
export const REMOTE_CAPABLE_EDITOR_IDS: string[] = EDITOR_DEFINITIONS.flatMap(editor => editor.remoteScheme ? [editor.id] : []);

export function remoteSchemeForEditor(id: string): string | undefined {
  return EDITOR_DEFINITIONS.find(editor => editor.id === id)?.remoteScheme;
}

export function editorLabel(id: string): string {
  return EDITOR_DEFINITIONS.find(editor => editor.id === id)?.label ?? id;
}

/**
 * `<scheme>://vscode-remote/ssh-remote+<host><path>` (Zed: `zed://ssh/<host><path>`), which opens
 * `absolutePath` on `host` in the local editor over SSH; undefined for editors without remote links.
 */
export function buildRemoteOpenUrl(input: { editor: string; host: string; absolutePath: string }): string | undefined {
  const scheme = remoteSchemeForEditor(input.editor);
  if (scheme === undefined) return undefined;
  // Windows server paths (`C:\...`) appear as `/C:/...` in vscode-remote URIs.
  const posixPath = input.absolutePath.replace(/\\/g, '/');
  const rootedPath = posixPath.startsWith('/') ? posixPath : `/${posixPath}`;
  const encodedHost = encodeURIComponent(input.host);
  if (input.editor === 'zed') {
    // Zed resolves a rooted path on the system drive: `C:\Users\x` becomes `/Users/x`; other drives stay.
    const zedPath = /^[Cc]:[\\/]/.test(input.absolutePath) ? rootedPath.slice(3) : rootedPath;
    return `${scheme}://ssh/${encodedHost}${zedPath.split('/').map(encodeURIComponent).join('/')}`;
  }
  return `${scheme}://vscode-remote/ssh-remote+${encodedHost}${rootedPath.split('/').map(encodeURIComponent).join('/')}`;
}
