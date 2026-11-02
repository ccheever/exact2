export interface ContentRect { x: number; y: number; width: number; height: number }
export interface ContentInsets { top: number; right: number; bottom: number; left: number }
export interface ContentRegion extends ContentRect { kind: 'division' | 'occlusion' }
type SizeClass = 'compact' | 'regular' | 'unspecified';
type Hinge = 'unknown' | 'absent' | 'present';
export interface ContentGeometry {
  state: 'unavailable' | 'ready'; owner: string; generation: number; serial: number;
  coordinateSpace: 'content-observer'; bounds: ContentRect; safeArea: ContentInsets;
  horizontalSizeClass: SizeClass; verticalSizeClass: SizeClass; hinge: Hinge;
  verticalBarEdge: 'none' | 'left' | 'right'; bottomBarInset: number | null;
  reservedRegions: ContentRegion[];
}

const record = (value: unknown): value is Record<string, unknown> => value !== null && typeof value === 'object' && !Array.isArray(value);
const finite = (value: unknown): value is number => typeof value === 'number' && Number.isFinite(value);
const nonnegative = (value: unknown): value is number => finite(value) && value >= 0;
const counter = (value: string): number => /^[1-9]\d*$/.test(value) && Number.isSafeInteger(Number(value)) ? Number(value) : 0;
const sizeClass = (value: unknown): value is SizeClass => value === 'compact' || value === 'regular' || value === 'unspecified';
const hinge = (value: unknown): value is Hinge => value === 'unknown' || value === 'absent' || value === 'present';
const barEdge = (value: unknown): value is ContentGeometry['verticalBarEdge'] => value === 'none' || value === 'left' || value === 'right';
const regionKind = (value: unknown): value is ContentRegion['kind'] => value === 'division' || value === 'occlusion';
const insets = (value: unknown): value is ContentInsets => record(value) && nonnegative(value.top)
  && nonnegative(value.right) && nonnegative(value.bottom) && nonnegative(value.left);
const rect = (value: unknown): value is ContentRect => record(value) && finite(value.x) && finite(value.y)
  && finite(value.width) && value.width > 0 && finite(value.height) && value.height > 0
  && Number.isFinite(value.x + value.width) && Number.isFinite(value.y + value.height);

function unavailable(owner: string, generation = 0, serial = 0, capability: Hinge = 'unknown'): ContentGeometry {
  return { state: 'unavailable', owner, generation, serial, coordinateSpace: 'content-observer',
    bounds: { x: 0, y: 0, width: 0, height: 0 }, safeArea: { top: 0, right: 0, bottom: 0, left: 0 },
    horizontalSizeClass: 'unspecified', verticalSizeClass: 'unspecified', hinge: capability,
    verticalBarEdge: 'none', bottomBarInset: null, reservedRegions: [] };
}

export function mobileContentGeometry(owner: string, storedOwner: string, generation: number, serial: number, packet: string): ContentGeometry {
  const empty = unavailable(owner);
  if (!owner || owner !== storedOwner) return empty;
  const parts = packet.split('\n');
  if (parts.length !== 3 || !counter(parts[0]!) || !counter(parts[1]!)
    || counter(parts[0]!) !== generation || counter(parts[1]!) !== serial) return empty;
  let value: unknown;
  try { value = JSON.parse(parts[2]!); } catch { return empty; }
  if (!record(value) || value.owner !== owner || value.generation !== generation || value.serial !== serial
    || value.coordinateSpace !== 'content-observer' || !hinge(value.hinge)) return empty;
  if (value.state === 'unavailable') return unavailable(owner, generation, serial, value.hinge);
  if (value.state !== 'ready' || !rect(value.bounds) || !insets(value.safeArea)
    || !sizeClass(value.horizontalSizeClass) || !sizeClass(value.verticalSizeClass)
    || !barEdge(value.verticalBarEdge)
    || value.bottomBarInset !== null && !nonnegative(value.bottomBarInset) || !Array.isArray(value.reservedRegions)) return empty;
  const bounds = value.bounds, regions: ContentRegion[] = [];
  for (const region of value.reservedRegions) {
    if (!record(region) || !rect(region) || !regionKind(region.kind)) return empty;
    const x = Math.max(bounds.x, region.x), y = Math.max(bounds.y, region.y);
    const width = Math.min(bounds.x + bounds.width, region.x + region.width) - x;
    const height = Math.min(bounds.y + bounds.height, region.y + region.height) - y;
    if (width > 0 && height > 0) regions.push({ kind: region.kind, x, y, width, height });
  }
  return { state: 'ready', owner, generation, serial, coordinateSpace: 'content-observer', bounds,
    safeArea: value.safeArea, horizontalSizeClass: value.horizontalSizeClass,
    verticalSizeClass: value.verticalSizeClass, hinge: value.hinge,
    verticalBarEdge: value.verticalBarEdge, bottomBarInset: value.bottomBarInset,
    reservedRegions: regions };
}
