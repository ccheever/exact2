// Heavy list benchmark, Expo port (see ../SPEC.md and README.md here).
// The most ordinary @expo/ui: one Host, a plain List, List.ForEach with data / keyExtractor /
// a row render function (recycling on, the default). Images are expo-image views hosted
// inside the SwiftUI tree with RNHostView, because @expo/ui's own Image only decodes a file
// synchronously on the main thread at full size (README: gaps).
import {
  Button,
  Divider,
  Host,
  HStack,
  List,
  Overlay,
  Rectangle,
  RNHostView,
  Spacer,
  Text,
  VStack,
  ZStack,
} from '@expo/ui/swift-ui';
import {
  accessibilityElement,
  accessibilityLabel,
  alignmentGuide,
  background,
  buttonStyle,
  clipShape,
  fixedSize,
  font,
  foregroundStyle,
  frame,
  italic,
  lineLimit,
  listRowBackground,
  listRowInsets,
  listRowSeparatorTint,
  listStyle,
  padding,
  shapes,
  strokeBorder,
} from '@expo/ui/swift-ui/modifiers';
import { BENCH_LIVE, BENCH_START_INDEX } from './modules/bench-env';
import { Image as ExpoImage } from 'expo-image';
import { useCallback, useEffect, useRef, useState } from 'react';
import { Dimensions } from 'react-native';

import file from './data/messages.json';
import { IMAGES } from './images';

// MARK: - Data

type Run = { t: string; s?: 'bold' | 'italic' | 'code' | 'link' | 'mention' | 'tag' };
type Photo = { src: string; w: number; h: number };
type LinkCard = { thumb: string; title: string; description: string; site: string };
type Quote = { id: string; author: string; excerpt: string };
type Reaction = { emoji: string; count: number };
type Message = {
  id: string;
  index: number;
  author: string;
  avatar: string;
  minutesAgo: number;
  paragraphs: Run[][];
  photos?: Photo[];
  link?: LinkCard;
  quote?: Quote;
  reactions?: Reaction[];
  /** Seconds after launch at which `minutesAgo` was true (live inserts). */
  bornAt?: number;
};

const MESSAGES = (file as { version: number; messages: Message[] }).messages;
const keyExtractor = (m: Message) => m.id;

// MARK: - Colors and sizes

const HAIRLINE = '#E5E5EA';
const SECONDARY = '#8E8E93';
const QUOTE_BAR = '#C7C7CC';
const FILL = '#F2F2F7';
const LABEL2 = '#3C3C43';
const CARD_BORDER = '#D1D1D6';
const LINK_BLUE = '#007AFF';
const TAG_PURPLE = '#5856D6';

// Content column = screen - row padding (16 + 16) - avatar (40) - gap (12).
const CONTENT_WIDTH = Dimensions.get('window').width - 16 - 40 - 12 - 16;

function relativeTime(m: Message, seconds: number): string {
  const minutes = m.minutesAgo + Math.floor((seconds - (m.bornAt ?? 0)) / 60);
  if (minutes < 60) return `${minutes}m ago`;
  if (minutes < 60 * 24) return `${Math.floor(minutes / 60)}h ago`;
  return `${Math.floor(minutes / (60 * 24))}d ago`;
}

// MARK: - App
// BENCH_START_INDEX: @expo/ui's List has no scroll-to-item (see README), so for screenshots the
// list starts AT that message instead: the data is sliced, it is not a scroll. Never set it for
// timing runs.
const START = BENCH_START_INDEX != null && BENCH_START_INDEX > 0 ? Math.min(BENCH_START_INDEX, MESSAGES.length - 1) : 0;

export default function App() {
  const [messages, setMessages] = useState<Message[]>(() => (START ? MESSAGES.slice(START) : MESSAGES));
  const [seconds, setSeconds] = useState(0); // seconds since launch; ticks only in live mode
  // Reaction taps and live bumps: app state keyed by message id + emoji (added to the data's count).
  const [bumps, setBumps] = useState<Readonly<Record<string, number>>>({});

  const bump = useCallback((key: string) => {
    setBumps((previous) => ({ ...previous, [key]: (previous[key] ?? 0) + 1 }));
  }, []);

  // Live mode: every 250 ms one insert at the top and one reaction bump; seconds tick every 4th step.
  const secondsRef = useRef(0);
  useEffect(() => {
    if (!BENCH_LIVE) return;
    let k = 0;
    const timer = setInterval(() => {
      k += 1;
      const base = MESSAGES[(k * 37) % 10_000];
      const copy: Message = {
        ...base,
        id: `live-${k}`,
        index: -k,
        minutesAgo: 0,
        bornAt: secondsRef.current,
      };
      // Gap: nothing holds the viewport across this insert. @expo/ui's only programmatic scroll
      // is scrollPosition(id:), which SwiftUI's List ignores (README), so rows above push the
      // content down as they would in any List that keeps its content offset.
      setMessages((previous) => [copy, ...previous]);

      // Bump original message (k*101) % 10000, or the next one that has reactions.
      let i = (k * 101) % 10_000;
      while (i < MESSAGES.length && !(MESSAGES[i].reactions?.length ?? 0)) i += 1;
      const target = MESSAGES[i];
      if (target?.reactions) {
        bump(`${target.id}|${target.reactions[k % target.reactions.length].emoji}`);
      }
      if (k % 4 === 0) {
        secondsRef.current += 1;
        setSeconds(secondsRef.current);
      }
    }, 250);
    return () => clearInterval(timer);
  }, [bump]);

  const renderItem = useCallback(
    ({ item }: { item: Message }) => (
      <MessageRow message={item} seconds={seconds} bumps={bumps} onBump={bump} />
    ),
    [seconds, bumps, bump]
  );

  return (
    <Host style={{ flex: 1 }} colorScheme="light">
      <VStack spacing={0} modifiers={[background('#FFFFFF')]}>
        <HStack
          modifiers={[padding({ horizontal: 16, vertical: 10 }), background('#FFFFFF')]}>
          <Text modifiers={[font({ size: 17, weight: 'semibold' })]}>Heavy list</Text>
          <Spacer />
          <Text modifiers={[font({ size: 13 }), foregroundStyle(SECONDARY)]}>
            {BENCH_LIVE ? 'Live: on' : 'Live: off'}
          </Text>
        </HStack>
        <Divider />
        <List modifiers={[listStyle('plain')]}>
          <List.ForEach
            data={messages}
            keyExtractor={keyExtractor}
            estimatedItemSize={160}
            modifiers={[
              listRowInsets({ top: 12, leading: 16, bottom: 12, trailing: 16 }),
              listRowSeparatorTint(HAIRLINE),
              listRowBackground('#FFFFFF'),
            ]}>
            {renderItem}
          </List.ForEach>
        </List>
      </VStack>
    </Host>
  );
}

// MARK: - Row

function MessageRow({
  message,
  seconds,
  bumps,
  onBump,
}: {
  message: Message;
  seconds: number;
  bumps: Readonly<Record<string, number>>;
  onBump: (key: string) => void;
}) {
  const time = relativeTime(message, seconds);
  return (
    <HStack
      alignment="top"
      spacing={12}
      modifiers={[
        alignmentGuide('listRowSeparatorLeading', 68 - 16),
        accessibilityElement('contain'),
        accessibilityLabel(`${message.author}, ${time}`),
      ]}>
      <BundleImage
        name={message.avatar}
        width={40}
        height={40}
        modifiers={[clipShape('circle')]}
      />
      <VStack alignment="leading" spacing={0} modifiers={[frame({ maxWidth: Infinity, alignment: 'leading' })]}>
        <HStack alignment="firstTextBaseline" spacing={6} modifiers={[lineLimit(1)]}>
          <Text modifiers={[font({ size: 15, weight: 'semibold' }), foregroundStyle('#000000')]}>
            {message.author}
          </Text>
          <Text modifiers={[font({ size: 13 }), foregroundStyle(SECONDARY)]}>{time}</Text>
        </HStack>

        {message.quote && <QuoteBox quote={message.quote} />}

        <VStack alignment="leading" spacing={8} modifiers={[padding({ top: 6 })]}>
          {message.paragraphs.map((runs, i) => (
            <Paragraph key={i} runs={runs} />
          ))}
        </VStack>

        {message.photos && message.photos.length > 0 && <PhotoGrid photos={message.photos} />}
        {message.link && <LinkCardView link={message.link} />}
        {message.reactions && message.reactions.length > 0 && (
          <Reactions id={message.id} reactions={message.reactions} bumps={bumps} onBump={onBump} />
        )}
      </VStack>
    </HStack>
  );
}

/** One paragraph as one Text: the runs are nested Texts, concatenated into one text flow. */
function Paragraph({ runs }: { runs: Run[] }) {
  return (
    <Text
      modifiers={[
        font({ size: 16 }),
        foregroundStyle('#000000'),
        frame({ maxWidth: Infinity, alignment: 'leading' }),
        fixedSize({ horizontal: false, vertical: true }),
      ]}>
      {runs.map((run, i) => {
        switch (run.s) {
          case 'bold':
            return <Text key={i} modifiers={[font({ size: 16, weight: 'semibold' })]}>{run.t}</Text>;
          case 'italic':
            return <Text key={i} modifiers={[font({ size: 16 }), italic()]}>{run.t}</Text>;
          case 'code':
            // Gap: nested Text takes no background, so the inline #F2F2F7 fill is missing.
            return <Text key={i} modifiers={[font({ size: 15, design: 'monospaced' })]}>{run.t}</Text>;
          case 'link':
            // Gap: nested Text takes no underline, so the link is blue but not underlined.
            return <Text key={i} modifiers={[foregroundStyle(LINK_BLUE)]}>{run.t}</Text>;
          case 'mention':
            return (
              <Text key={i} modifiers={[font({ size: 16, weight: 'semibold' }), foregroundStyle(LINK_BLUE)]}>
                {run.t}
              </Text>
            );
          case 'tag':
            return <Text key={i} modifiers={[foregroundStyle(TAG_PURPLE)]}>{run.t}</Text>;
          default:
            return <Text key={i}>{run.t}</Text>;
        }
      })}
    </Text>
  );
}

function QuoteBox({ quote }: { quote: Quote }) {
  return (
    <Overlay
      alignment="leading"
      modifiers={[clipShape('roundedRectangle', 8), padding({ top: 6 })]}>
      <VStack
        alignment="leading"
        spacing={0}
        modifiers={[
          foregroundStyle(LABEL2),
          frame({ maxWidth: Infinity, alignment: 'leading' }),
          padding({ all: 8 }),
          padding({ leading: 3 }),
          background(FILL),
        ]}>
        <Text modifiers={[font({ size: 13, weight: 'semibold' })]}>{quote.author}</Text>
        <Text modifiers={[font({ size: 13 }), lineLimit(2)]}>{quote.excerpt}</Text>
      </VStack>
      <Overlay.Content>
        <Rectangle modifiers={[foregroundStyle(QUOTE_BAR), frame({ width: 3 })]} />
      </Overlay.Content>
    </Overlay>
  );
}

function PhotoGrid({ photos }: { photos: Photo[] }) {
  const width = CONTENT_WIDTH;
  let grid;
  if (photos.length === 1) {
    const p = photos[0];
    grid = <BundleImage name={p.src} width={width} height={Math.min((width * p.h) / p.w, 320)} />;
  } else if (photos.length === 2) {
    const s = (width - 4) / 2;
    grid = (
      <HStack spacing={4}>
        {photos.map((p, i) => (
          <BundleImage key={i} name={p.src} width={s} height={s} />
        ))}
      </HStack>
    );
  } else if (photos.length === 3) {
    const small = (width - 4) / 3;
    const big = width - 4 - small;
    const h = (big - 4) / 2;
    grid = (
      <HStack spacing={4}>
        <BundleImage name={photos[0].src} width={big} height={big} />
        <VStack spacing={4}>
          <BundleImage name={photos[1].src} width={small} height={h} />
          <BundleImage name={photos[2].src} width={small} height={h} />
        </VStack>
      </HStack>
    );
  } else {
    const s = (width - 4) / 2;
    grid = (
      <VStack spacing={4}>
        <HStack spacing={4}>
          <BundleImage name={photos[0].src} width={s} height={s} />
          <BundleImage name={photos[1].src} width={s} height={s} />
        </HStack>
        <HStack spacing={4}>
          <BundleImage name={photos[2].src} width={s} height={s} />
          <BundleImage name={photos[3].src} width={s} height={s} />
        </HStack>
      </VStack>
    );
  }
  return (
    <ZStack modifiers={[clipShape('roundedRectangle', 12), padding({ top: 8 })]}>{grid}</ZStack>
  );
}

function LinkCardView({ link }: { link: LinkCard }) {
  return (
    <VStack
      alignment="leading"
      spacing={0}
      modifiers={[
        clipShape('roundedRectangle', 12),
        strokeBorder({
          content: CARD_BORDER,
          style: { lineWidth: 0.5 },
          shape: 'roundedRectangle',
          cornerRadius: 12,
        }),
        padding({ top: 8 }),
      ]}>
      <BundleImage name={link.thumb} width={CONTENT_WIDTH} height={140} />
      <VStack
        alignment="leading"
        spacing={0}
        modifiers={[frame({ maxWidth: Infinity, alignment: 'leading' }), padding({ all: 10 })]}>
        <Text modifiers={[font({ size: 12 }), foregroundStyle(SECONDARY)]}>
          {link.site.toUpperCase()}
        </Text>
        <Text modifiers={[font({ size: 15, weight: 'semibold' }), foregroundStyle('#000000'), lineLimit(2)]}>
          {link.title}
        </Text>
        <Text modifiers={[font({ size: 13 }), foregroundStyle(LABEL2), lineLimit(2)]}>
          {link.description}
        </Text>
      </VStack>
    </VStack>
  );
}

// MARK: - Reaction chips
// Gap: @expo/ui has no wrapping layout (SwiftUI's Layout protocol is not exposed), so the chips
// are broken into HStack lines here, greedily, from an estimate of each chip's width.

const EMOJI_WIDTH = 19; // Apple Color Emoji at 14 pt, rounded up
const DIGIT_WIDTH = 8.3; // SF Pro semibold digits at 13 pt, rounded up

function chipWidth(count: number): number {
  return 10 + EMOJI_WIDTH + 4 + String(count).length * DIGIT_WIDTH + 10;
}

function Reactions({
  id,
  reactions,
  bumps,
  onBump,
}: {
  id: string;
  reactions: Reaction[];
  bumps: Readonly<Record<string, number>>;
  onBump: (key: string) => void;
}) {
  const lines: { key: string; emoji: string; count: number }[][] = [];
  let x = 0;
  for (const r of reactions) {
    const key = `${id}|${r.emoji}`;
    const count = r.count + (bumps[key] ?? 0);
    const w = chipWidth(count);
    if (lines.length === 0 || (x > 0 && x + w > CONTENT_WIDTH)) {
      lines.push([]);
      x = 0;
    }
    lines[lines.length - 1].push({ key, emoji: r.emoji, count });
    x += w + 6;
  }
  return (
    <VStack alignment="leading" spacing={6} modifiers={[padding({ top: 8 })]}>
      {lines.map((line, i) => (
        <HStack key={i} spacing={6}>
          {line.map((chip) => (
            <Button key={chip.key} onPress={() => onBump(chip.key)} modifiers={[buttonStyle('borderless')]}>
              <HStack
                spacing={4}
                modifiers={[
                  fixedSize(),
                  padding({ horizontal: 10 }),
                  frame({ height: 28 }),
                  background(FILL, shapes.roundedRectangle({ cornerRadius: 14 })),
                ]}>
                <Text modifiers={[font({ size: 14 })]}>{chip.emoji}</Text>
                <Text modifiers={[font({ size: 13, weight: 'semibold' }), foregroundStyle(LABEL2)]}>
                  {String(chip.count)}
                </Text>
              </HStack>
            </Button>
          ))}
        </HStack>
      ))}
    </VStack>
  );
}

// MARK: - Images
// expo-image (downsampled to the view's size, decoded off the main thread, cached) hosted in the
// SwiftUI row through RNHostView. The #E5E5EA background shows until the decode lands.

function BundleImage({
  name,
  width,
  height,
  modifiers = [],
}: {
  name: string;
  width: number;
  height: number;
  modifiers?: ReturnType<typeof clipShape>[];
}) {
  return (
    <ZStack modifiers={[frame({ width, height }), ...modifiers]}>
      <RNHostView>
        <ExpoImage
          source={IMAGES[name]}
          recyclingKey={name}
          contentFit="cover"
          transition={null}
          style={{ width, height, backgroundColor: HAIRLINE }}
        />
      </RNHostView>
    </ZStack>
  );
}
