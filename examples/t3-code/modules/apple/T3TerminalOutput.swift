import Foundation

/// A terminal session's retained output: T3 Code 1e2ecbd975's output buffer (MIT reference, see
/// LICENSE-T3: packages/client-runtime/src/state/terminalOutput.ts), ported to Swift because the drawer's
/// bytes never pass the TypeScript inbox (they go from the socket to this buffer to the web view).
/// Changes from the reference: a value type with mutating `append` / `reset` instead of functions that
/// return a new state; offsets count UTF-16 units as the reference's string offsets do, so the shared
/// vectors (macos/tests/terminal/vectors.json, also read by terminal-output.test.ts) hold for both.
/// 512 KiB retained, 16 KiB chunks, at most 1,024 chunks; trimming never cuts a code point.
struct T3TerminalOutput {
    struct Chunk {
        /// UTF-16 offset within this generation and reset.
        var startOffset: Int
        var data: String
        var length: Int // UTF-16 units
        var byteLength: Int
    }
    struct Cursor: Equatable {
        var generation: Int
        var resetVersion: Int
        var offset: Int
        /// Forces the first read to resynchronize from a reset.
        static let initial = Cursor(generation: -1, resetVersion: -1, offset: 0)
    }
    enum Update: Equatable {
        case none
        case reset(String)
        case append(String)
    }

    static let defaultMaxBytes = 512 * 1024
    static let chunkBytes = 16 * 1024
    static let maxChunks = 1_024

    var generation = 0
    private(set) var chunks: [Chunk] = []
    private(set) var retainedBytes = 0
    private(set) var resetVersion = 0
    private(set) var nextOffset = 0

    var text: String { chunks.map(\.data).joined() }

    /// Split into chunks of at most `maxBytes` UTF-8 bytes without cutting a code point.
    private static func split(_ data: String, maxBytes: Int) -> [(data: String, length: Int, bytes: Int)] {
        if data.isEmpty { return [] }
        let encoded = Array(data.utf8)
        if encoded.count <= maxBytes { return [(data, data.utf16.count, encoded.count)] }
        var result: [(String, Int, Int)] = []
        var offset = 0
        while offset < encoded.count {
            var end = min(offset + maxBytes, encoded.count)
            while end < encoded.count, encoded[end] & 0xC0 == 0x80 { end -= 1 }
            // A budget smaller than one code point still advances by the whole code point.
            if end == offset {
                end = min(offset + maxBytes, encoded.count)
                while end < encoded.count, encoded[end] & 0xC0 == 0x80 { end += 1 }
            }
            let piece = String(decoding: encoded[offset..<end], as: UTF8.self)
            result.append((piece, piece.utf16.count, end - offset))
            offset = end
        }
        return result
    }

    private static func trim(_ buffer: String, maxBytes: Int) -> String {
        if maxBytes <= 0 { return "" }
        let encoded = Array(buffer.utf8)
        if encoded.count <= maxBytes { return buffer }
        var start = encoded.count - maxBytes
        while start < encoded.count, encoded[start] & 0xC0 == 0x80 { start += 1 }
        return String(decoding: encoded[start...], as: UTF8.self)
    }

    private static func chunks(_ data: String, from firstOffset: Int, maxBytes: Int) -> (chunks: [Chunk], nextOffset: Int, bytes: Int) {
        var next = firstOffset, bytes = 0
        let result = split(data, maxBytes: maxBytes).map { piece -> Chunk in
            defer { next += piece.length; bytes += piece.bytes }
            return Chunk(startOffset: next, data: piece.data, length: piece.length, byteLength: piece.bytes)
        }
        return (result, next, bytes)
    }

    /// Merge adjacent chunks without changing their positions.
    private static func compact(_ chunks: [Chunk]) -> [Chunk] {
        var compacted: [Chunk] = []
        for chunk in chunks {
            if let previous = compacted.last, previous.startOffset + previous.length == chunk.startOffset,
               previous.byteLength + chunk.byteLength <= chunkBytes {
                compacted[compacted.count - 1] = Chunk(startOffset: previous.startOffset, data: previous.data + chunk.data,
                                                       length: previous.length + chunk.length, byteLength: previous.byteLength + chunk.byteLength)
            } else {
                compacted.append(chunk)
            }
        }
        return compacted
    }

    /// Drop the first `bytesToDrop` bytes, by whole code points.
    private static func trimStart(_ chunk: Chunk, dropping bytesToDrop: Int) -> Chunk {
        var units = 0, dropped = 0
        var index = chunk.data.unicodeScalars.startIndex
        while dropped < bytesToDrop, index < chunk.data.unicodeScalars.endIndex {
            let value = chunk.data.unicodeScalars[index].value
            dropped += value <= 0x7F ? 1 : value <= 0x7FF ? 2 : value <= 0xFFFF ? 3 : 4
            units += value <= 0xFFFF ? 1 : 2
            index = chunk.data.unicodeScalars.index(after: index)
        }
        let rest = String(chunk.data.unicodeScalars[index...])
        return Chunk(startOffset: chunk.startOffset + units, data: rest, length: chunk.length - units, byteLength: chunk.byteLength - dropped)
    }

    mutating func append(_ data: String, maxBytes: Int = defaultMaxBytes) {
        if data.isEmpty { return }
        if maxBytes <= 0 {
            chunks = []; retainedBytes = 0; resetVersion += 1; nextOffset += data.utf16.count
            return
        }
        let appended = Self.chunks(data, from: nextOffset, maxBytes: min(Self.chunkBytes, max(1, maxBytes)))
        var all = chunks + appended.chunks
        var retained = retainedBytes + appended.bytes
        var first = 0
        while retained > maxBytes, first < all.count {
            let head = all[first], toDrop = retained - maxBytes
            if toDrop < head.byteLength {
                let trimmed = Self.trimStart(head, dropping: toDrop)
                retained -= head.byteLength - trimmed.byteLength
                if trimmed.byteLength > 0 { all[first] = trimmed } else { first += 1 }
                break
            }
            retained -= head.byteLength
            first += 1
        }
        var kept = first == 0 ? all : Array(all[first...])
        if kept.count > Self.maxChunks {
            kept = Self.compact(kept)
            let excess = kept.count - Self.maxChunks
            if excess > 0 {
                for chunk in kept[0..<excess] { retained -= chunk.byteLength }
                kept = Array(kept[excess...])
            }
        }
        chunks = kept; retainedBytes = retained; nextOffset = appended.nextOffset
    }

    mutating func reset(_ data: String, maxBytes: Int = defaultMaxBytes) {
        let retained = Self.trim(data, maxBytes: maxBytes)
        let reset = Self.chunks(retained, from: 0, maxBytes: min(Self.chunkBytes, max(1, maxBytes)))
        chunks = reset.chunks; retainedBytes = reset.bytes; resetVersion += 1; nextOffset = reset.nextOffset
    }

    func read(from cursor: Cursor) -> (update: Update, cursor: Cursor) {
        let next = Cursor(generation: generation, resetVersion: resetVersion, offset: nextOffset)
        if cursor.generation != generation || cursor.resetVersion != resetVersion || cursor.offset < (chunks.first?.startOffset ?? nextOffset) {
            return (.reset(text), next)
        }
        let appended = chunks.filter { $0.startOffset + $0.length > cursor.offset }
        if appended.isEmpty { return (.none, next) }
        let data = appended.map { chunk -> String in
            let skip = max(0, cursor.offset - chunk.startOffset)
            return skip == 0 ? chunk.data : String(decoding: Array(chunk.data.utf16.dropFirst(skip)), as: UTF16.self)
        }.joined()
        return (.append(data), next)
    }
}
