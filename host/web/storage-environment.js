// Storage's synchronous namespace and host clock are captured before a module
// Worker hardens its globals. The I/O adapters themselves can load on first use.
export const directories = Object.freeze({ data: 'app:/data', cache: 'app:/cache', temporary: 'app:/tmp' });
export const now = Date.now.bind(Date);
