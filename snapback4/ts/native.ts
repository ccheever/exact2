import type { Device, NativeModule, Request } from './snapback.ts';

// Dispatch's sync/deliver also apply pages, adopt backends, replay predictions
// and replace stores. Default unknown operations to later so a new row walk
// cannot silently enter Exact's 100 ms source turn.
const synchronous = new Set([
  'engine_info', 'engine_headers', 'status', 'sync_state', 'state', 'meta',
  'queued', 'outcome', 'read', 'query', 'route', 'backend', 'write_id',
  'changes', 'changed', 'refresh', 'refreshed', 'cancel',
]);

export function nativeDevice(native: NativeModule): Device {
  return {
    async call(request) {
      if (synchronous.has(String(request.op))) {
        const answer = native.call(request) as Request;
        if ((answer.denied as Request | undefined)?.code !== 'E_BUSY') return answer;
      }
      if (!native.later) throw new Error('Snapback4 requires native.later for device work');
      // A busy cheap call retries on its own host request. Awaiting another
      // source's promise would strand a worker-placed answer on work it cannot run.
      return await native.later(request) as Request;
    },
  };
}
