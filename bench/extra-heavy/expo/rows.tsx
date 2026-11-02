// The row: header + one body per kind (SPEC "Row kinds").
import { LegendList, useRecyclingState } from '@legendapp/list/react-native';
import { BlurView } from 'expo-blur';
import { Image } from 'expo-image';
import { LinearGradient } from 'expo-linear-gradient';
import { useVideoPlayer, VideoView } from 'expo-video';
import LottieView from 'lottie-react-native';
import { memo, useEffect, useLayoutEffect, useState } from 'react';
import { LayoutAnimation, Pressable, StyleSheet, Text, TextInput, View } from 'react-native';
import MapView, { Marker } from 'react-native-maps';
import Markdown from 'react-native-markdown-display';
import Animated, { Easing, useAnimatedStyle, useSharedValue, withRepeat, withTiming } from 'react-native-reanimated';
import { SvgXml } from 'react-native-svg';
import { WebView } from 'react-native-webview';

import { ANIM, IMAGES, LOTTIE, VIDEO } from './assets';
import { useC, useLive } from './clock';
import { EMBED_HTML } from './embed';
import { BENCH_FREEZE, BENCH_TICKS } from './modules/bench-env';
import { Filmstrip, Inbox } from './nested';
import { LiveCard, ShaderPhoto, SketchCanvas } from './skia';
import file from './data/feed.json';
import SVGS from './data/svgs.json';

export type FeedRow = { id: string; index: number; kind: string; author: string; handle: string; avatar: string; minutesAgo: number; [k: string]: any };
const FONTS = (file as unknown as { fonts: Record<string, { name: string; size: number; family: string }> }).fonts;
const SVG = SVGS as Record<string, string>;

// MARK: - Header

function rel(minutesAgo: number, s: number) {
  const m = minutesAgo + Math.floor(s / 60);
  if (m === 0) return 'now';
  if (m < 60) return `${m}m`;
  if (m < 1440) return `${Math.floor(m / 60)}h`;
  return `${Math.floor(m / 1440)}d`;
}

function Header({ row }: { row: FeedRow }) {
  const s = useLive();
  return (
    <View style={st.header}>
      <Image source={IMAGES[row.avatar]} style={st.avatar} contentFit="cover" recyclingKey={row.id} />
      <View style={st.headerText}>
        <Text style={st.author} numberOfLines={1}>{row.author}</Text>
        <Text style={st.handle} numberOfLines={1}>{`${row.handle} · ${rel(row.minutesAgo, s)}`}</Text>
      </View>
      <Text style={st.kind}>{row.kind.toUpperCase()}</Text>
    </View>
  );
}

// MARK: - Kinds

function Photo({ row }: { row: FeedRow }) {
  const C = useC();
  const p = row.photo;
  return (
    <>
      <Text style={st.caption}>{row.caption}</Text>
      <Image source={IMAGES[p.src]} recyclingKey={row.id} contentFit="cover"
        style={{ width: C, height: Math.min((C * p.h) / p.w, 1.25 * C), borderRadius: 12, backgroundColor: '#E5E5EA' }} />
    </>
  );
}

function Shimmer({ size, x, phase }: { size: number; x: number; phase: ReturnType<typeof useSharedValue<number>> }) {
  const G = size * 4 + 12;
  const band = 0.4 * G;
  const style = useAnimatedStyle(() => ({ transform: [{ translateX: -band + 1.4 * G * phase.value - x }] }));
  return (
    <View style={[StyleSheet.absoluteFill, { backgroundColor: '#E5E5EA' }]}>
      <Animated.View style={[{ position: 'absolute', top: 0, bottom: 0, left: 0, width: band }, style]}>
        <LinearGradient style={StyleSheet.absoluteFill} start={{ x: 0, y: 0.5 }} end={{ x: 1, y: 0.5 }}
          colors={['rgba(255,255,255,0)', 'rgba(255,255,255,0.65)', 'rgba(255,255,255,0)']} />
      </Animated.View>
    </View>
  );
}

function Thumbs({ row }: { row: FeedRow }) {
  const C = useC();
  const side = (C - 12) / 4;
  const [loading, setLoading] = useState(!BENCH_FREEZE);
  const phase = useSharedValue(0);
  const fade = useSharedValue(BENCH_FREEZE ? 1 : 0);
  useLayoutEffect(() => {
    if (BENCH_FREEZE) return;
    setLoading(true);
    fade.value = 0;
    phase.value = 0;
    phase.value = withRepeat(withTiming(1, { duration: 1000, easing: Easing.linear }), -1, false);
    const id = setTimeout(() => {
      setLoading(false);
      fade.value = withTiming(1, { duration: 200 });
    }, 1200);
    return () => clearTimeout(id);
  }, [row.id]);
  const fadeStyle = useAnimatedStyle(() => ({ opacity: fade.value }));
  return (
    <>
      <Text style={st.bold15}>{row.title}</Text>
      <View style={[st.grid, { width: C }]}>
        {row.thumbs.map((src: string, k: number) => (
          <View key={k} style={{ width: side, height: side, borderRadius: 8, overflow: 'hidden' }}>
            <Animated.View style={[StyleSheet.absoluteFill, fadeStyle]}>
              <Image source={IMAGES[src]} recyclingKey={`${row.id}-${k}`} contentFit="cover" style={StyleSheet.absoluteFill} />
            </Animated.View>
            {loading && <Shimmer size={side} x={(k % 4) * (side + 4)} phase={phase} />}
          </View>
        ))}
      </View>
    </>
  );
}

function Svgs({ row }: { row: FeedRow }) {
  const C = useC();
  return (
    <View style={{ gap: 12 }}>
      <View style={{ flexDirection: 'row', gap: 16 }}>
        {row.icons.map((n: string) => <SvgXml key={n} xml={SVG['icon-' + n]} width={28} height={28} />)}
      </View>
      <SvgXml xml={SVG[row.chart]} width={C} height={(C * 160) / 600} />
      <View style={{ width: C, height: C / 2, borderRadius: 12, overflow: 'hidden' }}>
        <SvgXml xml={SVG[row.art]} width={C} height={C / 2} />
      </View>
    </View>
  );
}

function Video({ row }: { row: FeedRow }) {
  const C = useC();
  const player = useVideoPlayer(VIDEO[row.video], (p) => {
    p.loop = true;
    p.muted = true;
    if (!BENCH_FREEZE) p.play();
  });
  return (
    <>
      <Text style={st.caption}>{row.caption}</Text>
      <View style={{ width: C, height: Math.round((C * 9) / 16), borderRadius: 12, overflow: 'hidden', backgroundColor: '#000' }}>
        <VideoView player={player} style={StyleSheet.absoluteFill} contentFit="cover" nativeControls={false} />
        <View style={st.pill}><Text style={st.pillText}>0:04</Text></View>
      </View>
    </>
  );
}

function MapRow({ row }: { row: FeedRow }) {
  const C = useC();
  const region = { latitude: row.lat, longitude: row.lon, latitudeDelta: 0.02, longitudeDelta: 0.02 };
  return (
    <>
      <View style={{ gap: 2 }}>
        <Text style={st.bold15}>{row.place}</Text>
        <Text style={st.sub13}>{row.address}</Text>
      </View>
      <View style={{ width: C, height: 200, borderRadius: 12, overflow: 'hidden' }} pointerEvents="none">
        <MapView style={StyleSheet.absoluteFill} region={region} scrollEnabled={false} zoomEnabled={false}
          rotateEnabled={false} pitchEnabled={false} toolbarEnabled={false}>
          <Marker coordinate={{ latitude: row.lat, longitude: row.lon }} title={row.place} />
        </MapView>
      </View>
    </>
  );
}

const md = StyleSheet.create({
  body: { fontSize: 16, color: '#000', gap: 8 },
  heading2: { fontSize: 20, fontWeight: '600', color: '#000', marginTop: 0, marginBottom: 0 },
  paragraph: { marginTop: 0, marginBottom: 0, fontSize: 16 },
  strong: { fontWeight: '700' },
  em: { fontStyle: 'italic' },
  code_inline: { fontFamily: 'SpaceMono-Regular', fontSize: 14, backgroundColor: '#F2F2F7', borderWidth: 0, padding: 0 },
  link: { color: '#007AFF', textDecorationLine: 'none' },
  bullet_list: { gap: 4 },
  list_item: { flexDirection: 'row' },
  bullet_list_icon: { marginLeft: 0, marginRight: 8, fontSize: 16, color: '#000' },
  bullet_list_content: { flex: 1 },
  blockquote: { backgroundColor: 'transparent', borderLeftWidth: 3, borderLeftColor: '#C7C7CC', paddingHorizontal: 10, marginLeft: 0 },
});
function plain(node: any): string {
  return (node.content ?? '') + (node.children ?? []).map(plain).join('');
}
const mdRules = {
  // SPEC: the quote is 16 italic #3C3C43 behind a 3 pt bar; the library's default renders body text.
  blockquote: (node: any, _children: any, _parent: any, styles: any) => (
    <View key={node.key} style={styles.blockquote}>
      <Text style={{ fontSize: 16, fontStyle: 'italic', color: '#3C3C43' }}>{plain(node)}</Text>
    </View>
  ),
  // SPEC: "•" then 8 gap (the library's default bullet is "·").
  list_item: (node: any, children: any, _parent: any, styles: any) => (
    <View key={node.key} style={styles._VIEW_SAFE_list_item}>
      <Text style={styles.bullet_list_icon}>•</Text>
      <View style={styles._VIEW_SAFE_bullet_list_content}>{children}</View>
    </View>
  ),
};
function MarkdownRow({ row }: { row: FeedRow }) {
  return <Markdown style={md} rules={mdRules}>{row.md}</Markdown>;
}

const TOKEN: Record<string, string> = { keyword: '#FF7B72', string: '#A5D6FF', number: '#79C0FF', comment: '#8B949E', function: '#D2A8FF', type: '#FFA657', plain: '#E6EDF3' };
const LANG: Record<string, string> = { ts: 'TS', rs: 'RUST', py: 'PYTHON' };
function Code({ row }: { row: FeedRow }) {
  return (
    <View style={st.code}>
      <View style={st.codeHead}>
        <Text style={st.codeFile}>{row.file}</Text>
        <Text style={st.codeLang}>{LANG[row.lang]}</Text>
      </View>
      {row.lines.map((toks: { t: string; k: string }[], i: number) => (
        <View key={i} style={st.codeLine}>
          <Text style={st.codeNum}>{i + 1}</Text>
          <Text style={st.codeText} numberOfLines={1} ellipsizeMode="clip">
            {toks.map((tk, j) => <Text key={j} style={{ color: TOKEN[tk.k] }}>{tk.t}</Text>)}
          </Text>
        </View>
      ))}
    </View>
  );
}

function Intl({ row }: { row: FeedRow }) {
  return (
    <View style={{ gap: 8 }}>
      {row.blocks.map((b: { text: string; dir: string }, i: number) => (
        <Text key={i} style={[st.intl, b.dir === 'rtl' ? { writingDirection: 'rtl', textAlign: 'right' } : { writingDirection: 'ltr', textAlign: 'left' }]}>{b.text}</Text>
      ))}
    </View>
  );
}

function Typeface({ row }: { row: FeedRow }) {
  const f = FONTS[row.font];
  return (
    <View style={{ backgroundColor: row.bg, borderRadius: 12, padding: 20, gap: 10 }}>
      <Text style={{ fontFamily: f.name, fontSize: f.size, color: '#1C1C1E' }}>{row.quote}</Text>
      <Text style={st.label13}>{`— ${row.by} · ${f.family}`}</Text>
    </View>
  );
}

type Card = { image: string; title: string; meta: string };
const renderCard = ({ item }: { item: Card }) => (
  <View style={{ width: 140 }}>
    <Image source={IMAGES[item.image]} contentFit="cover" style={{ width: 140, height: 100, borderRadius: 10 }} />
    <View style={{ height: 34, marginTop: 6 }}>
      <Text style={{ fontSize: 13, fontWeight: '600', color: '#000' }} numberOfLines={2}>{item.title}</Text>
    </View>
    <Text style={{ fontSize: 12, color: '#8E8E93', marginTop: 4 }} numberOfLines={1}>{item.meta}</Text>
  </View>
);
const cardSize = () => 140;
function Carousel({ row }: { row: FeedRow }) {
  const C = useC();
  // dataKey: a recycled row is a new dataset, so the strip starts again at its first card.
  return (
    <>
      <Text style={st.bold17}>{row.title}</Text>
      <View style={{ width: C, height: 170 }}>
        <LegendList horizontal data={row.cards as Card[]} dataKey={row.id} renderItem={renderCard}
          recycleItems getFixedItemSize={cardSize} contentContainerStyle={{ gap: 10 }}
          showsHorizontalScrollIndicator={false} />
      </View>
    </>
  );
}

function Motion({ row }: { row: FeedRow }) {
  const C = useC();
  const T = (C - 24) / 3;
  const tile = { width: T, height: T, borderRadius: 12, backgroundColor: '#F2F2F7', overflow: 'hidden' as const };
  return (
    <>
      <Text style={st.caption}>{row.caption}</Text>
      <View style={{ flexDirection: 'row', gap: 12 }}>
        <View style={{ gap: 4 }}>
          <Image source={ANIM[row.gif]} contentFit="contain" autoplay={!BENCH_FREEZE} recyclingKey={row.id + 'g'} style={tile} />
          <Text style={st.tileLabel}>GIF</Text>
        </View>
        <View style={{ gap: 4 }}>
          <Image source={ANIM[row.webp]} contentFit="contain" autoplay={!BENCH_FREEZE} recyclingKey={row.id + 'w'} style={tile} />
          <Text style={st.tileLabel}>WebP</Text>
        </View>
        <View style={{ gap: 4 }}>
          <View style={tile}>
            {BENCH_FREEZE
              ? <LottieView source={LOTTIE[row.lottie]} progress={0.5} style={{ width: T, height: T }} resizeMode="contain" />
              : <LottieView source={LOTTIE[row.lottie]} autoPlay loop style={{ width: T, height: T }} resizeMode="contain" />}
          </View>
          <Text style={st.tileLabel}>Lottie</Text>
        </View>
      </View>
    </>
  );
}

function Glass({ row }: { row: FeedRow }) {
  const C = useC();
  return (
    <View style={{ width: C, height: Math.round((C * 3) / 4), borderRadius: 16, overflow: 'hidden' }}>
      <Image source={IMAGES[row.photo.src]} recyclingKey={row.id} contentFit="cover" style={[StyleSheet.absoluteFill, { backgroundColor: '#E5E5EA' }]} />
      <BlurView tint="systemUltraThinMaterialLight" intensity={100} style={st.glassBar}>
        <Text style={st.glassTitle} numberOfLines={1}>{row.title}</Text>
        <Text style={st.glassSub} numberOfLines={1}>{row.subtitle}</Text>
      </BlurView>
      <BlurView tint="systemUltraThinMaterialLight" intensity={100} style={st.glassPill}>
        <Text style={{ fontSize: 13, fontWeight: '600', color: '#000' }}>{row.rating}</Text>
      </BlurView>
    </View>
  );
}

const threadState = new Map<string, boolean>();
const drafts = new Map<string, string>();
function Thread({ row }: { row: FeedRow }) {
  const s = useLive();
  // useRecyclingState resets when the recycled cell gets another row; its setter re-lays out the cell.
  const [expanded, setExpanded] = useRecyclingState((info: any) => threadState.get((info.item as FeedRow).id) ?? false);
  const [draft, setDraft] = useRecyclingState((info: any) => drafts.get((info.item as FeedRow).id) ?? '');
  const apply = (e: boolean) => {
    if (e === expanded) return;
    LayoutAnimation.configureNext(LayoutAnimation.create(250, 'easeInEaseOut', 'opacity'));
    threadState.set(row.id, e);
    setExpanded(e);
  };
  useEffect(() => {
    if (BENCH_TICKS && s > 0) apply((Math.floor(s / 3) + row.index) % 2 === 1);
  }, [Math.floor(s / 3)]);
  return (
    <>
      <View style={{ gap: 4 }}>
        <Text style={st.caption} numberOfLines={expanded ? undefined : 3}>{row.text}</Text>
        <Pressable onPress={() => apply(!expanded)}>
          <Text style={st.more}>{expanded ? 'Show less' : 'Show more'}</Text>
        </Pressable>
      </View>
      <View style={{ flexDirection: 'row', alignItems: 'center', gap: 8, marginTop: 2 }}>
        <TextInput style={st.input} placeholder="Add a comment…" placeholderTextColor="#8E8E93" value={draft}
          onChangeText={(v) => { drafts.set(row.id, v); setDraft(v); }} />
        <Text style={[st.send, { color: draft ? '#007AFF' : '#C7C7CC' }]}>Send</Text>
      </View>
    </>
  );
}

function Web({ row }: { row: FeedRow }) {
  const C = useC();
  const html = EMBED_HTML.replaceAll('{{HUE}}', String(row.hue)).replace('{{TITLE}}', row.title).replace('{{N}}', String(row.index))
    .replace('{{PLAY}}', BENCH_FREEZE ? 'paused' : 'running')
    .replace('{{BARS}}', row.bars.map((v: number) => `<div class="bar" style="height:${v}%"></div>`).join(''));
  return (
    <>
      <Text style={st.caption}>{row.caption}</Text>
      <View style={{ width: C, height: 220, borderRadius: 12, overflow: 'hidden', borderWidth: 0.5, borderColor: '#D1D1D6' }}>
        <WebView source={{ html }} originWhitelist={['*']} scrollEnabled={false} style={{ flex: 1 }} />
      </View>
    </>
  );
}

function Body({ row }: { row: FeedRow }) {
  switch (row.kind) {
    case 'photo': return <Photo row={row} />;
    case 'thumbs': return <Thumbs row={row} />;
    case 'shader': return <ShaderPhoto row={row} />;
    case 'canvas': return <SketchCanvas row={row} />;
    case 'svg': return <Svgs row={row} />;
    case 'video': return <Video row={row} />;
    case 'map': return <MapRow row={row} />;
    case 'markdown': return <MarkdownRow row={row} />;
    case 'code': return <Code row={row} />;
    case 'intl': return <Intl row={row} />;
    case 'typeface': return <Typeface row={row} />;
    case 'carousel': return <Carousel row={row} />;
    case 'motion': return <Motion row={row} />;
    case 'glass': return <Glass row={row} />;
    case 'live': return <LiveCard row={row} />;
    case 'thread': return <Thread row={row} />;
    case 'webview': return <Web row={row} />;
    case 'filmstrip': return <Filmstrip row={row} />;
    case 'inbox': return <Inbox row={row} />;
  }
  return null;
}

export const Row = memo(function Row({ row }: { row: FeedRow }) {
  const C = useC();
  return (
    <View style={st.row}>
      <View style={{ width: C, gap: 10 }}>
        <Header row={row} />
        <Body row={row} />
      </View>
    </View>
  );
});

const st = StyleSheet.create({
  row: { backgroundColor: '#fff', paddingVertical: 16, alignItems: 'center', borderBottomWidth: 0.5, borderBottomColor: '#E5E5EA' },
  header: { flexDirection: 'row', alignItems: 'center', height: 36 },
  avatar: { width: 36, height: 36, borderRadius: 18, backgroundColor: '#E5E5EA' },
  headerText: { marginLeft: 10, flex: 1, gap: 2 },
  author: { fontSize: 15, fontWeight: '600', color: '#000' },
  handle: { fontSize: 13, color: '#8E8E93' },
  kind: { fontSize: 11, fontWeight: '600', color: '#8E8E93' },
  caption: { fontSize: 15, color: '#000' },
  bold15: { fontSize: 15, fontWeight: '600', color: '#000' },
  bold17: { fontSize: 17, fontWeight: '600', color: '#000' },
  sub13: { fontSize: 13, color: '#8E8E93' },
  label13: { fontSize: 13, color: '#3C3C43' },
  grid: { flexDirection: 'row', flexWrap: 'wrap', gap: 4 },
  pill: { position: 'absolute', left: 8, bottom: 8, backgroundColor: 'rgba(0,0,0,0.55)', paddingVertical: 3, paddingHorizontal: 7, borderRadius: 6 },
  pillText: { fontSize: 12, fontWeight: '600', color: '#fff' },
  code: { backgroundColor: '#0D1117', borderRadius: 12, padding: 12, overflow: 'hidden' },
  codeHead: { flexDirection: 'row', justifyContent: 'space-between', alignItems: 'center', marginBottom: 8 },
  codeFile: { fontFamily: 'SpaceMono-Regular', fontSize: 12, color: '#8B949E' },
  codeLang: { fontSize: 11, fontWeight: '600', color: '#8B949E' },
  codeLine: { flexDirection: 'row', height: 20, alignItems: 'center' },
  codeNum: { width: 24, textAlign: 'right', fontFamily: 'SpaceMono-Regular', fontSize: 13, color: '#6E7681', marginRight: 12 },
  codeText: { flex: 1, fontFamily: 'SpaceMono-Regular', fontSize: 13, lineHeight: 20, color: '#E6EDF3' },
  intl: { fontSize: 17, color: '#000' },
  tileLabel: { fontSize: 11, fontWeight: '600', color: '#8E8E93', textAlign: 'center' },
  glassBar: { position: 'absolute', left: 12, right: 12, bottom: 12, height: 64, borderRadius: 14, overflow: 'hidden', paddingHorizontal: 12, justifyContent: 'center', gap: 2 },
  glassTitle: { fontSize: 16, fontWeight: '600', color: '#000' },
  glassSub: { fontSize: 13, color: '#3C3C43' },
  glassPill: { position: 'absolute', top: 12, right: 12, borderRadius: 999, overflow: 'hidden', paddingVertical: 6, paddingHorizontal: 10 },
  more: { fontSize: 15, fontWeight: '600', color: '#007AFF' },
  input: { flex: 1, height: 40, backgroundColor: '#F2F2F7', borderRadius: 20, paddingHorizontal: 14, fontSize: 15, color: '#000' },
  send: { fontSize: 15, fontWeight: '600' },
});
