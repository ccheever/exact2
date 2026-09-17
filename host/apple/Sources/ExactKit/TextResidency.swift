// Synchronous text residency. Accepted Paragraphs are owned by their views;
// this cache owns only reusable cold work and weakly indexes surviving leases.
import Foundation
import CoreText
import CExact

/// A namespace is retained by its identities, so restoring a checkpoint cannot
/// alias a candidate catalog, including after allocator address reuse.
final class TextCatalogIdentity {}

/// Payload-free key lifetime: weak lookup metadata cannot retain dead text. A
/// token remains unique while any key/checkpoint holds it, without raw-pointer
/// reuse or a process-global serial.
final class TextIdentityToken: Hashable {
    static func == (lhs: TextIdentityToken, rhs: TextIdentityToken) -> Bool { lhs === rhs }
    func hash(into hasher: inout Hasher) { hasher.combine(ObjectIdentifier(self)) }
}

final class TextIdentity: Hashable {
    let geometry: Spec
    let token = TextIdentityToken()
    let catalog: TextCatalogIdentity
    let utf8Bytes: Int
    let utf16Count: Int
    init(_ geometry: Spec, catalog: TextCatalogIdentity) {
        self.geometry = geometry; self.catalog = catalog
        utf8Bytes = geometry.runs.reduce(0) { $0 + $1.text.utf8.count }
        utf16Count = geometry.runs.reduce(0) { $0 + $1.text.utf16.count }
    }
    static func == (lhs: TextIdentity, rhs: TextIdentity) -> Bool { lhs === rhs }
    func hash(into hasher: inout Hasher) { hasher.combine(ObjectIdentifier(self)) }
    var ownedBytes: Int { utf8Bytes + geometry.runs.count * MemoryLayout<Run>.stride }
}

/// Text/font metrics are interned by exact Run equality, not a digest or ABI
/// pointer. Width and paint keys then share that owned identity and its Strings.
struct TextPaint: Hashable {
    struct Inline: Hashable {
        let color: [Double]?
        let decoration: String
        let href: String
    }
    let color: [Double]
    let runs: [Inline]
    init(_ spec: Spec) {
        color = spec.color
        runs = spec.runs.map { Inline(color: $0.color, decoration: $0.decoration, href: $0.href) }
    }
    func applying(to identity: TextIdentity) -> Spec {
        var spec = identity.geometry
        spec.color = color
        for i in spec.runs.indices {
            spec.runs[i].color = runs[i].color
            spec.runs[i].decoration = runs[i].decoration
            spec.runs[i].href = runs[i].href
        }
        return spec
    }
}

struct TextShapeKey: Hashable {
    let token: TextIdentityToken
    let paint: TextPaint
    init(identity: TextIdentity, paint: TextPaint) { token = identity.token; self.paint = paint }
}

struct TextParagraphKey: Hashable {
    let shape: TextShapeKey
    // IEEE bits preserve the exact offer; max-content is a distinct infinity.
    let widthBits: UInt64
    init(shape: TextShapeKey, width: CGFloat) {
        self.shape = shape; widthBits = Double(width == 0 ? 0 : width).bitPattern
    }
}

private struct TextGeometryKey: Hashable {
    let token: TextIdentityToken
    let widthBits: UInt64
}

enum TextScalarKind { case minimumWidth, minContent, maxContent }

final class TextShape {
    let key: TextShapeKey
    let identity: TextIdentity
    let spec: Spec
    let typesetter: CTTypesetter
    init(key: TextShapeKey, identity: TextIdentity, attributed: NSAttributedString) {
        self.key = key; self.identity = identity; spec = key.paint.applying(to: identity)
        typesetter = CTTypesetterCreateWithAttributedString(attributed)
    }
    // Policy estimate, not measurement of opaque CoreText allocations. Source
    // String payload is accounted once separately; paint key payload is owned.
    var ownedBytes: Int {
        spec.runs.count * MemoryLayout<Run>.stride
            + key.paint.color.count * MemoryLayout<Double>.stride + key.paint.runs.reduce(0) {
            $0 + MemoryLayout<TextPaint.Inline>.stride + ($1.color?.count ?? 0) * MemoryLayout<Double>.stride
                + $1.decoration.utf8.count + $1.href.utf8.count
        }
    }
    var opaqueEstimate: Int { identity.utf16Count * 32 + spec.runs.count * 512 }
}

private final class WeakTextIdentity {
    weak var value: TextIdentity?
    init(_ value: TextIdentity) { self.value = value }
}

private final class TextScalar {
    let identity: TextIdentity
    let metrics: ExactMetrics
    let minimum: CGFloat?
    init(_ identity: TextIdentity, _ metrics: ExactMetrics, minimum: CGFloat? = nil) {
        self.identity = identity; self.metrics = metrics; self.minimum = minimum
    }
}

private enum TextEntryKey: Hashable {
    case paragraph(TextParagraphKey)
    case shape(TextShapeKey)
    case scalar(TextIdentityToken, TextScalarKind)
}

private enum TextValue {
    case paragraph(Paragraph)
    case shape(TextShape)
    case scalar(TextScalar)
    var object: AnyObject {
        switch self { case .paragraph(let p): return p; case .shape(let s): return s; case .scalar(let s): return s }
    }
    var identity: TextIdentity {
        switch self {
        case .paragraph(let p): return p.shape!.identity
        case .shape(let s): return s.identity
        case .scalar(let s): return s.identity
        }
    }
    var shape: TextShape? {
        switch self { case .paragraph(let p): return p.shape; case .shape(let s): return s; case .scalar: return nil }
    }
}

private final class WeakTextValue {
    private weak var object: AnyObject?
    init(_ value: TextValue) { object = value.object }
    var value: TextValue? {
        if let p = object as? Paragraph { return .paragraph(p) }
        if let s = object as? TextShape { return .shape(s) }
        if let s = object as? TextScalar { return .scalar(s) }
        return nil
    }
}

struct TextResidencyStats {
    let coldEntries: Int
    let liveParagraphs: Int
    let liveShapes: Int
    let scalarEntries: Int
    let identities: Int
    /// Logical Swift payload tally (not malloc capacities/headers); shared
    /// source/shape identities count once. Strings in different runs may share
    /// storage, so this is conservative even before the CoreText estimate.
    let coldOwnedPayloadBytes: Int
    /// Named heuristic for otherwise opaque typesetter/CTLine allocations.
    let coldCoreTextEstimateBytes: Int
    let softTargetBytes: Int
    var coldEstimatedBytes: Int { coldOwnedPayloadBytes + coldCoreTextEstimateBytes }
    /// Working handoff/ink-index growth may exceed the target until the next
    /// miss or acceptance removes cold ownership. This is never a hard ceiling.
    var coldOverageBytes: Int { max(0, coldEstimatedBytes - softTargetBytes) }
    // These are this cache's cold holdings, not total residency. Accepted view
    // leases, external CTLine aliases, saved checkpoints, retired catalogs,
    // font caches, allocator slack and other CoreText internals are excluded.
}

/// Entry and LRU mutations have value semantics. Weak boxes never mutate their
/// referent; a saved checkpoint independently owns its cold values until dropped.
struct TextResidency {
    static let defaultSoftTargetBytes = 64 * 1024 * 1024
    let softTargetBytes: Int
    private let catalog = TextCatalogIdentity()
    private var identities: [Int: [WeakTextIdentity]] = [:]
    private struct Entry {
        let weak: WeakTextValue
        var cold: TextValue?
        var used: UInt64
    }
    private var entries: [TextEntryKey: Entry] = [:]
    private var geometryIndex: [TextGeometryKey: [WeakTextValue]] = [:]
    private var clock: UInt64 = 0

    init(softTargetBytes: Int = Self.defaultSoftTargetBytes) {
        self.softTargetBytes = max(0, softTargetBytes)
    }
    mutating func identity(_ spec: Spec) -> TextIdentity {
        let geometry = spec.geometry
        let hash = geometry.hashValue
        if let value = identities[hash]?.compactMap(\.value).first(where: { $0.geometry == geometry }) { return value }
        prune()
        let value = TextIdentity(geometry, catalog: catalog)
        identities[hash, default: []].append(WeakTextIdentity(value))
        return value
    }
    private mutating func get(_ key: TextEntryKey) -> TextValue? {
        guard let index = entries.index(forKey: key), let value = entries.values[index].weak.value else { return nil }
        clock &+= 1; entries.values[index].used = clock
        // A weak hit is already pinned elsewhere. Never turn it into a new cold
        // owner: releasing the last view must release an otherwise retired width.
        return value
    }
    mutating func paragraph(_ key: TextParagraphKey) -> Paragraph? {
        if case .paragraph(let p) = get(.paragraph(key)) { return p }; return nil
    }
    mutating func shape(_ key: TextShapeKey) -> TextShape? {
        if case .shape(let s) = get(.shape(key)) { return s }; return nil
    }
    mutating func scalar(_ identity: TextIdentity, kind: TextScalarKind) -> ExactMetrics? {
        if case .scalar(let s) = get(.scalar(identity.token, kind)) { return s.metrics }; return nil
    }
    mutating func minimum(_ identity: TextIdentity) -> CGFloat? {
        if case .scalar(let s) = get(.scalar(identity.token, .minimumWidth)) { return s.minimum }; return nil
    }
    mutating func putMinimum(_ identity: TextIdentity, width: CGFloat) {
        put(.scalar(identity.token, .minimumWidth), .scalar(TextScalar(identity, ExactMetrics(), minimum: width)))
    }
    /// Paint-independent lookup preserves the measured breaks without retaining
    /// a second black CTLine array once the colored paragraph is accepted.
    mutating func geometry(_ identity: TextIdentity, width: CGFloat) -> Paragraph? {
        let bits = Double(width == 0 ? 0 : width).bitPattern
        let key = TextGeometryKey(token: identity.token, widthBits: bits)
        for weak in geometryIndex[key] ?? [] {
            if case .paragraph(let p) = weak.value, let key = p.residencyKey {
                _ = get(.paragraph(key))
                return p
            }
        }
        return nil
    }
    mutating func retireWidths(_ key: TextParagraphKey) {
        for old in entries.keys {
            switch old {
            case .paragraph(let p) where p.shape.token === key.shape.token && p != key:
                entries[old]?.cold = nil
            case .shape(let s) where s.token === key.shape.token && s != key.shape:
                entries[old]?.cold = nil
            default: break
            }
        }
        prune()
    }
    mutating func accepted(_ paragraph: Paragraph) {
        guard let key = paragraph.residencyKey else { return }
        entries[.paragraph(key)]?.cold = nil
        entries[.shape(key.shape)]?.cold = nil
        prune()
    }
    mutating func put(_ paragraph: Paragraph) {
        guard let key = paragraph.residencyKey else { return }
        let geometry = TextGeometryKey(token: key.shape.token, widthBits: key.widthBits)
        geometryIndex[geometry, default: []].append(WeakTextValue(.paragraph(paragraph)))
        put(.paragraph(key), .paragraph(paragraph))
    }
    mutating func put(_ shape: TextShape) { put(.shape(shape.key), .shape(shape)) }
    mutating func put(_ identity: TextIdentity, kind: TextScalarKind, metrics: ExactMetrics) {
        put(.scalar(identity.token, kind), .scalar(TextScalar(identity, metrics)))
    }
    private mutating func put(_ key: TextEntryKey, _ value: TextValue) {
        clock &+= 1
        entries[key] = Entry(weak: WeakTextValue(value), cold: value, used: clock)
        // A single current working value and its shared dependencies may exceed
        // the soft target. Evict older cold work, never reject or truncate text.
        trim(incoming: 0, keeping: key)
    }
    mutating func prepare(estimatedBytes: Int) {
        prune()
        trim(incoming: estimatedBytes, keeping: nil)
    }
    private mutating func trim(incoming: Int, keeping: TextEntryKey?) {
        let allowance = max(0, softTargetBytes - min(softTargetBytes, incoming))
        if coldBytes().total <= allowance { return }
        let ordered = entries.filter { $0.value.cold != nil && $0.key != keeping }
            .sorted { $0.value.used < $1.value.used }.map(\.key)
        for key in ordered {
            if coldBytes().total <= allowance { break }
            entries[key]?.cold = nil
        }
        prune()
    }
    private mutating func prune() {
        entries = entries.filter { $0.value.weak.value != nil }
        geometryIndex = geometryIndex.mapValues { $0.filter { $0.value != nil } }.filter { !$0.value.isEmpty }
        identities = identities.mapValues { $0.filter { $0.value != nil } }.filter { !$0.value.isEmpty }
    }
    private func coldBytes() -> (owned: Int, opaque: Int, total: Int) {
        var sources = Set<TextIdentity>(), shapes = Set<ObjectIdentifier>()
        var owned = 0, opaque = 0
        for entry in entries.values {
            guard let value = entry.cold else { continue }
            if sources.insert(value.identity).inserted { owned += value.identity.ownedBytes }
            if let shape = value.shape, shapes.insert(ObjectIdentifier(shape)).inserted {
                owned += shape.ownedBytes; opaque += shape.opaqueEstimate
            }
            if case .paragraph(let p) = value {
                owned += p.ownedPayloadBytes; opaque += p.coreTextEstimateBytes
            }
            if case .scalar = value {
                owned += MemoryLayout<ExactMetrics>.stride + MemoryLayout<CGFloat?>.stride
            }
        }
        return (owned, opaque, owned + opaque)
    }
    var stats: TextResidencyStats {
        let bytes = coldBytes()
        var paragraphs = 0, shapes = 0, scalars = 0
        for entry in entries.values {
            switch entry.weak.value {
            case .paragraph: paragraphs += 1
            case .shape: shapes += 1
            case .scalar: scalars += 1
            case nil: break
            }
        }
        return TextResidencyStats(coldEntries: entries.values.filter { $0.cold != nil }.count,
            liveParagraphs: paragraphs, liveShapes: shapes, scalarEntries: scalars,
            identities: identities.values.reduce(0) { $0 + $1.filter { $0.value != nil }.count },
            coldOwnedPayloadBytes: bytes.owned, coldCoreTextEstimateBytes: bytes.opaque, softTargetBytes: softTargetBytes)
    }
}
