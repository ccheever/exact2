// The Linux build's desktop entry (LLP 1133.023 D3, D17): the app, and the
// manifest's `shortcuts` as its `Actions`, each running the app with its
// location as the first argument, which the host's `launch_location` reads.
// The format is the Desktop Entry Specification's
// (https://specifications.freedesktop.org/desktop-entry-spec/latest/):
// the Exec key's quoting and `%` escape, and an action's group named
// `[Desktop Action <id>]` for each id `Actions` lists.

/** An Exec argument: quoted when it holds a reserved character, `%` doubled (the spec's "The Exec key"). */
export function execArgument(text) {
  const escaped = String(text).replace(/%/g, '%%');
  if (!/[\s"'\\><~|&;$*?#()`]/.test(escaped)) return escaped;
  return `"${escaped.replace(/(["`$\\])/g, '\\$1')}"`;
}

/** A value of a string key: newlines, tabs and backslashes escaped (the spec's "Possible value types"), which
 * applies to an Exec line after its quoting. */
const value = (text) => String(text).replace(/\\/g, '\\\\').replace(/\n/g, '\\n').replace(/\t/g, '\\t').replace(/\r/g, '\\r');

/** An action's id: the spec allows A-Za-z0-9 and `-`; from its name, unique among the entry's. */
function actionIds(items) {
  const seen = new Set();
  return items.map((item, i) => {
    let id = String(item.name).replace(/[^A-Za-z0-9]+/g, '-').replace(/^-+|-+$/g, '').toLowerCase() || `shortcut-${i + 1}`;
    while (seen.has(id)) id += `-${i + 1}`;
    seen.add(id);
    return id;
  });
}

/** The desktop entry of `app`'s Linux build at `binary`. */
export function desktopEntry(app, binary) {
  const items = app.manifest.shortcuts ?? [];
  const ids = actionIds(items);
  const name = app.displayName ?? app.manifest.app?.name ?? app.manifest.name ?? app.id;
  const lines = ['[Desktop Entry]', 'Type=Application', `Name=${value(name)}`, `Exec=${value(execArgument(binary))}`, 'Terminal=false'];
  if (items.length) lines.push(`Actions=${ids.join(';')};`);
  items.forEach((item, i) => {
    lines.push('', `[Desktop Action ${ids[i]}]`, `Name=${value(item.name)}`, `Exec=${value(`${execArgument(binary)} ${execArgument(item.url)}`)}`);
  });
  return lines.join('\n') + '\n';
}
