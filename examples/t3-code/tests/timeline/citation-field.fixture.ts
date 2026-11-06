import { T3Client } from '../../src/client/client';
import type { Native } from '../../src/client/protocol';
import type { ComposerFieldCommand } from '../../src/composer/composer-field-commands';
import { composerFieldOwner } from '../../src/composer/auto-balance-owner';
import { composerFieldAcknowledged, composerFieldNative, composerFieldObserved, composerFieldResource, type ComposerFieldObservation } from '../../src/composer/composer-field-bridge';
import { composerChipOpen } from '../../src/composer/composer-chip-bridge';
import { composerSourceSession } from '../../src/composer/composer-rich-state';

/** Executes the shared request/input/independent-ACK boundary. Native mutation
 *  requests are rejected so a caller cannot accidentally bypass this queue. */
export function citationField(value = '', start = value.length, end = start, client = new T3Client(), raw?: Native) {
  if (!client.threadId) Object.assign(client, { environmentId: 'env', threadId: 'verify-timeline' });
  const owner = () => composerFieldOwner(client), calls: unknown[] = [];
  let observation: ComposerFieldObservation = { generation: client.generation, snapshotOwner: client.snapshotOwner, owner: owner(), value,
    start, end, composing: false, live: true, focused: true, inputSerial: 0 };
  let after = 0;
  const session = composerSourceSession(client);
  const publish = (next: ComposerFieldObservation, input: boolean) => {
    observation = next;
    session.observe(next.owner, next.value);
    return composerFieldObserved(client, [next, input], owner());
  };
  publish(observation, false);
  const underlying: Native = raw ?? { available: true, watch() {}, async later(request) {
    calls.push(request);
    if ((request as {op?:string}).op === 'wake') return {ok:true,generation:client.generation,value:{}};
    throw new Error('Unexpected native composer request');
  } };
  const native = composerFieldNative(client, underlying, owner);
  return {
    client, native, calls, owner, session, current: () => observation,
    next: () => composerFieldResource(client, [observation, after], owner()),
    input(command: ComposerFieldCommand) {
      const source = command.value.slice(0, command.start) + command.text + command.value.slice(command.end);
      publish({ ...observation, value: source, start: command.selectionStart, end: command.selectionEnd, inputSerial: observation.inputSerial + 1 }, true);
      return observation;
    },
    observe(changes: Partial<ComposerFieldObservation>, input = false) {
      return publish({ ...observation, ...changes }, input);
    },
    type(source: string) {
      return publish({ ...observation, value: source, start: source.length, end: source.length, inputSerial: observation.inputSerial + 1 }, true);
    },
    ack(command: ComposerFieldCommand, applied = true) {
      after = command.seq;
      return composerFieldAcknowledged(client, [observation, command.seq, applied], owner(), underlying);
    },
    press(kind = 'citation') {
      const chip = session.chips.find(candidate => candidate.kind === kind);
      if (!chip) throw new Error(`Missing ${kind} chip`);
      const press = composerChipOpen(client, { ...observation, ...chip, surface: 'composer', comment: kind === 'citation',
        frame: { unavailable: false, x: 10, y: 20, width: 80, height: 18 } });
      if (!press) throw new Error('Shared chip press refused');
      return press;
    },
  };
}
