import CoreGraphics
import Foundation
@testable import ExactKit

/// Alpha is the last byte in the tests' RGBA8 and BGRA8 captures.
enum CapturePixels {
    static func painted<Bytes: RandomAccessCollection>(_ rgba: Bytes, at: Int) -> Bool where Bytes.Index == Int, Bytes.Element == UInt8 {
        rgba[at + 3] > 0
    }

    static func opaque<Bytes: RandomAccessCollection>(_ rgba: Bytes, at: Int) -> Bool where Bytes.Index == Int, Bytes.Element == UInt8 {
        rgba[at + 3] == 255
    }

    static func hasPaint<Bytes: RandomAccessCollection>(_ rgba: Bytes) -> Bool where Bytes.Index == Int, Bytes.Element == UInt8 {
        stride(from: rgba.startIndex, to: rgba.endIndex, by: 4).contains { painted(rgba, at: $0) }
    }

    /// Pixel rows without their storage's padding; channels keep their source format.
    static func bytes(_ image: CGImage?) -> Data {
        guard let image, let data = image.dataProvider?.data as Data? else { return Data() }
        return data.withUnsafeBytes { raw in
            rows(raw.baseAddress!, width: image.width * image.bitsPerPixel / 8, height: image.height, stride: image.bytesPerRow)
        }
    }

    static func bytes(_ raster: TextRasterImage?) -> Data {
        guard let raster else { return Data() }
        #if os(macOS)
        let surface = raster.surface
        surface.lock(options: .readOnly, seed: nil)
        defer { surface.unlock(options: .readOnly, seed: nil) }
        return rows(surface.baseAddress, width: surface.width * surface.bytesPerElement, height: surface.height, stride: surface.bytesPerRow)
        #else
        return bytes(raster.image)
        #endif
    }

    private static func rows(_ base: UnsafeRawPointer, width: Int, height: Int, stride: Int) -> Data {
        var out = Data()
        for y in 0..<height { out.append(base.advanced(by: y * stride).assumingMemoryBound(to: UInt8.self), count: width) }
        return out
    }
}

#if os(macOS)
import AppKit
import IOSurface

extension NSBitmapImageRep {
    func paintedColorAt(x: Int, y: Int) -> NSColor? {
        guard let color = colorAt(x: x, y: y), color.alphaComponent > 0 else { return nil }
        return color
    }
}
#endif
