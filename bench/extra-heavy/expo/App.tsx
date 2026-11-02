// Extra Heavy feed benchmark, Expo port (see ../SPEC.md, Decisions included).
// The ordinary React Native way: rows from the bundled JSON, one LegendList (recycling, an item
// type per kind), a memoized row that switches on kind, community modules for every rich element.
import { LegendList } from '@legendapp/list/react-native';
import { StatusBar } from 'expo-status-bar';
import { useEffect, useState } from 'react';
import { StyleSheet, Text, useWindowDimensions, View } from 'react-native';
import { useFrameCallback, useSharedValue } from 'react-native-reanimated';
import { SafeAreaProvider, SafeAreaView } from 'react-native-safe-area-context';

import { LiveClock, MotionClock, Width } from './clock';
import { BENCH_FREEZE, BENCH_KINDS17, BENCH_KINDS_LIST, BENCH_START_INDEX, BENCH_TICKS } from './modules/bench-env';
import { Row, type FeedRow } from './rows';
import file from './data/feed.json';

const ALL_ROWS = (file as unknown as { rows: FeedRow[] }).rows;
const ROWS17 = ALL_ROWS.filter((r) => r.kind !== 'filmstrip' && r.kind !== 'inbox');
// BENCH_KINDS=<kind>[,<kind>…]: only those kinds, the 17-kind feed's row count, their rows in feed
// order cycled (a repeat's id gets `-<cycle>`), as the SwiftUI and exact2 apps do.
const PICK = BENCH_KINDS_LIST ? ALL_ROWS.filter((r) => BENCH_KINDS_LIST.includes(r.kind)) : [];
const ROWS = PICK.length
  ? ROWS17.map((_, i) => {
      const r = PICK[i % PICK.length];
      const cycle = Math.floor(i / PICK.length);
      return cycle ? { ...r, id: `${r.id}-${cycle}` } : r;
    })
  : BENCH_KINDS17 || BENCH_KINDS_LIST
    ? ROWS17
    : ALL_ROWS;
const keyExtractor = (r: FeedRow) => r.id;
const getItemType = (r: FeedRow) => r.kind;
const renderItem = ({ item }: { item: FeedRow }) => <Row row={item} />;

export default function App() {
  const { width } = useWindowDimensions();
  const C = Math.min(width - 32, 600);

  // Live clock s (1 Hz, only when ticks are on).
  const [s, setS] = useState(0);
  useEffect(() => {
    if (!BENCH_TICKS) return;
    const start = Date.now();
    const id = setInterval(() => setS(Math.floor((Date.now() - start) / 1000)), 1000);
    return () => clearInterval(id);
  }, []);

  // Motion clock t (every frame on the UI thread, unless frozen).
  const t = useSharedValue(0);
  useFrameCallback((info) => {
    t.value = info.timeSinceFirstFrame / 1000;
  }, !BENCH_FREEZE);

  return (
    <SafeAreaProvider>
      <SafeAreaView style={styles.screen} edges={['top', 'left', 'right']}>
        <StatusBar style="dark" />
        <View style={styles.bar}>
          <Text style={styles.barTitle}>Extra Heavy</Text>
          <Text style={styles.barLive}>{BENCH_TICKS ? 'Live: on' : 'Live: off'}</Text>
        </View>
        <Width.Provider value={C}>
          <MotionClock.Provider value={t}>
            <LiveClock.Provider value={s}>
              <View style={styles.listWrap}>
                <LegendList
                  data={ROWS}
                  renderItem={renderItem}
                  keyExtractor={keyExtractor}
                  getItemType={getItemType}
                  recycleItems
                  estimatedItemSize={470}
                  initialScrollIndex={BENCH_START_INDEX ?? undefined}
                />
              </View>
            </LiveClock.Provider>
          </MotionClock.Provider>
        </Width.Provider>
      </SafeAreaView>
    </SafeAreaProvider>
  );
}

const styles = StyleSheet.create({
  screen: { flex: 1, backgroundColor: '#fff' },
  bar: {
    flexDirection: 'row',
    justifyContent: 'space-between',
    alignItems: 'center',
    paddingVertical: 10,
    paddingHorizontal: 16,
    borderBottomWidth: 0.5,
    borderBottomColor: '#E5E5EA',
  },
  barTitle: { fontSize: 17, fontWeight: '600', color: '#000' },
  barLive: { fontSize: 13, color: '#8E8E93' },
  listWrap: { flex: 1 },
});
