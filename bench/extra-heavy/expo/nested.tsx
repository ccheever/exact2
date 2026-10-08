// Kinds 18 and 19 (SPEC "Row kinds"): a feed row that holds its own virtualized list.
// The ordinary React Native: a nested LegendList (horizontal for the strip, vertical with
// nestedScrollEnabled for the inbox) in a fixed-size box; its offset kept per row id in a
// module-level map. onScroll records it; the row id is the inner list's dataKey, so a recycled
// cell handed another row starts a new dataset at that row's initialScrollOffset.
import { LegendList } from '@legendapp/list/react-native';
import { Image } from 'expo-image';
import { memo, useCallback, useMemo } from 'react';
import { Platform, StyleSheet, Text, View, type NativeScrollEvent, type NativeSyntheticEvent } from 'react-native';

import { IMAGES } from './assets';
import { useC } from './clock';
import type { FeedRow } from './rows';
import file from './data/feed.json';

type Message = { name: string; avatar: string; text: string };
const POOL = (file as unknown as { messages: Message[] }).messages;

/** Inner-list offsets by row id (app state that outlives recycling). */
const offsets = new Map<string, number>();

function useKeptOffset(rowId: string, horizontal: boolean) {
  const onScroll = useCallback((e: NativeSyntheticEvent<NativeScrollEvent>) => {
    const o = e.nativeEvent.contentOffset;
    offsets.set(rowId, horizontal ? o.x : o.y);
  }, [rowId, horizontal]);
  // iOS (as measured) starts an unsaved list at 0. The web passes no offset when none is saved: LegendList
  // 3.4.0 on the web leaves a horizontal list given initialScrollOffset={0} with a dataKey at opacity 0.
  const kept = offsets.get(rowId);
  return { dataKey: rowId, initialScrollOffset: Platform.OS === 'web' ? kept : kept ?? 0, onScroll };
}

// MARK: 18 filmstrip

type Film = { image: string; caption: string };
const FilmCell = memo(function FilmCell({ item }: { item: Film }) {
  return (
    <View style={st.film}>
      <Image source={IMAGES[item.image]} contentFit="cover" recyclingKey={item.caption} style={st.filmImage} />
      <Text style={st.filmCaption} numberOfLines={1}>{item.caption}</Text>
    </View>
  );
});
const renderFilm = ({ item }: { item: Film }) => <FilmCell item={item} />;
const filmKey = (f: Film) => f.caption;
const filmSize = () => 112;

export function Filmstrip({ row }: { row: FeedRow }) {
  const C = useC();
  const items = useMemo<Film[]>(() => Array.from({ length: row.count }, (_, j) => ({
    image: `small-${String((row.img0 + j * row.imgStep) % 48).padStart(2, '0')}.jpg`,
    caption: `IMG_${row.num0 + j}`,
  })), [row.id]);
  const kept = useKeptOffset(row.id, true);
  return (
    <>
      <Text style={st.title}>{row.title}</Text>
      <View style={{ width: C, height: 136 }}>
        <LegendList horizontal data={items} renderItem={renderFilm} keyExtractor={filmKey} recycleItems
          getFixedItemSize={filmSize} contentContainerStyle={{ gap: 8 }} showsHorizontalScrollIndicator={false} {...kept} />
      </View>
    </>
  );
}

// MARK: 19 inbox

type Msg = Message & { time: string; key: string };
const MessageRow = memo(function MessageRow({ item }: { item: Msg }) {
  return (
    <View style={st.msg}>
      <Image source={IMAGES[item.avatar]} contentFit="cover" recyclingKey={item.key} style={st.msgAvatar} />
      <View style={st.msgBody}>
        <View style={st.msgTop}>
          <Text style={st.msgName} numberOfLines={1}>{item.name}</Text>
          <Text style={st.msgTime}>{item.time}</Text>
        </View>
        <Text style={st.msgText} numberOfLines={2}>{item.text}</Text>
      </View>
      <View style={st.msgRule} />
    </View>
  );
});
const renderMsg = ({ item }: { item: Msg }) => <MessageRow item={item} />;
const msgKey = (m: Msg) => m.key;

function clock(row: FeedRow, j: number) {
  const m = (((row.clock0 - 7 * j) % 1440) + 1440) % 1440;
  return `${String(Math.floor(m / 60)).padStart(2, '0')}:${String(m % 60).padStart(2, '0')}`;
}

export function Inbox({ row }: { row: FeedRow }) {
  const C = useC();
  const items = useMemo<Msg[]>(() => Array.from({ length: row.count }, (_, j) => ({
    ...POOL[(row.m0 + j * row.mStep) % POOL.length], time: clock(row, j), key: `${row.id}-${j}`,
  })), [row.id]);
  const kept = useKeptOffset(row.id, false);
  return (
    <>
      <Text style={st.title}>{row.title}</Text>
      <View style={[st.inbox, { width: C }]}>
        <LegendList data={items} renderItem={renderMsg} keyExtractor={msgKey} recycleItems estimatedItemSize={70}
          nestedScrollEnabled {...kept} />
      </View>
    </>
  );
}

const st = StyleSheet.create({
  title: { fontSize: 15, fontWeight: '600', color: '#000' },
  film: { width: 112, height: 136 },
  filmImage: { width: 112, height: 112, borderRadius: 8, backgroundColor: '#E5E5EA' },
  filmCaption: { marginTop: 4, fontSize: 12, color: '#3C3C43' },
  inbox: { height: 400, borderRadius: 12, borderWidth: 0.5, borderColor: '#D1D1D6', overflow: 'hidden', backgroundColor: '#fff' },
  msg: { flexDirection: 'row', alignItems: 'flex-start', paddingVertical: 10, paddingHorizontal: 12, gap: 10 },
  msgAvatar: { width: 32, height: 32, borderRadius: 16, backgroundColor: '#E5E5EA' },
  msgBody: { flex: 1, gap: 2, paddingBottom: 0 },
  msgTop: { flexDirection: 'row', alignItems: 'center', gap: 8 },
  msgName: { flex: 1, fontSize: 14, fontWeight: '600', color: '#000' },
  msgTime: { fontSize: 12, color: '#8E8E93', fontVariant: ['tabular-nums'] },
  msgText: { fontSize: 13, color: '#3C3C43' },
  msgRule: { position: 'absolute', left: 54, right: 0, bottom: 0, height: 0.5, backgroundColor: '#E5E5EA' },
});
