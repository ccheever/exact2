// Storage's synchronous namespace and host clock are captured before a module
// Worker hardens its globals. The I/O adapters themselves can load on first use.
export const directories = Object.freeze({ data: 'app:/data', cache: 'app:/cache', temporary: 'app:/tmp' });
export const now = Date.now.bind(Date);
// Whose store a page opens: the app's, or for a scripted drive (`?agent`) only
// a scratch store the drive names (`&storage=<name>`, `agent.mjs --storage`),
// apart from the app's own, as on native. null: this drive has no storage.
export function storageKey(appId, href) {
  const params = new URL(href).searchParams, name = params.get('storage');
  if (!params.has('agent')) return appId;
  if (name == null) return null;
  if (!/^[A-Za-z0-9._-]+$/.test(name) || name === '.' || name === '..') throw new Error("storage: one name of letters, digits, '.', '-' or '_'");
  return `${appId}/agent/${name}`;
}
export const agentStorageRefusal = 'storage is unavailable in agent mode unless the drive names a scratch store (--storage <name>)';
