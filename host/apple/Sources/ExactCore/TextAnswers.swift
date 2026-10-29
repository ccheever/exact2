// What measuring a paragraph answered, kept where both of a session's text
// engines read it (LLP 1072 §8.1): the measurer, on the owner thread, answers
// a paragraph it has answered before without typesetting it again, and the
// painter, on main, takes the line breaks its pixels are made from. Plain
// values under one lock; no CoreText layout object is kept or crosses a thread.
import Foundation
import CoreText
import CExact

/// A paragraph's metric content as bytes: its runs' UTF-8 and the style
/// fields that move glyphs, the paragraph's strut and wrapping. Two
/// paragraphs with equal bytes measure and break the same in one font
/// catalog. Built the same from a borrowed request and from an owned `Spec`,
/// into a buffer the caller reuses; compared exactly, never by digest alone.
///
/// Lengths are keyed at the kernel's precision. The kernel lays out in
/// `f32`; the painter's copies came through the batch's decimal text, where
/// a third of a point or a 0.41 letter spacing reads back as the nearest
/// double to the decimal, not the double of the `f32`. Rounded to `f32`
/// again they are the kernel's values, so both engines spell one key.
package enum TextAnswerKey {
    private static func put<T>(_ value: T, _ out: inout [UInt8]) {
        withUnsafeBytes(of: value) { out.append(contentsOf: $0) }
    }
    private static func fields(_ size: Double, _ weight: Int32, _ family: Int32, _ italic: Bool,
                               _ lineHeight: Double?, _ spacing: Double, _ numeric: Int32, _ stretch: Double,
                               _ variations: [FontAxis], _ out: inout [UInt8]) {
        put(size.bitPattern, &out); put(weight, &out); put(family, &out)
        out.append(italic ? 1 : 0); out.append(lineHeight == nil ? 0 : 1)
        put((lineHeight ?? 0).bitPattern, &out); put(spacing.bitPattern, &out); put(numeric, &out)
        put(stretch.bitPattern, &out)
        put(Int32(variations.count), &out)
        for v in variations { put(v.tag, &out); put(v.value.bitPattern, &out) }
    }
    private static func paragraph(_ align: Int, _ clamp: Int, _ wrap: Int, _ direction: Int, _ space: Int,
                                  _ indent: Double, _ hyphens: Int, _ out: inout [UInt8]) {
        put(Int32(truncatingIfNeeded: align), &out); put(Int32(truncatingIfNeeded: clamp), &out)
        put(Int32(truncatingIfNeeded: wrap), &out); put(Int32(truncatingIfNeeded: direction), &out)
        put(Int32(truncatingIfNeeded: space), &out)
        put(Int32(bitPattern: Float(indent).bitPattern), &out); put(Int32(truncatingIfNeeded: hyphens), &out)
    }
    /// An image's box (LLP 1011.002), as a spec's run and a request's both carry it.
    private static func box(_ width: Double, _ height: Double, _ align: Int, _ shift: Double, _ out: inout [UInt8]) {
        out.append(3); put(width.bitPattern, &out); put(height.bitPattern, &out)
        put(Int32(truncatingIfNeeded: align), &out); put(shift.bitPattern, &out)
    }
    private static func finish(_ out: [UInt8]) -> Int {
        var h = Hasher()
        out.withUnsafeBytes { h.combine(bytes: $0) }
        return h.finalize()
    }
    /// The key of `spec`'s geometry (its paint is no part of it) and its hash.
    package static func write(_ spec: Spec, into out: inout [UInt8]) -> Int {
        out.removeAll(keepingCapacity: true)
        put(Int32(truncatingIfNeeded: spec.runs.count), &out)
        for r in spec.runs {
            var text = r.text
            text.withUTF8 { bytes in
                put(Int32(truncatingIfNeeded: bytes.count), &out)
                out.append(contentsOf: bytes)
            }
            fields(single(r.size), Int32(truncatingIfNeeded: r.weight), Int32(truncatingIfNeeded: r.family), r.italic,
                   r.lineHeight.map(single), single(r.letterSpacing), Int32(truncatingIfNeeded: r.numeric),
                   single(r.stretch), r.variations, &out)
            // A list item's lines start at its indent (LLP 1045 D4): only an
            // expanded Markdown spec has one, never a request's plain runs.
            if r.indent != 0 || r.hang { out.append(r.hang ? 2 : 1); put(single(r.indent).bitPattern, &out) }
            if let b = r.box { box(single(b.width), single(b.height), b.align, single(b.shift), &out) }
        }
        out.append(spec.strut == nil ? 0 : 1)
        if let s = spec.strut {
            fields(single(s.size), Int32(truncatingIfNeeded: s.weight), Int32(truncatingIfNeeded: s.family), s.italic,
                   s.lineHeight.map(single), single(s.letterSpacing), Int32(truncatingIfNeeded: s.numeric),
                   single(s.stretch), s.variations, &out)
        }
        paragraph(spec.align, spec.lineClamp, spec.overflowWrap, spec.direction, spec.whiteSpace,
                  single(spec.textIndent), spec.hyphens, &out)
        if spec.automaticHyphens { out.append(contentsOf: Array(spec.language.utf8)) }
        // CSS `text-box`: a trimmed paragraph's height and baselines differ.
        out.append(contentsOf: [UInt8(truncatingIfNeeded: spec.textBoxTrim), UInt8(truncatingIfNeeded: spec.textBoxOver), UInt8(truncatingIfNeeded: spec.textBoxUnder)])
        out.append(spec.balance ? 1 : 0)
        put(UInt32(spec.emergencyBreaks.count), &out)
        for point in spec.emergencyBreaks { put(UInt32(point), &out) }
        return finish(out)
    }
    /// The same bytes from the kernel's request, before any `String` is made
    /// of it. A request always carries a strut. Text that is not UTF-8 keys
    /// apart from its decoded `Spec` (whose replacement characters differ),
    /// so such a request is never answered from here.
    package static func write(_ request: ExactMeasureRequest, into out: inout [UInt8]) -> Int {
        out.removeAll(keepingCapacity: true)
        func run(_ r: ExactTextRun) {
            fields(Double(r.font_size), Int32(r.font_weight), Int32(r.font_family), r.italic != 0,
                   r.has_line_height != 0 ? Double(r.line_height) : nil, Double(r.letter_spacing),
                   Int32(r.font_variant_numeric), Double(r.font_stretch), FontAxis.of(r), &out)
        }
        put(Int32(truncatingIfNeeded: request.count), &out)
        for r in UnsafeBufferPointer(start: request.runs, count: request.count) {
            put(Int32(truncatingIfNeeded: r.len), &out)
            out.append(contentsOf: UnsafeBufferPointer(start: r.text, count: r.len))
            run(r)
            if r.inline_box != 0 { box(Double(r.box_width), Double(r.box_height), Int(r.box_align), Double(r.box_shift), &out) }
        }
        out.append(1)
        run(request.strut)
        paragraph(Int(request.align), Int(request.line_clamp), Int(request.overflow_wrap), Int(request.direction),
                  Int(request.white_space), Double(request.text_indent), Int(request.hyphens), &out)
        if Spec.insertsHyphens(Int(request.hyphens), wrap: Int(request.overflow_wrap)), let lang = request.lang { out.append(contentsOf: UnsafeBufferPointer(start: lang, count: request.lang_len)) }
        out.append(contentsOf: [request.text_box_trim, request.text_box_over, request.text_box_under, request.text_wrap_style])
        put(UInt32(0), &out)
        return finish(out)
    }
    /// A length at the kernel's precision.
    private static func single(_ value: CGFloat) -> Double { Double(Float(value)) }
    /// An offer's bits: a definite width's own, or one of two that no width
    /// has. The kernel offers a height too, and it is no part of the key:
    /// no measurer on this host reads it (a paragraph's height is its
    /// lines', which its width, clamp and wrapping decide), as the
    /// residency's scalars never keyed it either.
    package static func offer(width: CGFloat) -> UInt64 { let w = single(width); return (w == 0 ? 0 : w).bitPattern }
    package static let minContent: UInt64 = 0x7FF8_0000_0000_0001, maxContent: UInt64 = 0x7FF8_0000_0000_0002
}

/// One session's answers, least recently used first out, bounded by bytes.
/// An answer is what typesetting the paragraph at that offer gives, so one
/// that is gone is only typeset again: nothing painted depends on what is
/// here. A font catalog change empties it (`TextEngine.dropMeasuredBreaks`).
package final class TextAnswers: @unchecked Sendable {
    package struct Stats: Equatable { package var paragraphs = 0, offers = 0, bytes = 0 }
    private struct Offer {
        let bits: UInt64
        let metrics: ExactMetrics
        /// A definite width's line breaks; an intrinsic offer has none.
        let lines: LineGeometry?
        var glyphs: MeasuredGlyphs?
    }
    private final class Entry {
        let hash: Int
        let key: [UInt8]
        var offers: [Offer] = []
        /// Replaced in turn past `TextAnswers.offersPerParagraph`.
        var next = 0
        var bytes = 0
        var glyphBytes = 0
        var olderGlyph: Entry?
        unowned(unsafe) var newerGlyph: Entry?
        var older: Entry?
        unowned(unsafe) var newer: Entry?
        /// The next entry whose key hashes the same.
        var chained: Entry?
        /// Its own copy of the key, no larger than its bytes: the caller's
        /// buffer is reused for every lookup and is as large as its longest.
        init(hash: Int, key: [UInt8]) { self.hash = hash; self.key = key.withUnsafeBufferPointer { Array($0) } }
    }
    /// A layout pass offers one paragraph several sizes (LLP 1044 F6).
    package static let offersPerParagraph = 8
    /// About 3,500 paragraphs of a sentence or two, each offered two or
    /// three sizes: a thousand-row list's travel out and back.
    package static let defaultLimit = 5 * 512 * 1024
    package let limit: Int
    private let lock = NSLock()
    private var buckets: [Int: Entry] = [:]
    private var oldest: Entry?, newest: Entry?
    private var oldestGlyph: Entry?, newestGlyph: Entry?
    private var glyphBytes = 0
    // Glyphs save one paint, so they occupy at most a sixteenth of the answer budget.
    private var glyphLimit: Int { min(256 * 1024, limit / 16) }
    private var stats = Stats()

    package init(limit: Int = TextAnswers.defaultLimit) { self.limit = limit }

    package var observation: Stats { lock.lock(); defer { lock.unlock() }; return stats }

    private func find(_ hash: Int, _ key: [UInt8]) -> Entry? {
        var entry = buckets[hash]
        while let e = entry {
            if e.key == key { return e }
            entry = e.chained
        }
        return nil
    }
    private func unlink(_ e: Entry) {
        if let newer = e.newer { newer.older = e.older } else { newest = e.older }
        if let older = e.older { older.newer = e.newer } else { oldest = e.newer }
        e.older = nil; e.newer = nil
    }
    private func append(_ e: Entry) {
        e.older = newest; e.newer = nil
        if let newest { newest.newer = e } else { oldest = e }
        newest = e
    }
    private func touch(_ e: Entry) {
        guard newest !== e else { return }
        unlink(e); append(e)
    }
    private func remove(_ e: Entry) {
        if e.glyphBytes > 0 { dropGlyphs(e) }
        unlink(e)
        if buckets[e.hash] === e { buckets[e.hash] = e.chained } else {
            var link = buckets[e.hash]
            while let l = link, l.chained !== e { link = l.chained }
            link?.chained = e.chained
        }
        e.chained = nil
        stats.paragraphs -= 1; stats.offers -= e.offers.count; stats.bytes -= e.bytes
    }
    /// What an entry costs the heap before its key's bytes and its offers:
    /// the object, two array headers, a bucket.
    private static let entryBytes = 104 + 32 + 32 + 32
    /// A definite offer's two arrays; its place in the entry's array is
    /// counted with that array's capacity.
    private static func cost(_ offer: Offer) -> Int {
        (offer.glyphs?.bytes ?? 0) + (offer.lines.map { 64 + (($0.ranges.count * (MemoryLayout<CFRange>.stride + MemoryLayout<CGFloat>.stride)) + 31) & ~31 } ?? 0)
    }

    /// What measuring the paragraph keyed `key` at `offer` answered.
    package func metrics(hash: Int, key: [UInt8], offer: UInt64) -> ExactMetrics? {
        lock.lock(); defer { lock.unlock() }
        guard let e = find(hash, key), let hit = e.offers.first(where: { $0.bits == offer }) else { return nil }
        touch(e)
        return hit.metrics
    }
    /// Where that paragraph's lines break at a definite width.
    package func lines(hash: Int, key: [UInt8], width: CGFloat) -> LineGeometry? {
        let offer = TextAnswerKey.offer(width: width)
        lock.lock(); defer { lock.unlock() }
        guard let e = find(hash, key), let hit = e.offers.first(where: { $0.bits == offer }) else { return nil }
        touch(e)
        return hit.lines
    }
    /// Transfer the optional glyph copy to one raster job, retaining scalar answers.
    package func takeGlyphs(hash: Int, key: [UInt8], width: CGFloat) -> MeasuredGlyphs? {
        let offer = TextAnswerKey.offer(width: width)
        lock.lock(); defer { lock.unlock() }
        guard let e = find(hash, key), let i = e.offers.firstIndex(where: { $0.bits == offer }),
              let glyphs = e.offers[i].glyphs else { return nil }
        if e.glyphBytes == glyphs.bytes { unlinkGlyphs(e) }
        e.offers[i].glyphs = nil
        e.glyphBytes -= glyphs.bytes; glyphBytes -= glyphs.bytes
        e.bytes -= glyphs.bytes; stats.bytes -= glyphs.bytes
        touch(e)
        return glyphs
    }
    package func discardGlyphs() {
        lock.lock(); defer { lock.unlock() }
        dropGlyphs()
    }
    private func unlinkGlyphs(_ e: Entry) {
        if let newer = e.newerGlyph { newer.olderGlyph = e.olderGlyph } else { newestGlyph = e.olderGlyph }
        if let older = e.olderGlyph { older.newerGlyph = e.newerGlyph } else { oldestGlyph = e.newerGlyph }
        e.olderGlyph = nil; e.newerGlyph = nil
    }
    private func appendGlyphs(_ e: Entry) {
        e.olderGlyph = newestGlyph
        if let newestGlyph { newestGlyph.newerGlyph = e } else { oldestGlyph = e }
        newestGlyph = e
    }
    private func dropGlyphs(_ e: Entry, keeping offer: UInt64? = nil) {
        unlinkGlyphs(e)
        // At most eight offers; entries without glyphs never enter this list.
        for i in e.offers.indices where e.offers[i].bits != offer {
            if let glyphs = e.offers[i].glyphs {
                e.bytes -= glyphs.bytes; stats.bytes -= glyphs.bytes
                e.glyphBytes -= glyphs.bytes; glyphBytes -= glyphs.bytes
                e.offers[i].glyphs = nil
            }
        }
        if e.glyphBytes > 0 { appendGlyphs(e) }
    }
    private func dropGlyphs() {
        while let e = oldestGlyph { dropGlyphs(e) }
    }
    /// The measurer's answer, published once per offer.
    package func put(hash: Int, key: [UInt8], offer: UInt64, metrics: ExactMetrics, lines: LineGeometry?, glyphs: MeasuredGlyphs? = nil) {
        guard limit > 0 else { return }
        lock.lock(); defer { lock.unlock() }
        let e: Entry
        if let found = find(hash, key) {
            e = found
            touch(e)
            if e.offers.contains(where: { $0.bits == offer }) { return }
        } else {
            e = Entry(hash: hash, key: key)
            // The entry, its key's and offers' arrays, its place in the table.
            e.bytes = Self.entryBytes + (key.count + 15) & ~15
            e.chained = buckets[hash]
            buckets[hash] = e
            append(e)
            stats.paragraphs += 1; stats.bytes += e.bytes
        }
        let kept = glyphs.flatMap { $0.bytes <= glyphLimit ? $0 : nil }
        let value = Offer(bits: offer, metrics: metrics, lines: lines, glyphs: kept)
        var cost = Self.cost(value), glyphCost = kept?.bytes ?? 0
        if e.glyphBytes > 0 { unlinkGlyphs(e) }
        if e.offers.count < Self.offersPerParagraph {
            let held = e.offers.capacity
            e.offers.append(value)
            cost += (e.offers.capacity - held) * MemoryLayout<Offer>.stride
            stats.offers += 1
        } else {
            cost -= Self.cost(e.offers[e.next])
            glyphCost -= e.offers[e.next].glyphs?.bytes ?? 0
            e.offers[e.next] = value
            e.next = (e.next + 1) % Self.offersPerParagraph
        }
        e.bytes += cost; stats.bytes += cost
        e.glyphBytes += glyphCost; glyphBytes += glyphCost
        if e.glyphBytes > 0 { appendGlyphs(e) }
        while glyphBytes > glyphLimit, let victim = oldestGlyph {
            dropGlyphs(victim, keeping: victim === e ? offer : nil)
        }
        while stats.bytes > limit, let victim = oldest, victim !== e { remove(victim) }
    }
    package func clear() {
        lock.lock(); defer { lock.unlock() }
        dropGlyphs()
        // Unlinked one by one: a long chain of `older` links released at
        // once would recurse through every entry.
        while let e = oldest { unlink(e); e.chained = nil }
        buckets.removeAll(keepingCapacity: true)
        stats = Stats()
    }
    deinit { dropGlyphs(); while let e = oldest { unlink(e); e.chained = nil } }
}
