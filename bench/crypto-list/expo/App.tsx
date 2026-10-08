// Crypto list benchmark, Expo port (see ../SPEC.md and README.md here).
// The ordinary React Native way: coins in React state (immutable updates), a memoized row,
// LegendList (recycling, fixed 64 pt rows) for the list, a Skia Canvas per row for the chart, Reanimated shared values
// driving the Skia props (draw-in, pulse) and the price colour (flash).
import { Canvas, Circle, Group, Path, Skia } from '@shopify/react-native-skia';
import { LegendList } from '@legendapp/list/react-native';
import { StatusBar } from 'expo-status-bar';
import { memo, useCallback, useEffect, useLayoutEffect, useMemo, useRef, useState } from 'react';
import { StyleSheet, Text, View } from 'react-native';
import Animated, {
  cancelAnimation,
  Easing,
  interpolateColor,
  useAnimatedStyle,
  useDerivedValue,
  useSharedValue,
  withRepeat,
  withTiming,
} from 'react-native-reanimated';
import { SafeAreaProvider, SafeAreaView } from 'react-native-safe-area-context';

import { BENCH_FREEZE, BENCH_START_INDEX, BENCH_TICKS } from './modules/bench-env';
import file from './data/coins.json';

// MARK: - Data

type Coin = {
  id: string;
  name: string;
  ticker: string;
  color: string;
  price: number;
  change24h: number;
  series: number[];
  /** Number of ticks that have touched this coin (0 = never); a change replays the flash. */
  seq?: number;
  /** Direction of the last tick (δ ≥ 0 counts as up). */
  up?: boolean;
};

const COINS = (file as { version: number; coins: Coin[] }).coins;
const keyExtractor = (c: Coin) => c.id;
const rowSize = () => 64; // SPEC: row pitch exactly 64 pt

// MARK: - Colours, sizes, easing

const GREEN = '#16a34a';
const RED = '#dc2626';
const HAIRLINE = '#E5E5EA';
const SECONDARY = '#8E8E93';

const CHART_W = 96;
const CHART_H = 32;
const OUTSET = 9; // the canvas is the chart box outset by 9 pt (the ring may overflow the box)

const EASE_OUT = Easing.bezier(0, 0, 0.58, 1); // CSS ease-out
const EASE = Easing.bezier(0.25, 0.1, 0.25, 1); // CSS ease (the default transition timing)

// MARK: - Formatting

const grouped = new Intl.NumberFormat('en-US', {
  minimumFractionDigits: 2,
  maximumFractionDigits: 2,
});

function formatPrice(p: number): string {
  if (p >= 1) return '$' + grouped.format(p);
  // 4 significant digits, trailing zeros kept, never exponent notation.
  const decimals = 3 - Math.floor(Math.log10(p));
  return '$' + p.toFixed(decimals);
}

function formatChange(c: number): string {
  return (c >= 0 ? '+' : '-') + Math.abs(c).toFixed(2) + '%';
}

// MARK: - Chart

function chartPoints(series: number[]) {
  let min = Infinity;
  let max = -Infinity;
  for (const v of series) {
    if (v < min) min = v;
    if (v > max) max = v;
  }
  const n = series.length - 1;
  return series.map((v, i) => ({
    x: OUTSET + (CHART_W * i) / n,
    y: OUTSET + (max === min ? CHART_H / 2 : CHART_H - (CHART_H * (v - min)) / (max - min)),
  }));
}

const Chart = memo(function Chart({ coinId, series, color }: { coinId: string; series: number[]; color: string }) {
  const { path, last } = useMemo(() => {
    const pts = chartPoints(series);
    const p = Skia.Path.Make();
    p.moveTo(pts[0].x, pts[0].y);
    for (let i = 1; i < pts.length; i++) p.lineTo(pts[i].x, pts[i].y);
    return { path: p, last: pts[pts.length - 1] };
  }, [series]);

  // Draw-in (stroke trim 0 → 1), then the dot and the breathing ring.
  const progress = useSharedValue(BENCH_FREEZE ? 1 : 0);
  const shown = useSharedValue(BENCH_FREEZE ? 1 : 0);
  const pulse = useSharedValue(BENCH_FREEZE ? 0.5 : 0); // 0.5 → radius 6, opacity 0.25

  // Keyed to the coin: runs on mount and whenever a recycled cell gets a different coin,
  // not when a tick changes the series of the coin already shown.
  useLayoutEffect(() => {
    if (BENCH_FREEZE) return;
    cancelAnimation(pulse);
    shown.value = 0;
    pulse.value = 0;
    progress.value = 0;
    progress.value = withTiming(1, { duration: 600, easing: EASE_OUT }, (finished) => {
      if (!finished) return;
      shown.value = 1;
      pulse.value = withRepeat(withTiming(1, { duration: 1200, easing: EASE_OUT }), -1, false);
    });
  }, [coinId]);

  const ringR = useDerivedValue(() => 3 + 6 * pulse.value);
  const ringOpacity = useDerivedValue(() => 0.5 * (1 - pulse.value));

  return (
    <Canvas style={styles.chart}>
      <Path
        path={path}
        style="stroke"
        strokeWidth={1.5}
        strokeJoin="round"
        strokeCap="round"
        color={color}
        end={progress}
      />
      <Group opacity={shown}>
        <Circle cx={last.x} cy={last.y} r={ringR} color={color} opacity={ringOpacity} />
        <Circle cx={last.x} cy={last.y} r={3} color={color} />
      </Group>
    </Canvas>
  );
});

// MARK: - Price (flashes on a tick)

function Price({ coin }: { coin: Coin }) {
  const flash = useSharedValue(0);
  const up = useSharedValue(1);
  const shownId = useRef(coin.id);
  const shownSeq = useRef(coin.seq ?? 0);

  useEffect(() => {
    const seq = coin.seq ?? 0;
    if (shownId.current !== coin.id) {
      // A recycled cell showing another coin: no flash carried over.
      shownId.current = coin.id;
      shownSeq.current = seq;
      cancelAnimation(flash);
      flash.value = 0;
      return;
    }
    if (seq !== shownSeq.current) {
      shownSeq.current = seq;
      up.value = coin.up === false ? 0 : 1;
      flash.value = 1;
      flash.value = withTiming(0, { duration: 400, easing: EASE });
    }
  }, [coin.id, coin.seq]);

  const colorStyle = useAnimatedStyle(() => ({
    color: interpolateColor(flash.value, [0, 1], ['#000000', up.value ? GREEN : RED]),
  }));

  return <Animated.Text style={[styles.price, colorStyle]}>{formatPrice(coin.price)}</Animated.Text>;
}

// MARK: - Row

const Row = memo(function Row({ coin }: { coin: Coin }) {
  const positive = coin.change24h >= 0;
  const tint = positive ? GREEN : RED;
  return (
    <View style={styles.row}>
      <View style={[styles.icon, { backgroundColor: coin.color }]}>
        <Text style={styles.iconLetter}>{coin.ticker[0]}</Text>
      </View>
      <View style={styles.nameBlock}>
        <Text style={styles.name} numberOfLines={1}>
          {coin.name}
        </Text>
        <Text style={styles.ticker} numberOfLines={1}>
          {coin.ticker}
        </Text>
      </View>
      <Chart coinId={coin.id} series={coin.series} color={tint} />
      <View style={styles.priceBlock}>
        <Price coin={coin} />
        <View style={[styles.pill, { backgroundColor: tint }]}>
          <Text style={styles.pillText}>{formatChange(coin.change24h)}</Text>
        </View>
      </View>
      <View style={styles.separator} />
    </View>
  );
});

// MARK: - Live ticks (SPEC "Live ticks")

function applyTick(coins: Coin[], k: number): Coin[] {
  const next = coins.slice();
  for (let j = 0; j < 40; j++) {
    const index = (k * 7919 + j * 104729) % 5000;
    const delta = (((k * 31 + j * 17) % 201) - 100) / 10000;
    const c = next[index];
    const price = c.price * (1 + delta);
    next[index] = {
      ...c,
      price,
      series: [...c.series.slice(1), price],
      change24h: c.change24h + delta * 100,
      seq: (c.seq ?? 0) + 1,
      up: delta >= 0,
    };
  }
  return next;
}

// MARK: - Screen

export default function App() {
  const [coins, setCoins] = useState(COINS);
  const tick = useRef(0);

  useEffect(() => {
    if (!BENCH_TICKS) return;
    const timer = setInterval(() => {
      const k = ++tick.current;
      setCoins((prev) => applyTick(prev, k));
    }, 100);
    return () => clearInterval(timer);
  }, []);

  const renderItem = useCallback(({ item }: { item: Coin }) => <Row coin={item} />, []);

  return (
    <SafeAreaProvider>
      <StatusBar style="dark" />
      <SafeAreaView style={styles.screen} edges={['top', 'left', 'right']}>
        <View style={styles.topBar}>
          <Text style={styles.title}>Markets</Text>
          <Text style={styles.live}>{BENCH_TICKS ? 'Live: on' : 'Live: off'}</Text>
        </View>
        <View style={styles.listBox}>
          <LegendList
            data={coins}
            renderItem={renderItem}
            keyExtractor={keyExtractor}
            recycleItems
            getFixedItemSize={rowSize}
            initialScrollIndex={BENCH_START_INDEX ?? undefined}
          />
        </View>
      </SafeAreaView>
    </SafeAreaProvider>
  );
}

const styles = StyleSheet.create({
  screen: { flex: 1, backgroundColor: '#FFFFFF' },
  listBox: { flex: 1 },
  topBar: {
    flexDirection: 'row',
    justifyContent: 'space-between',
    alignItems: 'center',
    paddingVertical: 10,
    paddingHorizontal: 16,
    borderBottomWidth: 0.5,
    borderBottomColor: HAIRLINE,
    backgroundColor: '#FFFFFF',
  },
  title: { fontSize: 17, fontWeight: '600', color: '#000000' },
  live: { fontSize: 13, color: SECONDARY },
  row: {
    height: 64,
    flexDirection: 'row',
    alignItems: 'center',
    paddingHorizontal: 16,
    backgroundColor: '#FFFFFF',
  },
  separator: {
    position: 'absolute',
    left: 0,
    right: 0,
    bottom: 0,
    height: StyleSheet.hairlineWidth, // 1 px
    backgroundColor: HAIRLINE,
  },
  icon: {
    width: 32,
    height: 32,
    borderRadius: 16,
    alignItems: 'center',
    justifyContent: 'center',
    marginRight: 12,
  },
  iconLetter: { fontSize: 15, fontWeight: '600', color: '#FFFFFF' },
  nameBlock: { flex: 1, minWidth: 0 },
  name: { fontSize: 16, fontWeight: '600', color: '#000000' },
  ticker: { fontSize: 13, color: SECONDARY, marginTop: 2 },
  // 96 × 32 chart box, 12 pt gaps on both sides; the canvas is outset 9 pt all round.
  chart: {
    width: CHART_W + 2 * OUTSET,
    height: CHART_H + 2 * OUTSET,
    marginLeft: 12 - OUTSET,
    marginRight: 12 - OUTSET,
    marginVertical: -OUTSET,
  },
  priceBlock: { width: 104, alignItems: 'flex-end' },
  price: { fontSize: 16, fontWeight: '600', color: '#000000', fontVariant: ['tabular-nums'] },
  pill: {
    marginTop: 4,
    paddingVertical: 2,
    paddingHorizontal: 6,
    borderRadius: 4,
  },
  pillText: { fontSize: 12, fontWeight: '600', color: '#FFFFFF' },
});
