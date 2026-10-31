// T3 Code 9c161462c9 SecretRequestCard.tsx (MIT, LICENSE-T3).
import type { T3Client } from '../client/client';
import { str, type Obj } from '../client/domain';
import { secretRequestDisplay, secretRequestPrivacyNote, SECRET_REQUEST_DEFAULT_PLACEHOLDER } from './secret-request';
import { secretAnswerKey, secretAnswerState } from './secret-answers';
export function secretCard(client: T3Client, item: Obj, visibility: unknown) {
  const secretStatus = item.secretStatus;
  if (secretStatus !== 'pending' && secretStatus !== 'saved' && secretStatus !== 'declined' && secretStatus !== 'cancelled') return [];
  const display = secretRequestDisplay({ secretStatus }, visibility === 'local' ? 'local' : visibility === 'synthetic' ? 'synthetic' : 'inherited');
  const answer = secretAnswerState(client, str(item.threadId), str(item.id));
  return [{ id: str(item.id), key: secretAnswerKey(client, str(item.threadId), str(item.id)), label: str(item.label), reason: str(item.reason),
    placeholder: typeof item.placeholder === 'string' ? item.placeholder : SECRET_REQUEST_DEFAULT_PLACEHOLDER,
    privacy: secretRequestPrivacyNote({ ...item.delivery === 'file' ? { delivery: 'file' } : {} }),
    kind: display.kind, outcome: display.kind === 'pending' ? '' : display.label,
    icon: display.kind === 'pending-elsewhere' ? 'lock' : display.kind === 'answered' && display.outcome === 'saved' ? 'check-mark' : 'minus',
    disabled: !client.writable || answer.pending || answer.submitted, pending: answer.pending, submitted: answer.submitted, error: answer.error }];
}
