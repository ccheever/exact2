// The first frame of an animated PNG, as a still PNG (LLP 1133.006 D1).
// Chromium's clipboard writer decodes frame 0 of the animation, and skips a
// default image (the `IDAT`) that is not part of it. ImageIO does not: for
// such a file its image at index 0 is not frame 0 (measured on macOS 26:
// the last frame's pixels). So the frame is cut out of the chunk stream
// here and decoded as a still image: the chunks before the image data (the
// header, a palette, colour information), then frame 0's data, as `IDAT`.
//
// Frame 0 covers the whole canvas (APNG: "the first frame … must be equal
// to the canvas"), so the header stays as it is.
import Foundation

enum PNGFrame {
    static let signature: [UInt8] = [0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A]

    struct Chunk { let type: String; let data: Data }

    /// The chunks of `png`, or nil when it is not a PNG.
    static func chunks(_ png: Data) -> [Chunk]? {
        let bytes = [UInt8](png)
        guard bytes.count >= 8, Array(bytes[0..<8]) == signature else { return nil }
        var out: [Chunk] = [], at = 8
        while at + 12 <= bytes.count {
            let length = Int(bytes[at]) << 24 | Int(bytes[at + 1]) << 16 | Int(bytes[at + 2]) << 8 | Int(bytes[at + 3])
            guard length >= 0, at + 12 + length <= bytes.count else { return nil }
            let type = String(decoding: bytes[(at + 4)..<(at + 8)], as: UTF8.self)
            out.append(Chunk(type: type, data: Data(bytes[(at + 8)..<(at + 8 + length)])))
            at += 12 + length
            if type == "IEND" { break }
        }
        return out
    }

    /// `png` as a still image of its first frame: itself when it is not
    /// animated, nil when its animation is malformed.
    static func first(_ png: Data) -> Data? {
        guard let chunks = chunks(png) else { return nil }
        guard let image = chunks.firstIndex(where: { $0.type == "IDAT" }),
              chunks[..<image].contains(where: { $0.type == "acTL" }) else { return png }
        let head = chunks[..<image].filter { $0.type != "acTL" && $0.type != "fcTL" }
        let data: [Data]
        if chunks[..<image].contains(where: { $0.type == "fcTL" }) {
            // The default image is frame 0.
            data = chunks[image...].prefix { $0.type == "IDAT" }.map(\.data)
        } else {
            // Frame 0 is the first `fcTL`'s `fdAT`s, each after its sequence number.
            guard let control = chunks.firstIndex(where: { $0.type == "fcTL" }) else { return nil }
            data = chunks[(control + 1)...].prefix { $0.type == "fdAT" }.compactMap { $0.data.count > 4 ? $0.data.dropFirst(4) : nil }.map { Data($0) }
        }
        guard !data.isEmpty else { return nil }
        var out = Data(signature)
        for chunk in head { out.append(encode(chunk.type, chunk.data)) }
        for d in data { out.append(encode("IDAT", d)) }
        out.append(encode("IEND", Data()))
        return out
    }

    /// The header's width and height, before anything is decoded.
    static func size(_ png: Data) -> (width: Int, height: Int)? {
        guard let header = chunks(png)?.first, header.type == "IHDR", header.data.count >= 8 else { return nil }
        let b = [UInt8](header.data)
        return (Int(b[0]) << 24 | Int(b[1]) << 16 | Int(b[2]) << 8 | Int(b[3]),
                Int(b[4]) << 24 | Int(b[5]) << 16 | Int(b[6]) << 8 | Int(b[7]))
    }

    private static func encode(_ type: String, _ data: Data) -> Data {
        var out = Data()
        let length = UInt32(data.count), body = Data(type.utf8) + data
        out.append(contentsOf: [UInt8(length >> 24), UInt8(length >> 16 & 0xFF), UInt8(length >> 8 & 0xFF), UInt8(length & 0xFF)])
        out.append(body)
        let crc = crc32(body)
        out.append(contentsOf: [UInt8(crc >> 24), UInt8(crc >> 16 & 0xFF), UInt8(crc >> 8 & 0xFF), UInt8(crc & 0xFF)])
        return out
    }

    private static let table: [UInt32] = (0..<256).map { n in
        (0..<8).reduce(UInt32(n)) { c, _ in c & 1 == 1 ? 0xEDB8_8320 ^ (c >> 1) : c >> 1 }
    }

    private static func crc32(_ data: Data) -> UInt32 {
        ~data.reduce(~UInt32(0)) { table[Int(($0 ^ UInt32($1)) & 0xFF)] ^ ($0 >> 8) }
    }
}
