// @ref LLP 1044.000 §6 S8 — the unchanged Apple JSON wire, read as UTF-8.
// One cursor, no Foundation containers or speculative type decoding. Errors
// leave only through the batch boundary. Strings own their bytes before return.
import Foundation

package struct BatchReader {
    package enum Invalid: Error { case wire }
    package let bytes: UnsafeBufferPointer<UInt8>
    package var offset = 0
    package var depth = 0
    package var colorChannels: ((BatchValue, Bool) -> [Double]?)? = nil
    package var profileResolver: AssetResolver? = nil
    /// While `style` decodes a style it will cache, the depth of the
    /// style's own values: those that are arrays or objects come from
    /// `StyleCache` by their bytes (at that depth only, so no byte is hashed
    /// twice), and its keys and long strings at any depth are shared.
    private var sharing: Int? = nil

    package init(bytes: UnsafeBufferPointer<UInt8>, colorChannels: ((BatchValue, Bool) -> [Double]?)? = nil, profileResolver: AssetResolver? = nil) {
        self.bytes = bytes; self.colorChannels = colorChannels; self.profileResolver = profileResolver
    }

    @inline(__always) package var byte: UInt8 { offset < bytes.count ? bytes[offset] : 0 }
    @inline(__always) package mutating func whitespace() {
        while offset < bytes.count {
            let b = bytes[offset]
            if b != 32 && b != 10 && b != 13 && b != 9 { break }
            offset += 1
        }
    }
    @inline(__always) package mutating func take(_ b: UInt8) -> Bool {
        whitespace()
        if byte == b && offset < bytes.count { offset += 1; return true }
        return false
    }
    @inline(__always) package mutating func expect(_ b: UInt8) throws {
        guard take(b) else { throw Invalid.wire }
    }
    package mutating func end() throws {
        whitespace()
        guard offset == bytes.count else { throw Invalid.wire }
    }
    package mutating func literal(_ text: StaticString) throws {
        whitespace()
        let matches = text.withUTF8Buffer { word in
            guard word.count <= bytes.count - offset else { return false }
            for b in word { guard bytes[offset] == b else { return false }; offset += 1 }
            return true
        }
        guard matches else { throw Invalid.wire }
    }
    @inline(__always) package mutating func null() throws -> Bool {
        whitespace()
        if byte != 110 { return false }
        try literal("null"); return true
    }
    package mutating func bool() throws -> Bool {
        whitespace()
        if byte == 116 { try literal("true"); return true }
        try literal("false"); return false
    }

    package mutating func string() throws -> String {
        try expect(34)
        let start = offset
        var ascii = true
        while offset < bytes.count {
            let b = bytes[offset]
            if b == 34 {
                let span = UnsafeBufferPointer(rebasing: bytes[start..<offset])
                // ASCII is valid UTF-8 as it stands: the standard library's
                // decoding copies it; only other text goes through
                // Foundation's validating conversion (most of a key's cost).
                let result: String
                if ascii { result = String(decoding: span, as: UTF8.self) } else {
                    guard let text = String(bytes: span, encoding: .utf8) else { throw Invalid.wire }
                    result = text
                }
                offset += 1
                return result
            }
            if b == 92 { return try escapedString(start: start) }
            guard b >= 32 else { throw Invalid.wire }
            if b >= 128 { ascii = false }
            offset += 1
        }
        throw Invalid.wire
    }
    private mutating func escapedString(start: Int) throws -> String {
        var result = Array(bytes[start..<offset])
        while offset < bytes.count {
            let b = bytes[offset]; offset += 1
            switch b {
            case 34:
                guard let text = String(bytes: result, encoding: .utf8) else { throw Invalid.wire }
                return text
            case 92:
                guard offset < bytes.count else { throw Invalid.wire }
                let escape = bytes[offset]; offset += 1
                switch escape {
                case 34, 47, 92: result.append(escape)
                case 98: result.append(8)
                case 102: result.append(12)
                case 110: result.append(10)
                case 114: result.append(13)
                case 116: result.append(9)
                case 117:
                    var scalar = try hex4()
                    if (0xD800...0xDBFF).contains(scalar) {
                        // Whitespace is not legal inside a surrogate escape.
                        guard offset + 2 <= bytes.count, bytes[offset] == 92, bytes[offset + 1] == 117 else { throw Invalid.wire }
                        offset += 2
                        let low = try hex4()
                        guard (0xDC00...0xDFFF).contains(low) else { throw Invalid.wire }
                        scalar = 0x10000 + ((scalar - 0xD800) << 10) + low - 0xDC00
                    }
                    guard let unicode = Unicode.Scalar(scalar) else { throw Invalid.wire }
                    result.append(contentsOf: String(unicode).utf8)
                default: throw Invalid.wire
                }
            default:
                guard b >= 32 else { throw Invalid.wire }
                result.append(b)
            }
        }
        throw Invalid.wire
    }
    private mutating func hex4() throws -> UInt32 {
        guard offset + 4 <= bytes.count else { throw Invalid.wire }
        var n: UInt32 = 0
        for _ in 0..<4 {
            let b = bytes[offset]; offset += 1
            let digit: UInt8
            switch b {
            case 48...57: digit = b - 48
            case 65...70: digit = b - 55
            case 97...102: digit = b - 87
            default: throw Invalid.wire
            }
            n = n * 16 + UInt32(digit)
        }
        return n
    }

    // Validate JSON's grammar before Swift's correctly rounded conversion. An
    // integer accumulator covers the IDs and short integral style/geometry rows.
    package mutating func numberSpan() throws -> Range<Int> {
        whitespace()
        let start = offset
        if byte == 45 { offset += 1 }
        if byte == 48 { offset += 1 }
        else {
            guard (49...57).contains(byte) else { throw Invalid.wire }
            repeat { offset += 1 } while (48...57).contains(byte)
        }
        if byte == 46 {
            offset += 1
            guard (48...57).contains(byte) else { throw Invalid.wire }
            repeat { offset += 1 } while (48...57).contains(byte)
        }
        if byte == 101 || byte == 69 {
            offset += 1
            if byte == 43 || byte == 45 { offset += 1 }
            guard (48...57).contains(byte) else { throw Invalid.wire }
            repeat { offset += 1 } while (48...57).contains(byte)
        }
        return start..<offset
    }
    package func number(_ span: Range<Int>) throws -> Double {
        var integer: UInt64 = 0
        let negative = bytes[span.lowerBound] == 45
        let first = span.lowerBound + (negative ? 1 : 0)
        if span.upperBound - first <= 15 {
            var integral = true
            for i in first..<span.upperBound {
                let b = bytes[i]
                if b < 48 || b > 57 { integral = false; break }
                integer = integer * 10 + UInt64(b - 48)
            }
            if integral { return negative ? -Double(integer) : Double(integer) }
        }
        guard let value = Double(String(decoding: bytes[span], as: UTF8.self)), value.isFinite else { throw Invalid.wire }
        // Foundation refuses a nonzero literal that rounds to zero, as it does one past the largest.
        if value == 0, bytes[span].prefix(while: { $0 != 101 && $0 != 69 }).contains(where: { (49...57).contains($0) }) {
            throw Invalid.wire
        }
        return value
    }
    package mutating func number() throws -> Double { try number(numberSpan()) }
    package mutating func id() throws -> UInt32 {
        let span = try numberSpan()
        // Foundation also accepts integral decimal/exponent spellings.
        guard let n = UInt32(exactly: try number(span)) else { throw Invalid.wire }
        return n
    }
    package mutating func enter(_ delimiter: UInt8) throws {
        try expect(delimiter)
        depth += 1
        guard depth <= 512 else { throw Invalid.wire }
    }
    package mutating func object(_ field: (inout BatchReader, String) throws -> Void) throws {
        try enter(123)
        if take(125) { depth -= 1; return }
        repeat {
            let key = try string(); try expect(58)
            try field(&self, key)
            if take(125) { depth -= 1; return }
            try expect(44)
        } while true
    }
    package mutating func array<T>(_ element: (inout BatchReader) throws -> T) throws -> [T] {
        try enter(91)
        var values: [T] = []
        if take(93) { depth -= 1; return values }
        repeat {
            values.append(try element(&self))
            if take(93) { depth -= 1; return values }
            try expect(44)
        } while true
    }
    package mutating func strings() throws -> [String: String] {
        var values: [String: String] = [:]
        try object { r, key in values[key] = try r.string() }
        return values
    }
    package mutating func values() throws -> NodeStyle {
        var values: NodeStyle = [:]
        try object { r, key in values[r.sharing != nil ? StyleCache.strings.share(key) : key] = try r.value() }
        return values
    }
    /// A node's style dictionary, shared with every earlier one of the same
    /// bytes: a list's rows repeat their template's styles, and building a
    /// dictionary of boxed values was most of a row's decode. The span is
    /// validated and found by `skip` (nothing materialized), then looked up.
    package mutating func style() throws -> NodeStyle {
        whitespace()
        let start = offset
        try skip()
        let span = UnsafeRawBufferPointer(rebasing: UnsafeRawBufferPointer(bytes)[start..<offset])
        // Raw ICC paths are generation-relative; resolve them on every
        // decode. An escape can spell one out (`\u0069cc:`) that the raw
        // bytes do not show, so a style with an escape is not cached either.
        let hasICC = span.count >= 4 && (0..<(span.count - 3)).contains {
            span[$0] == 105 && span[$0 + 1] == 99 && span[$0 + 2] == 99 && span[$0 + 3] == 58
        }
        let cached = !hasICC && !span.contains(92)
        if cached, let hit = StyleCache.styles.get(span) { return hit }
        let end = offset
        offset = start
        sharing = cached ? depth + 1 : nil
        defer { sharing = nil }
        let style = try CalculatedLength.decode(values())
        guard offset == end else { throw Invalid.wire }
        if cached { StyleCache.styles.put(span, style) }
        return style
    }
    package mutating func value() throws -> BatchValue {
        whitespace()
        if let level = sharing, depth == level, byte == 123 || byte == 91 { return try sharedValue() }
        return try plainValue()
    }
    /// An object or array in a style, as the earlier decode of the same
    /// bytes: styles that differ still repeat their colours, lengths and
    /// shadows, which would otherwise each be an array or a dictionary of
    /// their own (a colour's four channels are a 96-byte block).
    private mutating func sharedValue() throws -> BatchValue {
        let start = offset
        try skip()
        let span = UnsafeRawBufferPointer(rebasing: UnsafeRawBufferPointer(bytes)[start..<offset])
        if let hit = StyleCache.values.get(span) { return hit }
        let end = offset
        offset = start
        let value = try plainValue()
        guard offset == end else { throw Invalid.wire }
        StyleCache.values.put(span, value)
        return value
    }
    private mutating func plainValue() throws -> BatchValue {
        switch byte {
        case 34:
            let text = try string()
            return .string(sharing != nil ? StyleCache.strings.share(text) : text)
        case 123:
            var object = try values()
            if let name = object["s"]?.string, name.hasPrefix("icc:"),
               let handle = ProfileSpaces.bind(name, resolver: profileResolver) {
                object["s"] = .profile(handle)
            }
            return .object(object)
        case 91: return .array(try array { try $0.value() })
        case 116, 102: return .bool(try bool())
        case 110: try literal("null"); return .null
        default: return .number(try number())
        }
    }
    // Unknown fields are validated without materializing arrays/dictionaries or
    // converting their numbers (Foundation likewise ignores out-of-range values).
    package mutating func skip() throws {
        whitespace()
        switch byte {
        case 123: try object { r, _ in try r.skip() }
        case 91:
            try enter(91)
            if !take(93) {
                repeat {
                    try skip()
                    if take(93) { break }
                    try expect(44)
                } while true
            }
            depth -= 1
        case 34: _ = try string()
        case 116, 102: _ = try bool()
        case 110: try literal("null")
        default: _ = try numberSpan()
        }
    }
}

/// What `BatchReader.style` keeps: whole style dictionaries by their exact
/// bytes, the arrays and objects inside them by theirs, and one copy of each
/// long string they hold.
package enum StyleCache {
    package static let styles = SpanCache<NodeStyle>()
    package static let values = SpanCache<BatchValue>()
    package static let strings = SharedStrings()
}

/// Decoded values by their exact bytes. An entry keeps the bytes' length and two independent hashes of them, not
/// the bytes: FNV-1a 64 finds it and the process-seeded SipHash of `Hasher`
/// confirms it. Equality is therefore probabilistic: on 64-bit targets two
/// different styles of one length pass both by a chance too small to matter
/// for the host's own style text (it is no defence against crafted input). A
/// copy of every style's bytes (the host's mirrors keep the same texts) was
/// as large as the dictionaries. Bounded: when full it starts over. Batches
/// decode on more than one thread (a session's executor, the main thread),
/// so a lock guards it.
package final class SpanCache<Value>: @unchecked Sendable {
    private struct Entry { let count: Int; let check: Int; let value: Value }
    private var entries: [UInt64: Entry] = [:]
    private let lock = NSLock()
    private let capacity = 1024

    private static func hash(_ b: UnsafeRawBufferPointer) -> UInt64 {
        var h: UInt64 = 0xcbf2_9ce4_8422_2325
        for byte in b { h = (h ^ UInt64(byte)) &* 0x0000_0100_0000_01b3 }
        return h
    }
    private static func check(_ b: UnsafeRawBufferPointer) -> Int {
        var h = Hasher()
        h.combine(bytes: b)
        return h.finalize()
    }
    package func get(_ b: UnsafeRawBufferPointer) -> Value? {
        let h = Self.hash(b)
        lock.lock()
        let found = entries[h]
        lock.unlock()
        guard let e = found, e.count == b.count, e.check == Self.check(b) else { return nil }
        return e.value
    }
    package func put(_ b: UnsafeRawBufferPointer, _ value: Value) {
        guard b.count > 2 else { return }
        let entry = Entry(count: b.count, check: Self.check(b), value: value)
        let h = Self.hash(b)
        lock.lock(); defer { lock.unlock() }
        if entries.count >= capacity { entries.removeAll(keepingCapacity: true) }
        entries[h] = entry
    }
}

/// One copy of each long string the styles hold (a key such as
/// `border_radius_top_left`, a font family): Swift keeps a string of up to
/// 15 UTF-8 bytes inline and allocates storage for each longer one. Bounded
/// as `SpanCache` is.
package final class SharedStrings: @unchecked Sendable {
    private var strings: Set<String> = []
    private let lock = NSLock()

    /// `s`, or the earlier copy of it. Only ASCII: Swift's `==` also equates
    /// canonically equivalent Unicode spellings, which are different bytes.
    package func share(_ s: String) -> String {
        guard s.utf8.count > 15, s.utf8.allSatisfy({ $0 < 128 }) else { return s }
        lock.lock(); defer { lock.unlock() }
        if strings.count >= 1024, !strings.contains(s) { strings.removeAll(keepingCapacity: true) }
        return strings.insert(s).memberAfterInsert
    }
}
