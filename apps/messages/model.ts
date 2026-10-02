// Geometry and timing follow expo/react-native@chat-demo's ui-metrics.md.
// Reference fixture text: Meta Platforms, Inc. and affiliates, MIT (assets/reference-LICENSE.txt).
export type Model = { id: string; name: string; maker: string; initials: string; color: string };
export type Message = {
  id: string; body: string; from: string; at: number; reaction: string;
  reply: string; status: string; fresh: boolean; edited: boolean;
};
export type Thread = {
  id: string; name: string; initials: string; color: string; model: string;
  draft: string; unread: boolean; muted: boolean; messages: Message[];
  job: string; pending: boolean; error: string; response: string;
};
export const MODELS: Model[] = [
  { id: 'anthropic/claude-sonnet-5.5', name: 'Claude Sonnet 5.5', maker: 'Anthropic', initials: 'C', color: '#c47b5c' },
  { id: 'openai/gpt-6.1-sol', name: 'GPT-6.1 Sol', maker: 'OpenAI', initials: 'G', color: '#62a68e' },
  { id: 'google/gemini-3.8-flash', name: 'Gemini 3.8 Flash', maker: 'Google', initials: '✦', color: '#718cd9' },
  { id: 'x-ai/grok-4.7', name: 'Grok 4.7', maker: 'xAI', initials: 'X', color: '#555965' },
  { id: 'deepseek/deepseek-v4.1-flash', name: 'DeepSeek V4.1 Flash', maker: 'DeepSeek', initials: 'D', color: '#587dca' },
  { id: 'meta-llama/llama-4-maverick', name: 'Llama 4 Maverick', maker: 'Meta', initials: '∞', color: '#468cbc' },
];
export function blank(model: Model, id: string): Thread {
  return { id, name: model.name, initials: model.initials, color: model.color, model: model.id,
    draft: '', unread: false, muted: false, messages: [], job: '', pending: false, error: '', response: '' };
}
export function fixtures(): Thread[] {
  const demo = blank({ id: '', name: 'Chat', maker: '', initials: 'AG', color: '#9a91b8' }, 'demo');
  demo.messages = [
    { id: 'demo-1', from: 'Ada Lovelace', body: 'Did the keyboard cover the last message?' },
    { id: 'demo-2', from: 'me', body: 'It should not. Pull it down and watch.' },
    { id: 'demo-3', from: 'Ada Lovelace', body: 'The bar follows the keyboard rather than copying it.' },
  ].map(m => ({ ...m, at: 1770000000000, reaction: '', reply: '', status: m.from === 'me' ? 'Delivered' : '', fresh: false, edited: false }));
  return [demo, ...MODELS.map((m, i) => blank(m, `ai-${i + 1}`))];
}
export function time(at: number, offset = 0): string {
  const d = new Date(at + offset * 60000), h = d.getUTCHours();
  return `${h % 12 || 12}:${String(d.getUTCMinutes()).padStart(2, '0')} ${h < 12 ? 'AM' : 'PM'}`;
}
export function reveal(pulled: number): number { return pulled > 0 ? 66 * (1 - 1 / (1 + pulled / 66)) : 0; }
export function presentation(thread: Thread, clock: number, offset = 0) {
  let receipt = -1;
  thread.messages.forEach((m, i) => { if (m.from === 'me' && (m.status || clock - m.at >= 1130)) receipt = i; });
  return thread.messages.map((m, i, rows) => {
    const outgoing = m.from === 'me', previous = rows[i - 1], next = rows[i + 1];
    const closes = next?.from !== m.from || i === receipt;
    const elapsed = m.fresh ? clock - m.at : 100000;
    const status = i !== receipt ? '' : m.status || (elapsed >= 4650 ? `Read ${time(m.at, offset)}` : 'Delivered');
    return {
      ...m, outgoing, time: time(m.at, offset), tail: closes || !!next?.reaction,
      gap: closes && !!next ? 8 : 2,
      sender: !outgoing && !thread.model && previous?.from !== m.from ? m.from : '',
      avatar: !outgoing && !thread.model && closes,
      initials: m.from.split(' ').map(w => w[0]).join('').slice(0, 2),
      stamp: !previous || m.at - previous.at > 900000 ? `Today ${time(m.at, offset)}` : '',
      receipt: status, receiptNew: m.fresh && elapsed >= 1130 && elapsed < 2150,
      truncated: m.body.length > 6000, preview: m.body.length > 6000 ? m.body.slice(0, 240) + '…' : m.body,
      arrival: m.fresh && elapsed < 900,
    };
  });
}
