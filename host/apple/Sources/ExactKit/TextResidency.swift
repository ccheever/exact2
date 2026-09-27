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
                               _ lineHeight: CGFloat?, _ spacing: CGFloat, _ numeric: Int, _ h: inout Hasher) {
        h.combine(size); h.combine(weight); h.combine(family); h.combine(italic)
        h.combine(lineHeight); h.combine(spacing); h.combine(numeric)
    }
    private static func fields(_ r: Run, _ h: inout Hasher) {
        fields(r.size, r.weight, r.family, r.italic, r.lineHeight, r.letterSpacing, r.numeric, &h)
    }
    private static func fields(_ r: ExactTextRun, _ h: inout Hasher) {
        fields(CGFloat(r.font_size), Int(r.font_weight), Int(r.font_family), r.italic != 0,
               r.has_line_height != 0 ? CGFloat(r.line_height) : nil, CGFloat(r.letter_spacing),
               Int(r.font_variant_numeric), &h)
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
        h.combine(spec.direction); h.combine(spec.whiteSpace)
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
        h.combine(Int(request.direction)); h.combine(Int(request.white_space))
        // A Markdown request hashes apart from the plain request of its one
        // source run; a plain request hashes as its Spec does.
        if request.markup != 0 { h.combine(Int(request.markup)) }
        return h.finalize()
    }
    private static func equalFields(_ raw: ExactTextRun, _ owned: Run) -> Bool {
        CGFloat(raw.font_size) == owned.size && Int(raw.font_weight) == owned.weight
            && Int(raw.font_family) == owned.family && (raw.italic != 0) == owned.italic
            && (raw.has_line_height != 0 ? CGFloat(raw.line_height) : nil) == owned.lineHeight
            && CGFloat(raw.letter_spacing) == owned.letterSpacing
            && Int(raw.font_variant_numeric) == owned.numeric
    }
    static func matches(_ request: ExactMeasureRequest, _ geometry: Spec) -> Bool {
        // An expanded Markdown request has more runs than its one source run;
        // its geometry is keyed by the request hash and never borrowed by runs.
        guard request.markup == 0 else { return false }
        guard request.count == geometry.runs.count, Int(request.align) == geometry.align,
              Int(request.line_clamp) == geometry.lineClamp, Int(request.overflow_wrap) == geometry.overflowWrap,
              Int(request.direction) == geometry.direction, Int(request.white_space) == geometry.whiteSpace,
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
    let runEnds: [Int]
    init(_ geometry: Spec, catalog: TextCatalogIdentity) {
        self.geometry = geometry; self.catalog = catalog
        utf8Bytes = geometry.runs.reduce(0) { $0 + $1.text.utf8.count }
        var end = 0
        runEnds = geometry.runs.map { run in
            end += run.text.utf16.count
            return end
        }
        utf16Count = end
    }
    static func == (lhs: TextIdentity, rhs: TextIdentity) -> Bool { lhs === rhs }
    func hash(into hasher: inout Hasher) { hasher.combine(ObjectIdentifier(self)) }
    var ownedBytes: Int { utf8Bytes + geometry.runs.count * MemoryLayout<Run>.stride + runEnds.count * MemoryLayout<Int>.stride }
}

/// Text/font metrics are interned by exact Run equality, not a digest or ABI
/// pointer. Width and paint keys then share that owned identity and its Strings.
struct TextPaint: Hashable {
    struct Inline: Hashable {
        let color: [Double]?
        let decoration: String
        let href: String
        let background: [Double]?
    }
    let color: [Double]
    let runs: [Inline]
    let ellipsis: Bool
    init(_ spec: Spec) {
        color = spec.color
        ellipsis = spec.ellipsis
        runs = spec.runs.map { Inline(color: $0.color, decoration: $0.decoration, href: $0.href, background: $0.background) }
    }
    func applying(to identity: TextIdentity) -> Spec {
        var spec = identity.geometry
        spec.color = color
        spec.ellipsis = ellipsis
        for i in spec.runs.indices {
            spec.runs[i].color = runs[i].color
            spec.runs[i].decoration = runs[i].decoration
            spec.runs[i].href = runs[i].href
            spec.runs[i].background = runs[i].background
        }
        return spec
    }
}

struct TextShapeKey: Hashable {
    let token: TextIdentityToken
    let paint: TextPaint
    // LRU links and indexes hash this key many times per admission. Paint is
    // immutable; walk its run array once, retaining exact equality on hits.
    private let cachedHash: Int
    init(identity: TextIdentity, paint: TextPaint) {
        token = identity.token; self.paint = paint
        var hash = Hasher()
        hash.combine(token); hash.combine(paint)
        cachedHash = hash.finalize()
    }
    func hash(into hasher: inout Hasher) { hasher.combine(cachedHash) }
    static func == (lhs: Self, rhs: Self) -> Bool {
        lhs.cachedHash == rhs.cachedHash && lhs.token === rhs.token && lhs.paint == rhs.paint
    }
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
    // Reuse unchanged CTLines across widths while a view still owns its prior
    // paragraph. This weak reference adds no paragraph or width-history owner.
    weak var lastParagraph: Paragraph?
    let attributed: NSAttributedString
    // Unicode opportunities belong to this immutable source, never a width.
    // Filled lazily by the session's TextEngine; raster workers do not use it.
    var lineBreakBoundaries: [Int]?
    private(set) var flow: TextFlowSource?
    private(set) var prepareCount = 0
    func preparedFlow() -> TextFlowSource {
        if let flow { return flow }
        let prepared = TextFlowSource(self)
        flow = prepared; prepareCount += 1
        return prepared
    }
    init(key: TextShapeKey, identity: TextIdentity, attributed: NSAttributedString) {
        self.attributed = attributed
        self.key = key; self.identity = identity; spec = key.paint.applying(to: identity)
        typesetter = CTTypesetterCreateWithAttributedString(attributed)
    }
    // Policy estimate, not measurement of opaque CoreText allocations. Source
    // String payload is accounted once separately; paint key payload is owned.
    var ownedBytes: Int {
        (lineBreakBoundaries?.count ?? 0) * MemoryLayout<Int>.stride
            + (flow?.ownedBytes ?? 0) + spec.runs.count * MemoryLayout<Run>.stride
            + key.paint.color.count * MemoryLayout<Double>.stride + key.paint.runs.reduce(0) {
            $0 + MemoryLayout<TextPaint.Inline>.stride + ($1.color?.count ?? 0) * MemoryLayout<Double>.stride
                + $1.decoration.utf8.count + $1.href.utf8.count
        }
    }
    var opaqueEstimate: Int { identity.utf16Count * 32 + spec.runs.count * 512 + (flow == nil ? 0 : identity.utf16Count * 64 + 256) }
}

private final class WeakTextIdentity {
    weak var value: TextIdentity?
    init(_ value: TextIdentity) { self.value = value }
}

private final class TextScalar {
    let identity: TextIdentity
    let metrics: ExactMetrics
    let minimum: CGFloat?
    /// What is left of a measured paragraph whose shaped form went (its
    /// `CTTypesetter` and `CTLine`s, a few kilobytes): where its lines break
    /// and their baselines. A worker rasters from these and paint lays its
    /// lines there, so the paragraph coming back is not typeset again.
    let lines: (ranges: [CFRange], baselines: [CGFloat])?
    init(_ identity: TextIdentity, _ metrics: ExactMetrics, minimum: CGFloat? = nil,
         lines: (ranges: [CFRange], baselines: [CGFloat])? = nil) {
        self.identity = identity; self.metrics = metrics; self.minimum = minimum; self.lines = lines
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
///
/// Cold values are two lists. Shaped text (sources and paragraphs) is what a
/// measurement leaves: a list's travel leaves thousands, a few kilobytes each,
/// and what reuses them is a paragraph measured again. So they are held to
/// `shapedLimit`, which the host scales to the paragraphs a screen shows
/// (`fitShaped`), the most recently measured first: those in the direction
/// of travel and the screens just passed. A paragraph that goes leaves its
/// answer (a few hundred bytes: metrics, line breaks, baselines) on the other
/// list, so a list that comes back is measured and rastered without being
/// typeset again. Answers are held to `maxColdEntries` with everything else.
struct TextResidency {
    static let defaultSoftTargetBytes = 64 * 1024 * 1024
    static let maxColdEntries = 4096
    /// Two screens of shaped text behind the travel: a paragraph measured
    /// leaves a source and a paragraph (`fitShaped`).
    static let shapedPerParagraph = 4
    /// Below this the entries within one layout pass (a paragraph offered
    /// several widths) would evict each other.
    static let minShaped = 256
    static let maxLookupEntries = 8192
    static let maxIdentities = 4096
    // A measurement crosses several indexes. Sweep only when admitting a new
    // source, not at each nested lookup, put, prepare and acceptance. The entry
    // and identity caps independently bound metadata between these visits.
    private static let cleanupQuota = 1
    let softTargetBytes: Int
    private let catalog = TextCatalogIdentity()
    /// One entry in `slab`. Its links are `links` at the same index: the
    /// recency list of every entry and the list of cold ones. A key per link
    /// cost four keys an entry, and plain indexes relink without copying an
    /// entry (whose weak reference makes a copy a runtime call).
    private struct Entry {
        let key: TextEntryKey
        /// The value while a view or checkpoint owns it (weak lookup metadata).
        weak var object: AnyObject?
        var cold: TextValue?
        init(key: TextEntryKey, value: TextValue) {
            self.key = key; object = value.object; cold = value
        }
        var owned: TextValue? {
            if let p = object as? Paragraph { return .paragraph(p) }
            if let s = object as? TextShape { return .shape(s) }
            if let s = object as? TextScalar { return .scalar(s) }
            return nil
        }
        var value: TextValue? { cold ?? owned }
        var dead: Bool { cold == nil && object == nil }
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
    private var entries: [TextEntryKey: Int32] = [:]
    /// `colder`/`warmer` link an entry into its cold list (shaped or answers);
    /// `stamp` orders the two lists' entries against each other.
    private struct Links { var previous: Int32 = -1, next: Int32 = -1, colder: Int32 = -1, warmer: Int32 = -1, stamp: UInt64 = 0 }
    private var slab: [Entry?] = []
    private var links: [Links] = []
    private var vacant: [Int32] = []
    private var first: Int32 = -1, last: Int32 = -1, sweepEntry: Int32 = -1
    /// The shaped list, coldest first; the answers list.
    private var coldFirst: Int32 = -1, coldLast: Int32 = -1
    private var answerFirst: Int32 = -1, answerLast: Int32 = -1
    private var coldCount = 0, shapedCount = 0
    private var clock: UInt64 = 0
    /// At most this many cold shaped values (`fitShaped`).
    private(set) var shapedLimit = Self.maxColdEntries
    // Retirement visits layouts only; saved scalar answers need no width retirement.
    private var coldLayouts: [TextIdentityToken: Set<TextEntryKey>] = [:]
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
        return identityAfterBorrowedMiss(spec)
    }
    /// Completes admission after a borrowed lookup. Malformed UTF8 must be
    /// hashed again after replacement decoding.
    mutating func identityAfterBorrowedMiss(_ spec: Spec) -> TextIdentity {
        let geometry = spec.geometry, hash = TextMetricKey.hash(geometry)
        for token in identities[hash] ?? [] {
            maintenanceVisits &+= 1
            if let value = identityEntries[token]?.weak.value, value.geometry == geometry {
                touchIdentity(token)
                return value
            }
        }
        maintain()
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
    /// caller decodes normally and completes admission without repeating lookup.
    mutating func borrowedIdentity(_ request: ExactMeasureRequest) -> TextIdentity? {
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
    private func entry(_ key: TextEntryKey) -> Entry? { entries[key].flatMap { slab[Int($0)] } }
    private mutating func get(_ key: TextEntryKey) -> TextValue? {
        guard let i = entries[key], let value = slab[Int(i)]?.value else { return nil }
        touch(i)
        if slab[Int(i)]?.cold != nil { touchCold(i) }
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
        let key = TextGeometryKey(token: identity.token, widthBits: Double(width == 0 ? 0 : width).bitPattern)
        for entry in geometryIndex[key] ?? [] {
            maintenanceVisits &+= 1
            if let i = entries[entry], case .paragraph(let p) = slab[Int(i)]?.value {
                touch(i)
                if slab[Int(i)]?.cold != nil { touchCold(i) }
                return p
            }
        }
        return nil
    }
    mutating func retireWidths(_ key: TextParagraphKey) {
        // Only this source's cold variants; accepted widths stay weakly indexed.
        for old in coldLayouts[key.shape.token] ?? [] {
            maintenanceVisits &+= 1
            switch old {
            case .paragraph(let p) where p != key:
                // Layout can revisit an exploratory width after its CTLines
                // were retired. Keep its tiny answer under the same bounded
                // cold policy, without retaining another full paragraph.
                if case .paragraph(let paragraph) = entry(old)?.owned {
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
        insert(key, value)
        trim(incoming: 0, keeping: key)
        capLookups()
    }
    private mutating func capLookups() {
        while entries.count > Self.maxLookupEntries, first >= 0 { removeEntry(at: first) }
    }
    private mutating func insert(_ key: TextEntryKey, _ value: TextValue) {
        removeEntry(key)
        let i: Int32
        let entry = Entry(key: key, value: value), link = Links(previous: last)
        if let reused = vacant.popLast() {
            i = reused; slab[Int(i)] = entry; links[Int(i)] = link
        } else {
            i = Int32(slab.count); slab.append(entry); links.append(link)
        }
        entries[key] = i
        if last >= 0 { links[Int(last)].next = i } else { first = i }
        last = i
        linkCold(i, shaped: value.shape != nil)
        coldCount += 1
        if value.shape != nil {
            shapedCount += 1
            coldLayouts[value.identity.token, default: []].insert(key)
        }
        if case .paragraph(let p) = key {
            geometryIndex[TextGeometryKey(token: p.shape.token, widthBits: p.widthBits), default: []].insert(key)
        }
        charge(value, adding: true)
    }
    /// Hold cold shaped text to what `visibleParagraphs` paragraphs on a
    /// screen need: two screens' worth, never fewer than `minShaped`.
    mutating func fitShaped(visibleParagraphs: Int) {
        shapedLimit = min(Self.maxColdEntries, max(Self.minShaped, visibleParagraphs * Self.shapedPerParagraph))
        trim(incoming: 0, keeping: nil)
    }
    /// A paragraph's answer at its width, with the lines it broke into, if
    /// its shaped form has gone. Measuring and rastering it need no more.
    mutating func answerLines(_ identity: TextIdentity, width: CGFloat) -> ([CFRange], [CGFloat])? {
        let kind = TextScalarKind.definite(Double(width == 0 ? 0 : width).bitPattern)
        guard case .scalar(let s) = get(.scalar(identity.token, kind)), let lines = s.lines else { return nil }
        return (lines.ranges, lines.baselines)
    }
    /// Lazy walker storage becomes part of the resident shape's existing charge.
    mutating func refresh(_ shape: TextShape) {
        let key = ObjectIdentifier(shape)
        if let old = coldShapes[key] {
            ownedBudget += shape.ownedBytes - old.owned
            opaqueBudget += shape.opaqueEstimate - old.opaque
            coldShapes[key] = Charge(count: old.count, owned: shape.ownedBytes, opaque: shape.opaqueEstimate)
        }
        trim(incoming: 0, keeping: .shape(shape.key))
    }
    /// Checkpoints share immutable sources whose lazy preparation can grow.
    /// Their copied charge tables must catch up before re-admitting cold work.
    /// Restore is exceptional; ordinary lookups never scan the cache.
    mutating func refreshAfterRestore() {
        var seen: Set<ObjectIdentifier> = []
        for case let entry? in slab {
            guard let shape = entry.cold?.shape else { continue }
            let key = ObjectIdentifier(shape)
            guard seen.insert(key).inserted, let old = coldShapes[key] else { continue }
            ownedBudget += shape.ownedBytes - old.owned
            opaqueBudget += shape.opaqueEstimate - old.opaque
            coldShapes[key] = Charge(count: old.count, owned: shape.ownedBytes, opaque: shape.opaqueEstimate)
        }
        trim(incoming: 0, keeping: coldLast >= 0 ? slab[Int(coldLast)]?.key : nil)
    }
    /// Memory pressure: every cold value goes; views keep what they show.
    mutating func dropCold() {
        while coldFirst >= 0 { removeCold(at: coldFirst) }
        while answerFirst >= 0 { removeCold(at: answerFirst) }
        compact()
    }
    /// At rest: cold shaped text goes, and the lines kept of shaped text that
    /// went before; cold measurements (a few bytes each, what a row coming
    /// back needs first) stay.
    mutating func dropColdShaped() {
        while coldFirst >= 0 { removeCold(at: coldFirst) }
        var i = answerFirst
        while i >= 0 {
            let current = i
            i = links[Int(i)].warmer
            if case .scalar(let s)? = slab[Int(current)]?.cold, s.lines != nil { removeCold(at: current) }
        }
        compact()
    }
    /// At rest or under pressure: the lookup metadata of values no one owns
    /// any more goes too (a fling leaves thousands of entries, each with its
    /// key's arrays). Their slots and buckets stay for the next admissions.
    private mutating func compact() {
        var i = first
        while i >= 0 {
            let current = i
            i = links[Int(i)].next
            if slab[Int(current)]?.dead == true { removeEntry(at: current) }
        }
        for (token, e) in identityEntries where e.weak.value == nil { removeIdentity(token) }
    }
    mutating func prepare(estimatedBytes: Int) {
        trim(incoming: estimatedBytes, keeping: nil)
    }
    private mutating func trim(incoming: Int, keeping: TextEntryKey?) {
        let allowance = max(0, softTargetBytes - min(softTargetBytes, incoming))
        // O(1) when no eviction is needed; O(evictions) under pressure. One
        // current oversize value is preserved, as before, and stats expose it.
        while ownedBudget + opaqueBudget > allowance || coldCount > Self.maxColdEntries || shapedCount > shapedLimit {
            // Past the shaped limit the coldest shaped value goes and leaves
            // its answer; past the others the coldest value of either list.
            let shaped = shapedCount > shapedLimit
            let i = shaped || answerFirst < 0 ? coldFirst
                : coldFirst < 0 ? answerFirst
                : links[Int(coldFirst)].stamp < links[Int(answerFirst)].stamp ? coldFirst : answerFirst
            guard i >= 0, slab[Int(i)]?.key != keeping else { break }
            maintenanceVisits &+= 1
            if shaped { evict(at: i) } else { removeCold(at: i) }
        }
        capLookups()
    }
    /// A cold paragraph at a width leaves its answer; anything else just goes.
    private mutating func evict(at i: Int32) {
        guard case .paragraph(let p)? = slab[Int(i)]?.cold, let key = p.residencyKey, let shape = p.shape,
              Double(bitPattern: key.widthBits).isFinite else { removeCold(at: i); return }
        removeCold(at: i)
        let answer = TextEntryKey.scalar(shape.identity.token, .definite(key.widthBits))
        guard entries[answer] == nil else { return }
        let lines = shape.spec.lineClamp == 0 && p.origins.isEmpty
            ? (p.lines.map { CTLineGetStringRange($0) }, p.baselines) : nil
        let metrics = ExactMetrics(width: Float(p.width), height: Float(p.height), baseline: Float(p.firstBaseline))
        insert(answer, .scalar(TextScalar(shape.identity, metrics, lines: lines)))
    }
    private mutating func touch(_ i: Int32) {
        guard i != last else { return }
        let e = links[Int(i)]
        if e.previous >= 0 { links[Int(e.previous)].next = e.next } else { first = e.next }
        if e.next >= 0 { links[Int(e.next)].previous = e.previous }
        links[Int(i)].previous = last; links[Int(i)].next = -1
        if last >= 0 { links[Int(last)].next = i }
        last = i
    }
    private func isShaped(_ i: Int32) -> Bool {
        if case .scalar? = slab[Int(i)]?.cold { return false }
        return true
    }
    /// Append `i` at the warm end of its cold list.
    private mutating func linkCold(_ i: Int32, shaped: Bool) {
        clock &+= 1
        let tail = shaped ? coldLast : answerLast
        links[Int(i)].colder = tail; links[Int(i)].warmer = -1; links[Int(i)].stamp = clock
        if tail >= 0 { links[Int(tail)].warmer = i } else if shaped { coldFirst = i } else { answerFirst = i }
        if shaped { coldLast = i } else { answerLast = i }
    }
    private mutating func unlinkCold(_ i: Int32, shaped: Bool) {
        let e = links[Int(i)]
        if e.colder >= 0 { links[Int(e.colder)].warmer = e.warmer } else if shaped { coldFirst = e.warmer } else { answerFirst = e.warmer }
        if e.warmer >= 0 { links[Int(e.warmer)].colder = e.colder } else if shaped { coldLast = e.colder } else { answerLast = e.colder }
        links[Int(i)].colder = -1; links[Int(i)].warmer = -1
    }
    private mutating func touchCold(_ i: Int32) {
        let shaped = isShaped(i)
        guard i != (shaped ? coldLast : answerLast) else { return }
        unlinkCold(i, shaped: shaped)
        linkCold(i, shaped: shaped)
    }
    private mutating func removeCold(_ key: TextEntryKey) {
        if let i = entries[key] { removeCold(at: i) }
    }
    private mutating func removeCold(at i: Int32) {
        guard let value = slab[Int(i)]?.cold, let key = slab[Int(i)]?.key else { return }
        unlinkCold(i, shaped: value.shape != nil)
        slab[Int(i)]?.cold = nil
        if value.shape != nil {
            shapedCount -= 1
            coldLayouts[value.identity.token]?.remove(key)
            if coldLayouts[value.identity.token]?.isEmpty == true { coldLayouts.removeValue(forKey: value.identity.token) }
        }
        coldCount -= 1
        charge(value, adding: false)
    }
    private mutating func removeEntry(_ key: TextEntryKey) {
        if let i = entries[key] { removeEntry(at: i) }
    }
    private mutating func removeEntry(at i: Int32) {
        removeCold(at: i)
        guard let key = slab[Int(i)]?.key else { return }
        let e = links[Int(i)]
        entries.removeValue(forKey: key)
        slab[Int(i)] = nil; links[Int(i)] = Links(); vacant.append(i)
        maintenanceVisits &+= 1
        if e.previous >= 0 { links[Int(e.previous)].next = e.next } else { first = e.next }
        if e.next >= 0 { links[Int(e.next)].previous = e.previous } else { last = e.previous }
        if sweepEntry == i { sweepEntry = e.next >= 0 ? e.next : first }
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
        // Fixed work per new source. Hard caps above bound metadata even if
        // every observed weak value dies just after this incremental sweep.
        for _ in 0..<Self.cleanupQuota {
            let i = sweepEntry >= 0 ? sweepEntry : first
            if i >= 0, slab[Int(i)] != nil {
                maintenanceVisits &+= 1
                sweepEntry = links[Int(i)].next >= 0 ? links[Int(i)].next : first
                if slab[Int(i)]?.dead == true { removeEntry(at: i) }
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
        for case let entry? in slab {
            switch entry.owned {
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
