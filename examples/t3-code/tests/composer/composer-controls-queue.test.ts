// Lane composer-controls: composer commands take the snapshot clock.
import { describe, expect, test } from 'bun:test';
import { arr } from '../../src/client/domain';
import { opened, running } from '../support/composer-controls-fixture';
import { snapshot } from '../../src/client/presentation';

describe('composer commands', () => {
  test('composer commands run without Date.now() (data sources have none): the snapshot clock stands in', async () => {
    const { client, native, command } = await opened();
    client.local.clientSettings.followUpBehavior = 'queue'; // follow-ups steer by default
    snapshot(client, Date.parse('2026-10-03T02:00:00.000Z'));
    running(client);
    const real = Date.now;
    Date.now = () => { throw new Error('Date.now() is unavailable in data sources; pass time or a random seed as an argument'); };
    try {
      const result = await command('send', '', 'a follow-up while running');
      expect(result.message).toBe('');
      expect(native.committed.at(-1)).toMatchObject({ type: 'message.dispatch', dispatchMode: { type: 'queue_after_active' } });
      expect((await command('cclocal:dismiss-woke')).message).toBe('');
      expect((await command('cclocal:usage-limits')).message).toBe('');
      expect(client.error).toBe('');
    } finally { Date.now = real; }
  });
});
