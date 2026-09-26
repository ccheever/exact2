// @ref LLP 1044.000 §6 S8 — the unchanged Apple JSON wire, read as UTF-8.
// One cursor, no Foundation containers or speculative type decoding. Errors
// leave only through the batch boundary. Strings own their bytes before return.
import Foundation

struct BatchReader {
    enum Invalid: Error { case wire }
    let bytes: UnsafeBufferPointer<UInt8>
    var offset = 0
    var depth = 0

    @inline(__always) var byte: UInt8 { offset < bytes.count ? bytes[offset] : 0 }
    @inline(__always) mutating func whitespace() {
        while offset < bytes.count {
            let b = bytes[offset]
            if b != 32 && b != 10 && b != 13 && b != 9 { break }
            offset += 1
        }
    }
    @inline(__always) mutating func take(_ b: UInt8) -> Bool {
        whitespace()
        if byte == b && offset < bytes.count { offset += 1; return true }
        return false
    }
    @inline(__always) mutating func expect(_ b: UInt8) throws {
        guard take(b) else { throw Invalid.wire }
    }
    mutating func end() throws {
        whitespace()
        guard offset == bytes.count else { throw Invalid.wire }
    }
    mutating func literal(_ text: StaticString) throws {
        whitespace()
        let matches = text.withUTF8Buffer { word in
            guard word.count <= bytes.count - offset else { return false }
            for b in word { guard bytes[offset] == b else { return false }; offset += 1 }
            return true
        }
        guard matches else { throw Invalid.wire }
    }
    @inline(__always) mutating func null() throws -> Bool {
        whitespace()
        if byte != 110 { return false }
        try literal("null"); return true
    }
    mutating func bool() throws -> Bool {
        whitespace()
        if byte == 116 { try literal("true"); return true }
        try literal("false"); return false
    }

    mutating func string() throws -> String {
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
    mutating func numberSpan() throws -> Range<Int> {
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
    func number(_ span: Range<Int>) throws -> Double {
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
    mutating func number() throws -> Double { try number(numberSpan()) }
    mutating func id() throws -> UInt32 {
        let span = try numberSpan()
        // Foundation also accepts integral decimal/exponent spellings.
        guard let n = UInt32(exactly: try number(span)) else { throw Invalid.wire }
        return n
    }
    mutating func enter(_ delimiter: UInt8) throws {
        try expect(delimiter)
        depth += 1
        guard depth <= 512 else { throw Invalid.wire }
    }
    mutating func object(_ field: (inout BatchReader, String) throws -> Void) throws {
        try enter(123)
        if take(125) { depth -= 1; return }
        repeat {
            let key = try string(); try expect(58)
            try field(&self, key)
            if take(125) { depth -= 1; return }
            try expect(44)
        } while true
    }
    mutating func array<T>(_ element: (inout BatchReader) throws -> T) throws -> [T] {
        try enter(91)
        var values: [T] = []
        if take(93) { depth -= 1; return values }
        repeat {
            values.append(try element(&self))
            if take(93) { depth -= 1; return values }
            try expect(44)
        } while true
    }
    mutating func strings() throws -> [String: String] {
        var values: [String: String] = [:]
        try object { r, key in values[key] = try r.string() }
        return values
    }
    mutating func values() throws -> NodeStyle {
        var values: NodeStyle = [:]
        try object { r, key in values[key] = try r.value() }
        return values
    }
    mutating func value() throws -> BatchValue {
        whitespace()
        switch byte {
        case 34: return .string(try string())
        case 123: return .object(try values())
        case 91: return .array(try array { try $0.value() })
        case 116, 102: return .bool(try bool())
        case 110: try literal("null"); return .null
        default: return .number(try number())
        }
    }
    // Unknown fields are validated without materializing arrays/dictionaries or
    // converting their numbers (Foundation likewise ignores out-of-range values).
    mutating func skip() throws {
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
