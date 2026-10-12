// Lane r8-pointer: ⌘] (navigation.forward) returns to Settings after ⌘[ left
// it. The reference's Settings is a router location (/settings/<route>): Back
// from it pops to the thread, and Forward pushes the settings route again until
// another location is visited. The clone's own history (keyboard-dispatch.ts
// visit) lists threads and drafts only, so the settings route is kept beside it:
// recorded while Settings is open, offered as Forward while the main view is
// still where Settings was left from, dropped as soon as it moves elsewhere.
type Here = { threadId: string; projectId: string };
const trails = new WeakMap<object, { route: string; here: string }>();
const hereOf = (client: Here) => client.threadId ? `thread:${client.threadId}` : client.projectId ? `draft:${client.projectId}` : '';

/** While Settings is open: remember its route. Otherwise: the route Forward reopens, or ''. */
export function settingsForward(client: Here, settingsOpen: boolean, route = ''): string {
  if (settingsOpen) { trails.set(client, { route: route || 'general', here: hereOf(client) }); return ''; }
  const trail = trails.get(client);
  if (!trail) return '';
  if (trail.here !== hereOf(client)) { trails.delete(client); return ''; }
  return trail.route;
}
