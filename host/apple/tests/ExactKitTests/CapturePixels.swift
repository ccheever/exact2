/// Alpha is the last byte in the tests' RGBA8 and BGRA8 captures.
enum CapturePixels {
    static func painted<Bytes: RandomAccessCollection>(_ rgba: Bytes, at: Int) -> Bool where Bytes.Index == Int, Bytes.Element == UInt8 {
        rgba[at + 3] > 0
    }

    static func opaque<Bytes: RandomAccessCollection>(_ rgba: Bytes, at: Int) -> Bool where Bytes.Index == Int, Bytes.Element == UInt8 {
        rgba[at + 3] == 255
    }
}
