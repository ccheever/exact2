// Synchronous text residency. Accepted Paragraphs are owned by their views;
// this cache owns only reusable cold work and weakly indexes surviving leases.
import Foundation
import CoreText
import CExact

/// Same metric-only key for owned geometry and a synchronous borrowed request.
/// Hashes select candidates only; matches performs exact field/UTF8 equality.
/// No borrowed buffer, request, or pointer is retained in the identity index.
enum TextMetricKey {
    private static func fields(_ size: CGFloat, _ weight: Int, _ family: Int, _ italic: Bool,
                               _ lineHeight: CGFloat?, _ spacing: CGFloat, _ h: inout Hasher) {
        h.combine(size); h.combine(weight); h.combine(family); h.combine(italic)
        h.combine(lineHeight); h.combine(spacing)
    }
    private static func fields(_ r: Run, _ h: inout Hasher) {
        fields(r.size, r.weight, r.family, r.italic, r.lineHeight, r.letterSpacing, &h)
    }
    private static func fields(_ r: ExactTextRun, _ h: inout Hasher) {
        fields(CGFloat(r.font_size), Int(r.font_weight), Int(r.font_family), r.italic != 0,
               r.has_line_height != 0 ? CGFloat(r.line_height) : nil, CGFloat(r.letter_spacing), &h)
    }
    static func hash(_ spec: Spec) -> Int {
        var h = Hasher()
        h.combine(spec.runs.count)
        for r in spec.runs {
            var text = r.text
            text.withUTF8 { h.combine(bytes: UnsafeRawBufferPointer($0)) }
            fields(r, &h)
        }
        h.combine(spec.strut != nil)
        if let strut = spec.strut { fields(strut, &h) }
        h.combine(spec.align); h.combine(spec.lineClamp); h.combine(spec.overflowWrap)
        return h.finalize()
    }
    static func hash(_ request: ExactMeasureRequest) -> Int {
        var h = Hasher()
        h.combine(request.count)
        for r in UnsafeBufferPointer(start: request.runs, count: request.count) {
            h.combine(bytes: UnsafeRawBufferPointer(start: r.text, count: r.len))
            fields(r, &h)
        }
        h.combine(true) // The callback always constructs a strut, even for empty text.
        fields(request.strut, &h)
        h.combine(Int(request.align)); h.combine(Int(request.line_clamp)); h.combine(Int(request.overflow_wrap))
        return h.finalize()
    }
    private static func equalFields(_ raw: ExactTextRun, _ owned: Run) -> Bool {
        CGFloat(raw.font_size) == owned.size && Int(raw.font_weight) == owned.weight
            && Int(raw.font_family) == owned.family && (raw.italic != 0) == owned.italic
            && (raw.has_line_height != 0 ? CGFloat(raw.line_height) : nil) == owned.lineHeight
            && CGFloat(raw.letter_spacing) == owned.letterSpacing
    }
    static func matches(_ request: ExactMeasureRequest, _ geometry: Spec) -> Bool {
        guard request.count == geometry.runs.count, Int(request.align) == geometry.align,
              Int(request.line_clamp) == geometry.lineClamp, Int(request.overflow_wrap) == geometry.overflowWrap,
              let strut = geometry.strut, equalFields(request.strut, strut) else { return false }
        let runs = UnsafeBufferPointer(start: request.runs, count: request.count)
        for i in runs.indices {
            guard equalFields(runs[i], geometry.runs[i]) else { return false }
            var text = geometry.runs[i].text
            let equal = text.withUTF8 { $0.elementsEqual(UnsafeBufferPointer(start: runs[i].text, count: runs[i].len)) }
            guard equal else { return false }
        }
        return true
    }
}

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

enum TextScalarKind: Hashable {
    case minimumWidth, minContent, maxContent
    case definite(UInt64)
}

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
    let metadataEntries: Int
    let identityEntries: Int
    let geometryEntries: Int
    let maintenanceVisits: UInt64
    /// Includes the deterministic potential ink-index payload; not actual RSS.
    let coldAdmissionBytes: Int
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

/// Value-semantic dictionaries and links preserve independent checkpoint owners.
/// Cold entries and weak lookup metadata have separate count caps. Forgetting a
/// lookup never releases a view/checkpoint-owned Paragraph; it may cost a later
/// remeasurement. Dead weak metadata cannot grow with historical row count.
struct TextResidency {
    static let defaultSoftTargetBytes = 64 * 1024 * 1024
    static let maxColdEntries = 4096
    static let maxLookupEntries = 8192
    static let maxIdentities = 4096
    private static let cleanupQuota = 4
    let softTargetBytes: Int
    private let catalog = TextCatalogIdentity()
    private struct Entry {
        let weak: WeakTextValue
        var cold: TextValue?
        var previous: TextEntryKey?
        var next: TextEntryKey?
        var colder: TextEntryKey?
        var warmer: TextEntryKey?
    }
    private struct IdentityEntry {
        let hash: Int
        let weak: WeakTextIdentity
        var previous: TextIdentityToken?
        var next: TextIdentityToken?
    }
    private struct Charge {
        var count: Int
        let owned: Int
        let opaque: Int
    }
    private var entries: [TextEntryKey: Entry] = [:]
    private var first: TextEntryKey?, last: TextEntryKey?, sweepEntry: TextEntryKey?
    private var coldFirst: TextEntryKey?, coldLast: TextEntryKey?
    private var coldCount = 0
    private var coldGroups: [TextIdentityToken: Set<TextEntryKey>] = [:]
    private var geometryIndex: [TextGeometryKey: Set<TextEntryKey>] = [:]
    private var identities: [Int: Set<TextIdentityToken>] = [:]
    private var identityEntries: [TextIdentityToken: IdentityEntry] = [:]
    private var firstIdentity: TextIdentityToken?, lastIdentity: TextIdentityToken?, sweepIdentity: TextIdentityToken?
    private var coldSources: [TextIdentityToken: Charge] = [:]
    private var coldShapes: [ObjectIdentifier: Charge] = [:]
    private var ownedBudget = 0, opaqueBudget = 0
    private var maintenanceVisits: UInt64 = 0

    init(softTargetBytes: Int = Self.defaultSoftTargetBytes) {
        self.softTargetBytes = max(0, softTargetBytes)
    }
    mutating func identity(_ spec: Spec) -> TextIdentity {
        maintain()
        return identityAfterBorrowedMiss(spec)
    }
    /// Completes the same identity admission after borrowedIdentity already ran
    /// maintenance. Malformed UTF8 must be hashed again after replacement decoding.
    mutating func identityAfterBorrowedMiss(_ spec: Spec) -> TextIdentity {
        let geometry = spec.geometry, hash = TextMetricKey.hash(geometry)
        for token in identities[hash] ?? [] {
            maintenanceVisits &+= 1
            if let value = identityEntries[token]?.weak.value, value.geometry == geometry {
                touchIdentity(token)
                return value
            }
        }
        let value = TextIdentity(geometry, catalog: catalog)
        // Token comes from the actual identity; no pointer interning or serial reuse.
        let key = value.token
        identityEntries[key] = IdentityEntry(hash: hash, weak: WeakTextIdentity(value), previous: lastIdentity)
        if let lastIdentity { identityEntries[lastIdentity]?.next = key } else { firstIdentity = key }
        lastIdentity = key
        identities[hash, default: []].insert(key)
        while identityEntries.count > Self.maxIdentities, let oldest = firstIdentity { removeIdentity(oldest) }
        return value
    }
    /// Metric-cache lookup before String/Run/Spec construction. On a miss the
    /// caller decodes normally and completes admission without a second sweep.
    mutating func borrowedIdentity(_ request: ExactMeasureRequest) -> TextIdentity? {
        maintain()
        let hash = TextMetricKey.hash(request)
        for token in identities[hash] ?? [] {
            maintenanceVisits &+= 1
            if let value = identityEntries[token]?.weak.value, TextMetricKey.matches(request, value.geometry) {
                touchIdentity(token)
                return value
            }
        }
        return nil
    }
    private mutating func touchIdentity(_ token: TextIdentityToken) {
        guard lastIdentity !== token, let e = identityEntries[token] else { return }
        if let p = e.previous { identityEntries[p]?.next = e.next } else { firstIdentity = e.next }
        if let n = e.next { identityEntries[n]?.previous = e.previous }
        identityEntries[token]?.previous = lastIdentity; identityEntries[token]?.next = nil
        if let lastIdentity { identityEntries[lastIdentity]?.next = token }
        lastIdentity = token
    }
    private mutating func removeIdentity(_ token: TextIdentityToken) {
        guard let e = identityEntries.removeValue(forKey: token) else { return }
        maintenanceVisits &+= 1
        if let p = e.previous { identityEntries[p]?.next = e.next } else { firstIdentity = e.next }
        if let n = e.next { identityEntries[n]?.previous = e.previous } else { lastIdentity = e.previous }
        if sweepIdentity === token { sweepIdentity = e.next ?? firstIdentity }
        identities[e.hash]?.remove(token)
        if identities[e.hash]?.isEmpty == true { identities.removeValue(forKey: e.hash) }
    }
    private mutating func get(_ key: TextEntryKey) -> TextValue? {
        maintain()
        guard let value = entries[key]?.weak.value else { return nil }
        touch(key)
        if entries[key]?.cold != nil { touchCold(key) }
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
    /// Exact same-source, same-width paint handoff; index membership is retired
    /// with the entry, so dead arrays of historical paints cannot accumulate.
    mutating func geometry(_ identity: TextIdentity, width: CGFloat) -> Paragraph? {
        maintain()
        let key = TextGeometryKey(token: identity.token, widthBits: Double(width == 0 ? 0 : width).bitPattern)
        for entry in geometryIndex[key] ?? [] {
            maintenanceVisits &+= 1
            if case .paragraph(let p) = entries[entry]?.weak.value {
                touch(entry)
                if entries[entry]?.cold != nil { touchCold(entry) }
                return p
            }
        }
        return nil
    }
    mutating func retireWidths(_ key: TextParagraphKey) {
        maintain()
        // Only this source's cold variants; accepted widths stay weakly indexed.
        for old in coldGroups[key.shape.token] ?? [] {
            maintenanceVisits &+= 1
            switch old {
            case .paragraph(let p) where p != key:
                // Layout can revisit an exploratory width after its CTLines
                // were retired. Keep its tiny answer under the same bounded
                // cold policy, without retaining another full paragraph.
                if case .paragraph(let paragraph) = entries[old]?.weak.value {
                    let metrics = ExactMetrics(width: Float(paragraph.width), height: Float(paragraph.height),
                                               baseline: Float(paragraph.firstBaseline))
                    removeCold(old)
                    put(paragraph.shape!.identity, kind: .definite(p.widthBits), metrics: metrics)
                } else { removeCold(old) }
            case .shape(let s) where s != key.shape: removeCold(old)
            default: break
            }
        }
    }
    mutating func accepted(_ paragraph: Paragraph) {
        maintain()
        guard let key = paragraph.residencyKey else { return }
        removeCold(.paragraph(key)); removeCold(.shape(key.shape))
    }
    mutating func put(_ paragraph: Paragraph) {
        guard let key = paragraph.residencyKey else { return }
        put(.paragraph(key), .paragraph(paragraph))
    }
    mutating func put(_ shape: TextShape) { put(.shape(shape.key), .shape(shape)) }
    mutating func put(_ identity: TextIdentity, kind: TextScalarKind, metrics: ExactMetrics) {
        put(.scalar(identity.token, kind), .scalar(TextScalar(identity, metrics)))
    }
    private mutating func put(_ key: TextEntryKey, _ value: TextValue) {
        maintain(); removeEntry(key)
        entries[key] = Entry(weak: WeakTextValue(value), cold: value, previous: last, colder: coldLast)
        if let last { entries[last]?.next = key } else { first = key }
        last = key
        if let coldLast { entries[coldLast]?.warmer = key } else { coldFirst = key }
        coldLast = key; coldCount += 1
        coldGroups[value.identity.token, default: []].insert(key)
        if case .paragraph(let p) = key {
            geometryIndex[TextGeometryKey(token: p.shape.token, widthBits: p.widthBits), default: []].insert(key)
        }
        charge(value, adding: true)
        trim(incoming: 0, keeping: key)
        while entries.count > Self.maxLookupEntries, let oldest = first { removeEntry(oldest) }
    }
    mutating func prepare(estimatedBytes: Int) {
        maintain(); trim(incoming: estimatedBytes, keeping: nil)
    }
    private mutating func trim(incoming: Int, keeping: TextEntryKey?) {
        let allowance = max(0, softTargetBytes - min(softTargetBytes, incoming))
        // O(1) when no eviction is needed; O(evictions) under pressure. One
        // current oversize value is preserved, as before, and stats expose it.
        while ownedBudget + opaqueBudget > allowance || coldCount > Self.maxColdEntries {
            guard let key = coldFirst, key != keeping else { break }
            maintenanceVisits &+= 1
            removeCold(key)
        }
    }
    private mutating func touch(_ key: TextEntryKey) {
        guard key != last, let e = entries[key] else { return }
        if let p = e.previous { entries[p]?.next = e.next } else { first = e.next }
        if let n = e.next { entries[n]?.previous = e.previous }
        entries[key]?.previous = last; entries[key]?.next = nil
        if let last { entries[last]?.next = key }
        last = key
    }
    private mutating func touchCold(_ key: TextEntryKey) {
        guard key != coldLast, let e = entries[key] else { return }
        if let p = e.colder { entries[p]?.warmer = e.warmer } else { coldFirst = e.warmer }
        if let n = e.warmer { entries[n]?.colder = e.colder }
        entries[key]?.colder = coldLast; entries[key]?.warmer = nil
        if let coldLast { entries[coldLast]?.warmer = key }
        coldLast = key
    }
    private mutating func removeCold(_ key: TextEntryKey) {
        guard let e = entries[key], let value = e.cold else { return }
        if let p = e.colder { entries[p]?.warmer = e.warmer } else { coldFirst = e.warmer }
        if let n = e.warmer { entries[n]?.colder = e.colder } else { coldLast = e.colder }
        entries[key]?.cold = nil; entries[key]?.colder = nil; entries[key]?.warmer = nil
        coldGroups[value.identity.token]?.remove(key)
        if coldGroups[value.identity.token]?.isEmpty == true { coldGroups.removeValue(forKey: value.identity.token) }
        coldCount -= 1
        charge(value, adding: false)
    }
    private mutating func removeEntry(_ key: TextEntryKey) {
        guard entries[key] != nil else { return }
        removeCold(key)
        guard let e = entries.removeValue(forKey: key) else { return }
        maintenanceVisits &+= 1
        if let p = e.previous { entries[p]?.next = e.next } else { first = e.next }
        if let n = e.next { entries[n]?.previous = e.previous } else { last = e.previous }
        if sweepEntry == key { sweepEntry = e.next ?? first }
        if case .paragraph(let p) = key {
            let geometry = TextGeometryKey(token: p.shape.token, widthBits: p.widthBits)
            geometryIndex[geometry]?.remove(key)
            if geometryIndex[geometry]?.isEmpty == true { geometryIndex.removeValue(forKey: geometry) }
        }
    }
    private mutating func charge(_ value: TextValue, adding: Bool) {
        let sign = adding ? 1 : -1, token = value.identity.token
        if adding {
            if coldSources[token] == nil {
                let cost = value.identity.ownedBytes
                coldSources[token] = Charge(count: 0, owned: cost, opaque: 0); ownedBudget += cost
            }
            coldSources[token]!.count += 1
        } else {
            coldSources[token]!.count -= 1
            if coldSources[token]!.count == 0 {
                ownedBudget -= coldSources.removeValue(forKey: token)!.owned
            }
        }
        if let shape = value.shape {
            let key = ObjectIdentifier(shape)
            if adding {
                if coldShapes[key] == nil {
                    let cost = Charge(count: 0, owned: shape.ownedBytes, opaque: shape.opaqueEstimate)
                    coldShapes[key] = cost; ownedBudget += cost.owned; opaqueBudget += cost.opaque
                }
                coldShapes[key]!.count += 1
            } else {
                coldShapes[key]!.count -= 1
                if coldShapes[key]!.count == 0 {
                    let cost = coldShapes.removeValue(forKey: key)!
                    ownedBudget -= cost.owned; opaqueBudget -= cost.opaque
                }
            }
        }
        switch value {
        case .paragraph(let p):
            ownedBudget += sign * p.admissionPayloadBytes; opaqueBudget += sign * p.coreTextEstimateBytes
        case .scalar:
            ownedBudget += sign * (MemoryLayout<ExactMetrics>.stride + MemoryLayout<CGFloat?>.stride)
        case .shape: break
        }
    }
    private mutating func maintain() {
        // Fixed work per operation. Hard caps above bound metadata even if
        // every observed weak value dies just after this incremental sweep.
        for _ in 0..<Self.cleanupQuota {
            if let key = sweepEntry ?? first, let e = entries[key] {
                maintenanceVisits &+= 1
                sweepEntry = e.next ?? first
                if e.weak.value == nil { removeEntry(key) }
            }
            if let key = sweepIdentity ?? firstIdentity, let e = identityEntries[key] {
                maintenanceVisits &+= 1
                sweepIdentity = e.next ?? firstIdentity
                if e.weak.value == nil { removeIdentity(key) }
            }
        }
    }
    /// Explicit diagnostics may enumerate metadata. Hot admission uses cached
    /// charges, including possible lazy ink; reported payload uses actual ink.
    var stats: TextResidencyStats {
        var paragraphs = 0, shapes = 0, scalars = 0, inkSlack = 0
        for entry in entries.values {
            switch entry.weak.value {
            case .paragraph: paragraphs += 1
            case .shape: shapes += 1
            case .scalar: scalars += 1
            case nil: break
            }
            if case .paragraph(let p) = entry.cold { inkSlack += p.admissionPayloadBytes - p.ownedPayloadBytes }
        }
        return TextResidencyStats(metadataEntries: entries.count, identityEntries: identityEntries.count,
            geometryEntries: geometryIndex.values.reduce(0) { $0 + $1.count }, maintenanceVisits: maintenanceVisits,
            coldAdmissionBytes: ownedBudget + opaqueBudget, coldEntries: coldCount,
            liveParagraphs: paragraphs, liveShapes: shapes, scalarEntries: scalars,
            identities: identityEntries.values.filter { $0.weak.value != nil }.count,
            coldOwnedPayloadBytes: ownedBudget - inkSlack, coldCoreTextEstimateBytes: opaqueBudget, softTargetBytes: softTargetBytes)
    }
}
