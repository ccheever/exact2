// Caltrain's data source in TypeScript — the executor's fixture (LLP 1027
// D5, §9 stage 1): the same seam as apps/caltrain/data/src/lib.rs, source by
// source and message by message, so `tests/caltrain.rs` can hold the two to
// the same bytes. Caltrain the app stays Rust (Charlie, 2026-09-03); this
// file is a test. No imports, no I/O, no globals that reach out.

type Location = { lat: number; lon: number };
type Station = { id: string; name: string; zone: number; distance: number };
type Departure = { id: string; train: number; service: string; headsign: string; at: number };


type Answer =
  | { tag: 0; value: unknown }
  | { tag: 2; kind: "UnknownSource" | "BadArguments" | "Unavailable"; message: string };

class DataError extends Error {
  constructor(public kind: "UnknownSource" | "BadArguments" | "Unavailable", message: string) {
    super(message);
  }
}

type Row = { id: string; name: string; zone: number; lat: number; lon: number; offsetMin: number };

const STATIONS: Row[] = [
  { id: "sf", name: "San Francisco", zone: 1, lat: 37.7765, lon: -122.3947, offsetMin: 0 },
  { id: "22nd", name: "22nd Street", zone: 1, lat: 37.7573, lon: -122.392, offsetMin: 5 },
  { id: "millbrae", name: "Millbrae", zone: 2, lat: 37.6003, lon: -122.3868, offsetMin: 20 },
  { id: "sanmateo", name: "San Mateo", zone: 2, lat: 37.568, lon: -122.324, offsetMin: 27 },
  { id: "redwood", name: "Redwood City", zone: 3, lat: 37.4855, lon: -122.231, offsetMin: 38 },
  { id: "paloalto", name: "Palo Alto", zone: 3, lat: 37.4436, lon: -122.1647, offsetMin: 45 },
  { id: "mv", name: "Mountain View", zone: 4, lat: 37.3947, lon: -122.0763, offsetMin: 52 },
  { id: "sunnyvale", name: "Sunnyvale", zone: 4, lat: 37.3784, lon: -122.0312, offsetMin: 57 },
  { id: "sj", name: "San Jose Diridon", zone: 4, lat: 37.3297, lon: -121.9023, offsetMin: 70 },
];

const DAY_START_MS = 1_787_875_200_000;
const DEFAULT_LOCATION: Location = { lat: 37.3947, lon: -122.0763 };
const SERVICES: [string, number, number][] = [
  ["Local", 5, 1],
  ["Limited", 25, 0.8],
  ["Express", 45, 0.6],
];

function station(id: string): Row | undefined {
  return STATIONS.find((s) => s.id === id);
}

/** Great-circle distance in meters (haversine). */
function distanceM(a: Location, b: Location): number {
  const r = 6_371_000;
  const lat1 = a.lat * (Math.PI / 180);
  const lon1 = a.lon * (Math.PI / 180);
  const lat2 = b.lat * (Math.PI / 180);
  const lon2 = b.lon * (Math.PI / 180);
  const dlat = lat2 - lat1;
  const dlon = lon2 - lon1;
  const s1 = Math.sin(dlat / 2);
  const s2 = Math.sin(dlon / 2);
  const h = s1 * s1 + Math.cos(lat1) * Math.cos(lat2) * s2 * s2;
  return 2 * r * Math.asin(Math.sqrt(h));
}

function stationValue(s: Row, location: Location): Station {
  return { id: s.id, name: s.name, zone: s.zone, distance: Math.round(distanceM(location, s)) };
}

function departures(s: Row, direction: string): Departure[] {
  const terminus = direction === "north" ? STATIONS[0] : STATIONS[STATIONS.length - 1];
  if (terminus.id === s.id) return [];
  const sign = direction === "north" ? -1 : 1;
  const out: [number, Departure][] = [];
  SERVICES.forEach(([service, every, speed], si) => {
    let minute = 5 * 60 + si * 7;
    let n = 0;
    while (minute < 23 * 60) {
      const at = minute + sign * s.offsetMin * speed;
      if (at >= 0 && at < 24 * 60) {
        const train = 100 + si * 100 + n * 2 + (direction === "north" ? 1 : 0);
        const headsign = direction === "north" ? "San Francisco" : "San Jose";
        out.push([at, { id: `${s.id}-${direction}-${train}`, train, service, headsign, at: DAY_START_MS + at * 60_000 }]);
      }
      minute += every * 6;
      n += 1;
    }
  });
  out.sort((a, b) => a[0] - b[0]);
  return out.map(([, d]) => d);
}

// --- argument checks, the crate's, message for message ---------------------

function arity(args: unknown[], expected: number): void {
  if (args.length !== expected) {
    throw new DataError("BadArguments", `expected ${expected} arguments, got ${args.length}`);
  }
}

function loc(args: unknown[], i: number): Location {
  const v = args[i] as Partial<Location> | undefined;
  if (
    v && typeof v === "object" && Object.keys(v).length === 2 &&
    typeof v.lat === "number" && typeof v.lon === "number" &&
    Number.isFinite(v.lat) && Number.isFinite(v.lon) &&
    v.lat >= -90 && v.lat <= 90 && v.lon >= -180 && v.lon <= 180
  ) {
    return { lat: v.lat, lon: v.lon };
  }
  throw new DataError("BadArguments", "location");
}

function text(args: unknown[], i: number): string {
  const v = args[i];
  if (typeof v === "string") return v;
  throw new DataError("BadArguments", `argument ${i}`);
}

function finiteNumber(args: unknown[], i: number): number {
  const v = args[i];
  if (typeof v === "number" && Number.isFinite(v)) return v;
  throw new DataError("BadArguments", `argument ${i}`);
}

// --- the sources -------------------------------------------------------------

function query(source: string, args: unknown[]): unknown {
  switch (source) {
    case "defaultLocation": {
      arity(args, 0);
      console.log("defaultLocation", DEFAULT_LOCATION);
      return DEFAULT_LOCATION;
    }
    case "stations": {
      arity(args, 1);
      const location = loc(args, 0);
      return STATIONS.map((s) => stationValue(s, location));
    }
    case "nearest": {
      arity(args, 2);
      const location = loc(args, 0);
      const count = finiteNumber(args, 1);
      if (count % 1 !== 0 || count < 0 || count > STATIONS.length) {
        throw new DataError("BadArguments", "count");
      }
      const all = STATIONS.slice();
      all.sort((a, b) => distanceM(location, a) - distanceM(location, b));
      return all.slice(0, count).map((s) => stationValue(s, location));
    }
    case "station": {
      arity(args, 2);
      const id = text(args, 0);
      const location = loc(args, 1);
      const s = station(id);
      if (!s) throw new DataError("Unavailable", `station ${id}`);
      return stationValue(s, location);
    }
    case "board": {
      arity(args, 3);
      const id = text(args, 0);
      const direction = text(args, 1);
      const nowMs = finiteNumber(args, 2);
      if (direction !== "north" && direction !== "south") {
        throw new DataError("BadArguments", "direction");
      }
      const s = station(id);
      if (!s) throw new DataError("Unavailable", `station ${id}`);
      return departures(s, direction).filter((d) => d.at >= nowMs);
    }
    case "search": {
      arity(args, 2);
      const q = text(args, 0).toLowerCase();
      const location = loc(args, 1);
      return STATIONS.filter((s) => s.name.toLowerCase().includes(q)).map((s) => stationValue(s, location));
    }
    default:
      throw new DataError("UnknownSource", source);
  }
}

// --- the seam ------------------------------------------------------------------

function answer(source: string, argsJson: string): string {
  const args = JSON.parse(argsJson) as unknown[];
  try {
    return JSON.stringify({ tag: 0, value: query(source, args) } satisfies Answer);
  } catch (e) {
    if (e instanceof DataError) return JSON.stringify({ tag: 2, kind: e.kind, message: e.message } satisfies Answer);
    throw e;
  }
}

(globalThis as any).exact = { abi: 1, appId: "com.exact.caltrain", grants: "", answer };
