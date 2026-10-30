// The stand-in world's content that `transcript-world.ts` turns into Discord's shapes: the transcript's
// test history (Excord LLP 0001 §4.4), generated the same every run.
export interface User { id: string; username: string; global_name: string | null; bot: boolean; tint: number }
export interface Attachment { id: string; url: string; filename: string; width?: number; height?: number; spoiler: boolean }
export interface Embed {
  type: string; title?: string; description?: string; url?: string; color?: number;
  provider?: { name: string }; author?: { name: string };
  fields?: { name: string; value: string; inline?: boolean }[]; footer?: { text: string }; timestamp?: string;
  image?: { url: string; width?: number; height?: number };
  thumbnail?: { url: string; width?: number; height?: number };
}
export interface Message {
  id: string; channel_id: string; author: User; content: string; timestamp: number;
  edited_timestamp: number | null; attachments: Attachment[]; embeds: Embed[];
  reactions: { emoji: { id: string | null; name: string }; count: number; me: boolean }[];
  message_reference?: { message_id: string }; nonce?: string; type: number;
  sticker_items?: { id: string; name: string; format_type: number }[];
  forwarded?: string;
  poll?: Record<string, unknown>;
  // An app's answer to a slash command, and who used it.
  command?: { name: string; by: string };
  state: 'sent' | 'pending' | 'failed'; revision: number;
}

export const WORLD_NOW = Date.UTC(2026, 9, 22, 18, 0, 0);
const EPOCH = Date.UTC(2015, 0, 1);
export const members: User[] = [
  ['me', 'charlie', 'Charlie'], ['101', 'ada', 'Ada'], ['102', 'grace', 'Grace'],
  ['103', 'lin', 'Lin Chen'], ['104', 'maya', 'Maya Patel'], ['105', 'sam', 'Sam Rivera'],
  ['106', 'nora', 'Nora Williams'], ['107', 'leo', 'Leo García'], ['108', 'imani', 'Imani Brooks'],
  ['109', 'jules', 'Jules Martin'], ['110', 'priya', 'Priya Shah'], ['111', 'owen', 'Owen Kim'],
  ['112', 'alex', 'Alex Morgan'], ['113', 'ci', 'Buildkite'], ['114', 'release', 'Release Bot'],
].map(([id, username, global_name], i) => ({ id, username, global_name, tint: i % 8, bot: i >= 13 }));

export const channelSpecs = [
  { id: 'general', name: 'general', topic: 'Building Exact, and everything around it.', count: 10000, unread: 37, days: 400, seed: 721 },
  { id: 'help', name: 'help', topic: 'Small reproductions and patient explanations.', count: 1200, unread: 260, days: 120, seed: 819 },
  { id: 'random', name: 'random', topic: 'Bikes, lunch, and the view from outside.', count: 300, unread: 0, days: 90, seed: 917 },
  // Never read: its read state names no message.
  { id: 'announcements', name: 'announcements', topic: 'Releases and community notes.', count: 60, unread: 60, days: 360, seed: 1015 },
  // Age-restricted (Discord's `nsfw`): it opens behind Excord's age gate.
  { id: 'after-hours', name: 'after-hours', topic: 'Late-night talk, for adults.', count: 40, unread: 0, days: 30, seed: 1113, nsfw: true },
];

export const emojiNames = ['exact', 'ferris', 'party', 'shipit', 'coffee', 'bike', 'spark', 'heart', 'check'];

export const photos = [
  { name: 'harbor', width: 960, height: 540 }, { name: 'ridge', width: 800, height: 600 },
  { name: 'cafe', width: 640, height: 640 }, { name: 'tower', width: 480, height: 960 },
  { name: 'coast', width: 1200, height: 360 }, { name: 'trail', width: 600, height: 900 },
  { name: 'desk', width: 900, height: 600 }, { name: 'garden', width: 720, height: 720 },
  { name: 'bridge', width: 1000, height: 500 }, { name: 'noodles', width: 600, height: 480 },
];

export function idAt(at: number, sequence: number): string {
  return String(Math.max(0, Math.floor((at - EPOCH) / 1000))).padStart(11, '0') + String(sequence).padStart(8, '0');
}

function random(seed: number): () => number {
  return () => {
    let t = seed += 0x6D2B79F5;
    t = Math.imul(t ^ t >>> 15, t | 1);
    t ^= t + Math.imul(t ^ t >>> 7, t | 61);
    return ((t ^ t >>> 14) >>> 0) / 4294967296;
  };
}

const chat = [
  'Morning! Coffee is on.', 'That fixed it, thank you.', 'The smallest reproduction is twelve lines now.',
  'I can take a look after lunch.', 'Has anyone tried this with VoiceOver?', 'Nice, that feels much better.',
  'I pushed a screenshot to the thread.', 'The simulator was lying to me again.', 'One more run on a real phone.',
  'I think the parent is constraining the width.', 'The docs example could use a second paragraph.',
  'This is why I keep the old test around.', 'Can reproduce on my machine.', 'The window stays put now 🎉',
  'The Rust error message actually helped this time.', 'Swift value semantics save the day.',
  'The first frame looks right. Checking the second.', 'It was a stale cache entry all along.',
  'A tiny change, but scrolling feels different.', 'Lunch at the noodle place?', 'Back from the bike shop.',
  'Windy on the bridge today.', 'The new tires are so much quieter.', 'Anyone riding the coast on Saturday?',
  'I made way too much focaccia.', 'The garden finally has tomatoes.', 'Rain check on the hike?',
  'That trail is worth the climb.', 'My cat has opinions about mechanical keyboards.', 'Tea break, back in ten.',
  'I like the version with less chrome.', 'Does the selection survive a resize?', 'The profiler agrees with you.',
  'A name is harder than the implementation.', 'The simplest fix was deleting the extra wrapper.',
  'Looks good at the largest text size too.', 'I will check the landscape layout.', 'We should test the empty state.',
  'Good catch on the trailing newline.', 'Can we keep the original timestamp?', 'The animation is finally interruptible.',
  'This screenshot is from the slow phone.', 'A narrow window found another edge case.', 'The build is green again.',
  'I forgot the min-width. Of course.', 'The list is doing less work now.', 'That is a useful failure message.',
  'Tomorrow works for me.', 'Saved this for the next release notes.', 'Thanks for explaining the ownership bit.',
];
const subjects = ['layout engine', 'text measurement', 'keyboard inset', 'scroll anchor', 'image decode',
  'Rust iterator', 'Swift task', 'UIKit collection', 'Exact list', 'reply preview', 'release build', 'font fallback'];
const observations = ['stays stable after a resize', 'needs a smaller reproduction', 'works with an empty input',
  'was allocating on every frame', 'behaves differently in landscape', 'is ready for another review',
  'finally passes the accessibility check', 'is much easier to follow now', 'only fails with a very long line',
  'keeps the original order', 'looks good on the older phone', 'has a surprisingly simple fix'];
const code = [
  '```rust\nlet widths: Vec<_> = nodes.iter()\n    .map(|node| node.width())\n    .collect();\nassert_eq!(widths.len(), nodes.len());\n```',
  '```swift\n@MainActor\nfunc scrollToLatest() {\n    collectionView.layoutIfNeeded()\n    collectionView.scrollToItem(at: last, at: .bottom, animated: true)\n}\n```',
  '```css\n.transcript {\n  display: flex;\n  flex-direction: column;\n  min-height: 0;\n}\n```',
  '```ts\nconst next = rows.map(row =>\n  row.id === changed.id ? changed : row\n);\nreturn { rows: next };\n```',
  '```rust\nfn clamp_width(width: f32, available: f32) -> f32 {\n    width.max(0.0).min(available)\n}\n```',
  '```sh\ncargo test --tests --no-fail-fast\nbun test ./apps/excord\n```',
];
const rich = [
  '**Small win:** the *whole* list stays still now. __Finally.__ ~~Probably fixed~~ tested on device.',
  'The trick was `min-height: 0`, not another wrapper. ||I spent two hours on this.||',
  '> Does it still work with large text?\n> And after rotating?\nYes to both, just checked.',
  '>>> Notes from the ride\nThe coast road is open.\nBring a layer; it gets cold after the climb.',
  '- Reproduce the jump\n- Measure the old anchor\n  - Keep its row key\n  - Keep the offset\n- Resize and compare',
  '1. Build the smallest example\n2. Run on a device\n  1. Rotate once\n  2. Open the keyboard\n3. Write down what changed',
  '# Weekend ride\nMeet at the ferry building.\n## Route\nCoast, coffee, home.\n-# Weather permitting.',
  '[The Rust book](https://doc.rust-lang.org/book/) has a nice explanation of this.\nAlso https://www.swift.org/documentation/',
  'Thanks <@101>! I moved the reproduction to <#help>. <@&201> might have ideas.',
  'The <@&202> group is meeting <t:1792692000:R>; local time <t:1792692000:t>.',
  '📅 <t:1792692000:D> at <t:1792692000:T> — short date <t:1792692000:d>.',
  'Next demo: <t:1792692000:F>. Calendar entry: <t:1792692000:f>.',
  '<:ferris:501> approves this borrow. <:exact:500> looks sharp.',
  '🎉 <:party:502> 🚲', '🔥 🔥', '<:ferris:501> <:heart:507>',
  'Two things I noticed:\nThe first row holds its place.\nThe image can finish decoding without moving it.',
  '- Capture the offset\n\n  Keep the stable key too.\n- Apply the new window',
];

export function attachment(n: number): Attachment {
  const photo = photos[n % photos.length];
  const size = n % 10 === 9 ? {} : n % 5 === 0
    ? { width: photo.height, height: photo.width * 2 }
    : { width: photo.width, height: photo.height };
  return { id: `photo-${n}`, url: `assets/media/${photo.name}.png`, filename: `${photo.name}.png`,
    ...size, spoiler: n % 13 === 0 };
}

export function linkEmbed(n: number): Embed {
  const photo = photos[n % photos.length];
  const image = { url: `assets/media/${photo.name}.png`, width: photo.width, height: photo.height };
  const variants = [
    { provider: { name: 'GitHub' }, title: 'exact: keep the visible row anchored', url: 'https://github.com/exact',
      description: '**Merged** a smaller layout pass.\n- Preserve row identity\n- Test edits above the viewport', color: 0x5865f2 },
    { provider: { name: 'YouTube' }, title: 'A quiet ride along the coast', url: 'https://www.youtube.com/',
      description: 'Thirty kilometres, a headwind, and a *very* good bakery.', color: 0xef4444 },
    { provider: { name: 'Exact Journal' }, title: 'What makes scrolling feel right?', url: 'https://example.com/scrolling',
      description: 'The visible row is the reference. `offset` is part of the answer.', color: 0x2f9e78 },
  ];
  return { type: 'rich', ...variants[n % variants.length], author: { name: n % 2 ? 'Maya Patel' : 'Buildkite' },
    ...(n % 3 === 0 ? { thumbnail: image } : { image }) };
}

// `deleted`: the ids of messages that replies point at but that were deleted before the history begins.
export function generate(channel: string): { messages: Message[]; lastRead: string; deleted: string[] } {
  const spec = channelSpecs.find(c => c.id === channel);
  if (!spec) throw new Error(`Unknown channel: ${channel}`);
  const rng = random(spec.seed);
  const pick = <T>(values: T[]): T => values[Math.floor(rng() * values.length)];
  const times: number[] = [];
  let elapsed = 0, remaining = 0;
  for (let i = 0; i < spec.count; i++) {
    if (!remaining) {
      elapsed += (2 + rng() * 28) * 3600000;
      remaining = 3 + Math.floor(rng() * 38);
    } else elapsed += (0.25 + rng() * 9) * 60000;
    times.push(elapsed);
    remaining--;
  }
  const end = WORLD_NOW - 5 * 60000;
  const scale = spec.days * 86400000 / (elapsed - times[0]);
  const messages: Message[] = [];
  const deleted: string[] = [];
  let author = members[1], imageNumber = 0;
  for (let i = 0; i < spec.count; i++) {
    const at = Math.round(end - (elapsed - times[i]) * scale);
    if (rng() > 0.44) author = members[Math.floor(rng() * 13)];
    let content = rng() < 0.6 ? pick(chat) : `The ${pick(subjects)} ${pick(observations)}.`;
    const mode = rng();
    if (channel === 'help' && mode < 0.65) content = `${content}\n${pick(code)}`;
    else if (mode < 0.16) content = pick(rich);
    else if (mode < 0.22) content += `\n${pick(code)}`;
    if (i % 163 === 0) content += '\n<@me> could you check this on iOS?';
    if (i % 997 === 0) content = '@everyone the new build is ready for a look.';
    // The tail has exactly one mention, so the unread badge is predictable.
    if (channel === 'general' && i >= spec.count - 37) {
      content = i === spec.count - 21 ? '<@me> the anchor fix is ready to try.' : content.replace(/<@me>|@everyone|@here/g, 'folks');
    }
    const msg: Message = { id: idAt(at, i + 1), channel_id: channel, author, content, timestamp: at,
      edited_timestamp: i % 71 === 0 ? at + 60000 : null, attachments: [], embeds: [], reactions: [],
      type: 0, state: 'sent', revision: 0 };
    if (rng() < (channel === 'random' ? 0.65 : 0.08)) msg.attachments.push(attachment(imageNumber++));
    // Some of #random's photos come in twos, threes and fours, which the transcript tiles.
    if (channel === 'random' && msg.attachments.length && i % 9 === 4) {
      for (let extra = 1 + Math.floor(i / 9) % 3; extra > 0; extra--) msg.attachments.push(attachment(imageNumber++));
    }
    if (channel === 'announcements' || i % 47 === 0) {
      msg.author = members[13 + i % 2];
      msg.embeds.push(linkEmbed(i));
      if (channel === 'announcements') msg.content = `# ${i % 2 ? 'Release notes' : 'Community update'}\n${content}`;
    }
    if (i > 5 && i % 11 === 0) {
      const target = i % 55 === 0 ? Math.max(0, i - 3200) : i - 3;
      const gone = idAt(at - 1, 99999999);
      if (i % 121 === 0) deleted.push(gone);
      msg.message_reference = { message_id: i % 121 === 0 ? gone : messages[target].id };
      msg.type = 19;
    }
    if (i % 7 === 0) msg.reactions.push({ emoji: { id: null, name: pick(['👍', '🎉', '❤️', '👀', '🚲']) }, count: 1 + i % 12, me: i % 3 === 0 });
    if (i % 29 === 0) msg.reactions.push({ emoji: { id: '501', name: 'ferris' }, count: 1 + i % 12, me: false });
    if (i % 863 === 1) { msg.type = 7; msg.content = ''; msg.message_reference = undefined; }
    messages.push(msg);
  }
  // #random ends with a sticker (Discord draws a PNG one as a picture) after one that is a Lottie animation.
  if (channel === 'random' && messages.length > 1) {
    messages[messages.length - 2].sticker_items = [{ id: '602', name: 'Party Parrot', format_type: 3 }];
    // And, before those, a forward of a message from elsewhere: the latest with no reply, picture or link of its own.
    const plainOne = messages.slice(0, -2).findLast(m => !m.message_reference && !m.attachments.length && !m.embeds.length && m.type === 0);
    if (plainOne) plainOne.forwarded = 'Shipping the new **anchor** fix tonight; ping me if anything regresses.';
    // And before it a poll, open for two more days, that the reader has not voted in.
    const asked = messages.slice(0, -2).findLast(m => m !== plainOne && !m.message_reference && !m.attachments.length
      && !m.embeds.length && m.type === 0 && m.author.id !== 'me');
    if (asked) Object.assign(asked, { content: '', poll: {
      question: { text: 'Where should the team offsite be?' },
      answers: [
        { answer_id: 1, poll_media: { text: 'Mountains', emoji: { id: null, name: '🏔️' } } },
        { answer_id: 2, poll_media: { text: 'The beach', emoji: { id: null, name: '🏖️' } } },
        { answer_id: 3, poll_media: { text: 'Stay in the city' } },
      ],
      expiry: new Date(WORLD_NOW + 2 * 86400000).toISOString(), allow_multiselect: false, layout_type: 1,
      results: { is_finalized: false, answer_counts: [{ id: 1, count: 5, me_voted: false }, { id: 2, count: 3, me_voted: false }] },
    } });
    // And before that, an app's answer to a member's command, and a thread started.
    const plain = (m: Message) => m !== plainOne && m !== asked && !m.message_reference && !m.attachments.length
      && !m.embeds.length && m.type === 0 && m.author.id !== 'me';
    const answered = messages.slice(0, -2).findLast(plain);
    if (answered) Object.assign(answered, { author: members[13], type: 20, content: 'Deployed **main** to staging.',
      command: { name: 'deploy', by: '101' }, embeds: [{ type: 'rich', title: 'Deploy #482', color: 0x2f9e78,
        fields: [{ name: 'Branch', value: '`main`', inline: true }, { name: 'Duration', value: '2m 14s', inline: true },
          { name: 'Status', value: 'Passed', inline: true }, { name: 'Changes', value: '- Keep the visible row anchored\n- A faster first paint' }],
        footer: { text: 'Buildkite' }, timestamp: new Date(answered.timestamp).toISOString() }] });
    const thread = messages.slice(0, -2).findLast(m => plain(m) && m !== answered);
    if (thread) Object.assign(thread, { type: 18, content: 'Weekend ride plans' });
    messages[messages.length - 1].sticker_items = [{ id: '601', name: 'Wave', format_type: 1 }];
  }
  return { messages, lastRead: messages[spec.count - spec.unread - 1]?.id ?? '', deleted };
}
