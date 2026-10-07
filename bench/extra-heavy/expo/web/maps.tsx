// react-native-maps has no web implementation (its docs point to Google Maps JS, which needs an API key).
// The web counterpart used here draws the same thing exact2's web `native-map` module draws (exact2
// apps/map-demo/modules/web/index.js): OpenStreetMap raster tiles as <img>s for the region at the zoom that
// shows `latitudeDelta`, and one default pin at the centre. Non-interactive, as the row asks.
import { createElement, useState, type ReactNode } from 'react';
import { View } from 'react-native';

const TILE = 256;
const project = (lat: number, lon: number, z: number) => {
  const s = TILE * 2 ** z, r = Math.sin((lat * Math.PI) / 180);
  return [((lon + 180) / 360) * s, (0.5 - Math.log((1 + r) / (1 - r)) / (4 * Math.PI)) * s];
};
const PIN = `<svg width="28" height="38" viewBox="0 0 28 38"><path d="M14 37C14 37 27 22 27 13.5A13 13 0 0 0 1 13.5C1 22 14 37 14 37Z" fill="#2d6cdf" stroke="#fff" stroke-width="2"/><circle cx="14" cy="13.5" r="4.5" fill="#fff"/></svg>`;

type Region = { latitude: number; longitude: number; latitudeDelta: number; longitudeDelta: number };
export default function MapView({ region, style }: { region: Region; style?: any; children?: ReactNode; [k: string]: any }) {
  const [size, setSize] = useState({ w: 0, h: 0 });
  const tiles: ReactNode[] = [];
  if (size.w && size.h) {
    const [, y0] = project(region.latitude - region.latitudeDelta / 2, 0, 0), [, y1] = project(region.latitude + region.latitudeDelta / 2, 0, 0);
    const zoom = Math.min(19, Math.max(2, Math.log2(size.h / Math.abs(y0 - y1))));
    const z = Math.round(zoom), k = 2 ** (zoom - z), n = 2 ** z;
    const [cx, cy] = project(region.latitude, region.longitude, z);
    const left = cx - size.w / 2 / k, top = cy - size.h / 2 / k;
    for (let ty = Math.floor(top / TILE); ty <= Math.floor((top + size.h / k) / TILE); ty++) {
      if (ty < 0 || ty >= n) continue;
      for (let tx = Math.floor(left / TILE); tx <= Math.floor((left + size.w / k) / TILE); tx++) {
        const key = `${z}/${((tx % n) + n) % n}/${ty}`;
        tiles.push(createElement('img', {
          key: key + '@' + tx, src: `https://tile.openstreetmap.org/${key}.png`, alt: '', decoding: 'async',
          style: { position: 'absolute', width: TILE, height: TILE, transformOrigin: '0 0',
            transform: `translate(${(tx * TILE - left) * k}px, ${(ty * TILE - top) * k}px) scale(${k})` },
        }));
      }
    }
  }
  return (
    <View style={[style, { backgroundColor: '#dfe6ea', overflow: 'hidden' }]} pointerEvents="none"
      onLayout={(e) => setSize({ w: e.nativeEvent.layout.width, h: e.nativeEvent.layout.height })}>
      {tiles}
      {createElement('div', { style: { position: 'absolute', left: '50%', top: '50%', transform: 'translate(-50%, -100%)' }, dangerouslySetInnerHTML: { __html: PIN } })}
      {createElement('div', { style: { position: 'absolute', right: 0, bottom: 0, font: '11px system-ui', background: 'rgba(255,255,255,.8)', padding: '2px 6px', color: '#333' } }, '© OpenStreetMap contributors')}
    </View>
  );
}
export function Marker(_: any) { return null; }
