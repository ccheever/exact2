// The Skia-drawn kinds: the shader photo, the canvas sketch, and the live card's rings + waveform.
import {
  Canvas, Circle, Fill, Group, Image as SkImage, ImageShader, LinearGradient, Path, Rect, RoundedRect,
  Shader, Skia, Text as SkText, matchFont, rect, rrect, useFont, useImage, vec,
} from '@shopify/react-native-skia';
import { useMemo } from 'react';
import { Platform, StyleSheet, Text, View } from 'react-native';
import { useDerivedValue } from 'react-native-reanimated';

import { IMAGES } from './assets';
import { useC, useLive, useMotion } from './clock';
import { BENCH_FREEZE } from './modules/bench-env';
import type { FeedRow } from './rows';

// MARK: - shader

const WAVE = Skia.RuntimeEffect.Make(`
uniform shader image;
uniform float t;
uniform float2 size;
uniform float3 duoA;
uniform float3 duoB;
half4 main(float2 xy) {
  float d = 0.012 * size.x;
  float sx = clamp(xy.x + d * sin(24.0 * xy.y / size.y + 2.0 * t), 0.5, size.x - 0.5);
  float sy = clamp(xy.y + d * cos(18.0 * xy.x / size.x + 1.6 * t), 0.5, size.y - 0.5);
  half4 c = image.eval(float2(sx, sy));
  float l = 0.2126 * c.r + 0.7152 * c.g + 0.0722 * c.b;
  half3 duo = half3(mix(duoA, duoB, l));
  return half4(mix(c.rgb, duo, 0.7), 1.0);
}`)!;

function rgb(hex: string) {
  return [parseInt(hex.slice(1, 3), 16) / 255, parseInt(hex.slice(3, 5), 16) / 255, parseInt(hex.slice(5, 7), 16) / 255];
}

export function ShaderPhoto({ row }: { row: FeedRow }) {
  const C = useC();
  const H = Math.round((C * 9) / 16);
  const t = useMotion();
  const image = useImage(IMAGES[row.photo.src]);
  const a = rgb(row.duoA), b = rgb(row.duoB);
  const uniforms = useDerivedValue(() => ({ t: BENCH_FREEZE ? 1.25 : t.value, size: [C, H], duoA: a, duoB: b }));
  return (
    <>
      <Text style={styles.caption}>{row.caption}</Text>
      <View style={{ width: C, height: H, borderRadius: 12, overflow: 'hidden', backgroundColor: '#E5E5EA' }}>
        {image && (
          <Canvas style={{ width: C, height: H }}>
            <Fill>
              <Shader source={WAVE} uniforms={uniforms}>
                <ImageShader image={image} fit="cover" rect={rect(0, 0, C, H)} />
              </Shader>
            </Fill>
          </Canvas>
        )}
      </View>
    </>
  );
}

// MARK: - canvas

// Web build (the web pair, ../web): CanvasKit has no system font manager (matchFont throws on React Native
// Web), so the web canvas title uses the bundled Inter Regular at 20 through useFont, Skia's documented web path.
const nativeTitleFont = Platform.OS === 'web' ? null : matchFont({ fontFamily: 'System', fontSize: 20, fontWeight: '600' });
const WEB_TITLE = Platform.OS === 'web' ? require('./data/fonts/Inter-Regular.ttf') : null;

export function SketchCanvas({ row }: { row: FeedRow }) {
  const C = useC();
  const W = C, H = 220;
  const image = useImage(IMAGES[row.image]);
  const strokes = useMemo(() => row.strokes.map((s: { p: number[]; color: string; width: number }) => {
    const p = Skia.Path.Make();
    p.moveTo(s.p[0] * W, s.p[1] * H);
    p.cubicTo(s.p[2] * W, s.p[3] * H, s.p[4] * W, s.p[5] * H, s.p[6] * W, s.p[7] * H);
    return { path: p, color: s.color, width: s.width };
  }), [row.id, W]);
  const clip = useMemo(() => { const p = Skia.Path.Make(); p.addCircle(W - 48, 48, 32); return p; }, [W]);
  const webTitleFont = useFont(WEB_TITLE, 20);
  const titleFont = nativeTitleFont ?? webTitleFont;
  const ascent = titleFont ? -titleFont.getMetrics().ascent : 0;
  return (
    <View style={{ width: W, height: H, borderRadius: 12, overflow: 'hidden' }}>
      <Canvas style={{ width: W, height: H }}>
        <Fill color={row.bg} />
        <RoundedRect x={16} y={H - 60} width={0.4 * W} height={44} r={10}>
          <LinearGradient start={vec(16, 0)} end={vec(16 + 0.4 * W, 0)} colors={row.band} />
        </RoundedRect>
        {strokes.map((s: { path: any; color: string; width: number }, i: number) => (
          <Path key={i} path={s.path} style="stroke" strokeWidth={s.width} strokeCap="round" strokeJoin="round" color={s.color} />
        ))}
        {row.dots.map((d: { x: number; y: number; r: number; color: string }, i: number) => (
          <Circle key={i} cx={d.x * W} cy={d.y * H} r={d.r} color={d.color} opacity={0.6} />
        ))}
        {image && (
          <Group clip={clip}>
            <SkImage image={image} x={W - 80} y={16} width={64} height={64} fit="cover" />
          </Group>
        )}
        {titleFont && <SkText x={16} y={16 + ascent} text={row.title} font={titleFont} color="#1C1C1E" />}
      </Canvas>
    </View>
  );
}

// MARK: - live card

const RING_COLORS = ['#FF3B30', '#34C759', '#007AFF'];
const RING_D = 56, RING_W = 6;

function Ring({ base, i }: { base: number; i: number }) {
  const t = useMotion();
  const arc = useMemo(() => {
    const p = Skia.Path.Make();
    p.addArc({ x: RING_W / 2, y: RING_W / 2, width: RING_D - RING_W, height: RING_D - RING_W }, -90, 359.999);
    return p;
  }, []);
  const end = useDerivedValue(() =>
    BENCH_FREEZE ? base : Math.min(1, Math.max(0, base + 0.2 * Math.sin(2 * Math.PI * (t.value / 4 + i / 3)))));
  return (
    <View style={{ width: RING_D, height: RING_D, alignItems: 'center', justifyContent: 'center' }}>
      <Canvas style={StyleSheet.absoluteFill}>
        <Circle cx={RING_D / 2} cy={RING_D / 2} r={(RING_D - RING_W) / 2} style="stroke" strokeWidth={RING_W} color="#E5E5EA" />
        <Path path={arc} style="stroke" strokeWidth={RING_W} strokeCap="round" color={RING_COLORS[i]} end={end} />
      </Canvas>
      <Text style={styles.ringLabel}>{`${Math.round(base * 100)}%`}</Text>
    </View>
  );
}

function Waveform({ wave }: { wave: number[] }) {
  const t = useMotion();
  const bars = useMemo(() => {
    const p = Skia.Path.Make();
    wave.forEach((h, j) => p.addRRect(rrect(rect(j * 5, (40 - h) / 2, 3, h), 1.5, 1.5)));
    return p;
  }, [wave]);
  const played = useDerivedValue(() => {
    const q = BENCH_FREEZE ? 0.4 : (t.value % 12) / 12;
    return rect(0, 0, Math.ceil(48 * q) * 5, 40);
  });
  return (
    <Canvas style={{ width: 238, height: 40 }}>
      <Path path={bars} color="#C7C7CC" />
      <Group clip={played}>
        <Path path={bars} color="#007AFF" />
      </Group>
    </Canvas>
  );
}

const pad2 = (n: number) => String(n).padStart(2, '0');

export function LiveCard({ row }: { row: FeedRow }) {
  const s = useLive();
  const left = Math.max(0, row.endsInSec - s);
  const hms = `${pad2(Math.floor(left / 3600))}:${pad2(Math.floor((left % 3600) / 60))}:${pad2(left % 60)}`;
  return (
    <View style={styles.card}>
      <View style={styles.cardHead}>
        <Text style={styles.bold15}>{row.title}</Text>
        <Text style={styles.updated}>{`Updated ${(row.updatedSec + s) % 60}s ago`}</Text>
      </View>
      <Text style={{ marginTop: 10 }}>
        <Text style={styles.endsIn}>Ends in </Text>
        <Text style={styles.countdown}>{hms}</Text>
      </Text>
      <View style={{ flexDirection: 'row', gap: 16, marginTop: 12 }}>
        {row.rings.map((b: number, i: number) => <Ring key={i} base={b} i={i} />)}
      </View>
      <View style={{ flexDirection: 'row', alignItems: 'center', gap: 10, marginTop: 12 }}>
        <View style={styles.play}><View style={styles.triangle} /></View>
        <Waveform wave={row.wave} />
        <Text style={styles.time}>{`0:${pad2(s % 12)} / 0:12`}</Text>
      </View>
    </View>
  );
}

const styles = StyleSheet.create({
  caption: { fontSize: 15, color: '#000' },
  bold15: { fontSize: 15, fontWeight: '600', color: '#000' },
  card: { borderWidth: 0.5, borderColor: '#D1D1D6', borderRadius: 12, padding: 14 },
  cardHead: { flexDirection: 'row', justifyContent: 'space-between', alignItems: 'center' },
  updated: { fontSize: 12, color: '#8E8E93' },
  endsIn: { fontSize: 15, color: '#3C3C43' },
  countdown: { fontSize: 22, fontWeight: '600', color: '#000', fontVariant: ['tabular-nums'] },
  ringLabel: { fontSize: 12, fontWeight: '600', color: '#000' },
  play: { width: 32, height: 32, borderRadius: 16, backgroundColor: '#007AFF', alignItems: 'center', justifyContent: 'center' },
  triangle: { width: 0, height: 0, borderTopWidth: 6, borderBottomWidth: 6, borderLeftWidth: 10, borderTopColor: 'transparent', borderBottomColor: 'transparent', borderLeftColor: '#fff' },
  time: { fontSize: 12, color: '#8E8E93', fontVariant: ['tabular-nums'] },
});
