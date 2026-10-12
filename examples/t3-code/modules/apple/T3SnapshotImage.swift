import Foundation
import ImageIO
import UniformTypeIdentifiers

enum T3SnapshotImage {
    struct Image { let data: Data; let width: Int; let height: Int }
    static func boundedPNG(_ data: Data, limit: Int = 10 * 1024 * 1024, cancelled: () -> Bool) -> Image? {
        guard !cancelled(), let source = CGImageSourceCreateWithData(data as CFData, nil),
              let properties = CGImageSourceCopyPropertiesAtIndex(source, 0, nil) as? [String: Any],
              let width = properties[kCGImagePropertyPixelWidth as String] as? Int,
              let height = properties[kCGImagePropertyPixelHeight as String] as? Int, width > 0, height > 0 else { return nil }
        if data.count <= limit { return Image(data: data, width: width, height: height) }
        var dimension = max(width, height)
        for _ in 0..<16 {
            guard !cancelled(), dimension > 1 else { return nil }
            dimension = max(1, Int(Double(dimension) * 0.75))
            let options: [CFString: Any] = [kCGImageSourceCreateThumbnailFromImageAlways: true, kCGImageSourceThumbnailMaxPixelSize: dimension, kCGImageSourceCreateThumbnailWithTransform: true]
            guard let image = CGImageSourceCreateThumbnailAtIndex(source, 0, options as CFDictionary), !cancelled() else { return nil }
            let output = NSMutableData()
            guard let destination = CGImageDestinationCreateWithData(output, UTType.png.identifier as CFString, 1, nil) else { return nil }
            CGImageDestinationAddImage(destination, image, nil)
            guard CGImageDestinationFinalize(destination), !cancelled() else { return nil }
            if output.length <= limit { return Image(data: output as Data, width: image.width, height: image.height) }
        }
        return nil
    }
}
