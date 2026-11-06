import CoreGraphics
import Foundation
import ImageIO
import UniformTypeIdentifiers

/// T3 Code 9c161462c9's fitImageToMaxEdge (apps/web/src/lib/imageCompression.ts) for the draft images: a PNG or
/// JPEG whose displayed (EXIF-oriented) long edge is over the attachment limit the client passes
/// (IMAGE_ATTACHMENT_MAX_EDGE, image-attachment-size.ts) is scaled down to it. HEIC and HEIF are
/// scaled too, since the reference converts them to JPEG before it fits them; GIF and WebP are left
/// as attached. The result is a PNG, the format every draft image here is stored in.
enum T3ImageFit {
    struct Image { let data: Data; let width: Int; let height: Int }

    /// fitImageAttachmentSize: the size, keeping the aspect ratio, whose long edge is `maxEdge`; nil when the image fits.
    static func fittedSize(width: Int, height: Int, maxEdge: Int) -> (width: Int, height: Int)? {
        let longest = max(width, height)
        guard maxEdge > 0, longest > maxEdge else { return nil }
        let scale = Double(maxEdge) / Double(longest)
        // Math.round: a half rounds up.
        let side = { (value: Int) in max(1, min(maxEdge, Int((Double(value) * scale + 0.5).rounded(.down)))) }
        return (side(width), side(height))
    }

    static func downscalable(_ type: UTType) -> Bool {
        [UTType.png, .jpeg, .heic, .heif].contains { type.conforms(to: $0) }
    }

    static func downscalable(_ data: Data) -> Bool {
        guard let source = CGImageSourceCreateWithData(data as CFData, nil),
              let identifier = CGImageSourceGetType(source), let type = UTType(identifier as String) else { return false }
        return downscalable(type)
    }

    /// The scaled PNG; nil when the image fits, is another format, or cannot be decoded or encoded, so the caller keeps the original.
    static func fit(_ data: Data, maxEdge: Int) -> Image? {
        guard maxEdge > 0, let source = CGImageSourceCreateWithData(data as CFData, nil),
              let identifier = CGImageSourceGetType(source), let type = UTType(identifier as String), downscalable(type),
              let properties = CGImageSourceCopyPropertiesAtIndex(source, 0, nil) as? [CFString: Any],
              let pixelWidth = properties[kCGImagePropertyPixelWidth] as? Int, let pixelHeight = properties[kCGImagePropertyPixelHeight] as? Int,
              pixelWidth > 0, pixelHeight > 0 else { return nil }
        // EXIF orientations 5 to 8 turn the image a quarter, so its displayed width is the stored height.
        let turned = (properties[kCGImagePropertyOrientation] as? Int ?? 1) >= 5
        let width = turned ? pixelHeight : pixelWidth, height = turned ? pixelWidth : pixelHeight
        guard let size = fittedSize(width: width, height: height, maxEdge: maxEdge) else { return nil }
        // The whole image, oriented (createImageBitmap's `imageOrientation: "from-image"`).
        let options: [CFString: Any] = [kCGImageSourceCreateThumbnailFromImageAlways: true, kCGImageSourceCreateThumbnailWithTransform: true,
                                        kCGImageSourceThumbnailMaxPixelSize: max(width, height)]
        guard let image = CGImageSourceCreateThumbnailAtIndex(source, 0, options as CFDictionary) else { return nil }
        let context = [image.colorSpace, CGColorSpace(name: CGColorSpace.sRGB)].lazy.compactMap { space -> CGContext? in
            guard let space, space.model == .rgb else { return nil }
            return CGContext(data: nil, width: size.width, height: size.height, bitsPerComponent: 8, bytesPerRow: 0, space: space,
                             bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue)
        }.first
        guard let context else { return nil }
        context.interpolationQuality = .high
        context.draw(image, in: CGRect(x: 0, y: 0, width: size.width, height: size.height))
        let output = NSMutableData()
        guard let scaled = context.makeImage(),
              let destination = CGImageDestinationCreateWithData(output, UTType.png.identifier as CFString, 1, nil) else { return nil }
        CGImageDestinationAddImage(destination, scaled, nil)
        guard CGImageDestinationFinalize(destination) else { return nil }
        return Image(data: output as Data, width: size.width, height: size.height)
    }
}
