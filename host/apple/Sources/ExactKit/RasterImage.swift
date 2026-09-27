import Foundation
import CoreGraphics
import ImageIO
import ObjectiveC

enum RasterFailure: Error { case encodedLimit, headerLimit, dimensions, sourcePixels, overflow, tooLarge, decode, reservation }

/// Metadata is read before allocating pixels. Only the bounded prefix is passed
/// to ImageIO here; its later internal decoder allocations are not measurable by
/// our ledger and are not claimed to fit the Exact-owned storage budget.
struct RasterMetadata: Equatable, Sendable {
    static let encodedLimit = 64 * 1024 * 1024
    static let headerLimit = 256 * 1024
    static let pixelLimit = 64 * 1024 * 1024
    let sourceWidth: Int
    let sourceHeight: Int
    let orientation: Int
    let encodedBytes: Int
    let headerBytes: Int
    var naturalSize: CGSize {
        (5...8).contains(orientation)
            ? CGSize(width: sourceHeight, height: sourceWidth)
            : CGSize(width: sourceWidth, height: sourceHeight)
    }

    static func validated(width: Int, height: Int, orientation: Int, encodedBytes: Int, headerBytes: Int) throws -> Self {
        guard encodedBytes > 0, encodedBytes <= encodedLimit else { throw RasterFailure.encodedLimit }
        guard headerBytes > 0, headerBytes <= headerLimit, headerBytes <= encodedBytes else { throw RasterFailure.headerLimit }
        guard width > 0, height > 0, (1...8).contains(orientation) else { throw RasterFailure.dimensions }
        let (pixels, overflow) = width.multipliedReportingOverflow(by: height)
        guard !overflow, pixels <= pixelLimit else { throw RasterFailure.sourcePixels }
        return Self(sourceWidth: width, sourceHeight: height, orientation: orientation,
                    encodedBytes: encodedBytes, headerBytes: headerBytes)
    }

    static func read(prefix: Data, encodedBytes: Int) throws -> Self {
        guard encodedBytes > 0, encodedBytes <= encodedLimit else { throw RasterFailure.encodedLimit }
        guard !prefix.isEmpty, prefix.count <= headerLimit, prefix.count <= encodedBytes else { throw RasterFailure.headerLimit }
        // ImageIO reads WebP only whole; its size is in the first chunk, so a
        // prefix of a large WebP (the Bluesky CDN serves every image as one)
        // answers from the header itself.
        if prefix.count < encodedBytes, let (width, height) = webpSize(prefix) {
            return try validated(width: width, height: height, orientation: 1,
                                 encodedBytes: encodedBytes, headerBytes: prefix.count)
        }
        let source = CGImageSourceCreateIncremental([kCGImageSourceShouldCache: false] as CFDictionary)
        CGImageSourceUpdateData(source, prefix as CFData, prefix.count == encodedBytes)
        guard let properties = CGImageSourceCopyPropertiesAtIndex(source, 0, nil) as? [CFString: Any],
              let width = properties[kCGImagePropertyPixelWidth] as? NSNumber,
              let height = properties[kCGImagePropertyPixelHeight] as? NSNumber else { throw RasterFailure.headerLimit }
        return try validated(width: width.intValue, height: height.intValue,
            orientation: (properties[kCGImagePropertyOrientation] as? NSNumber)?.intValue ?? 1,
            encodedBytes: encodedBytes, headerBytes: prefix.count)
    }
}

/// A WebP's canvas size from its RIFF header (`VP8 `, `VP8L` or `VP8X`),
/// or nil when the prefix is not one.
func webpSize(_ data: Data) -> (Int, Int)? {
    let b = [UInt8](data.prefix(32))
    guard b.count >= 30, b[0...3] == [0x52, 0x49, 0x46, 0x46], b[8...11] == [0x57, 0x45, 0x42, 0x50] else { return nil }
    let le16 = { (i: Int) in Int(b[i]) | Int(b[i + 1]) << 8 }
    let le24 = { (i: Int) in Int(b[i]) | Int(b[i + 1]) << 8 | Int(b[i + 2]) << 16 }
    switch String(bytes: b[12...15], encoding: .ascii) {
    case "VP8 ":
        guard b[23...25] == [0x9d, 0x01, 0x2a] else { return nil }
        return (le16(26) & 0x3fff, le16(28) & 0x3fff)
    case "VP8L":
        guard b[20] == 0x2f else { return nil }
        let bits = Int(b[21]) | Int(b[22]) << 8 | Int(b[23]) << 16 | Int(b[24]) << 24
        return ((bits & 0x3fff) + 1, ((bits >> 14) & 0x3fff) + 1)
    case "VP8X":
        return (le24(24) + 1, le24(27) + 1)
    default:
        return nil
    }
}

struct RasterDecodePlan: Sendable {
    let width: Int
    let height: Int
    let maxPixel: Int
    let stride: Int
    let outputBytes: Int
    /// Conservative allowance for the reduced ImageIO thumbnail and conversion.
    /// This is a reservation policy, not a bound on opaque codec RSS. The final
    /// provider storage is exact; no app-owned full-source bitmap is created.
    let scratchBytes: Int
    var peakBytes: Int { outputBytes + scratchBytes }

    init(metadata: RasterMetadata, maxPixel: Int) throws {
        guard maxPixel > 0 else { throw RasterFailure.dimensions }
        let natural = metadata.naturalSize
        let longest = Int(max(natural.width, natural.height))
        self.maxPixel = min(maxPixel, longest)
        // Keep the longest axis exact: floating ceil can add one pixel and
        // make a worker reconstruct a different reservation from this size.
        width = try Self.scaled(Int(natural.width), pixel: self.maxPixel, longest: longest)
        height = try Self.scaled(Int(natural.height), pixel: self.maxPixel, longest: longest)
        stride = try Self.aligned(try Self.product(width, 4), to: 64)
        outputBytes = try Self.product(stride, height)
        // Allow up to 128-bit staging pixels and page-aligned scanlines. ImageIO
        // does not expose a contractual internal allocator ceiling.
        let stagingStride = try Self.aligned(try Self.product(width, 16), to: 4096)
        let staging = try Self.product(stagingStride, height)
        let (scratch, overflow) = staging.addingReportingOverflow(64 * 1024)
        guard !overflow else { throw RasterFailure.overflow }
        scratchBytes = scratch
        let (peak, peakOverflow) = outputBytes.addingReportingOverflow(scratchBytes)
        guard !peakOverflow, peak <= 32 * 1024 * 1024 else { throw RasterFailure.tooLarge }
    }
    private static func product(_ a: Int, _ b: Int) throws -> Int {
        let (n, overflow) = a.multipliedReportingOverflow(by: b)
        guard !overflow else { throw RasterFailure.overflow }; return n
    }
    private static func scaled(_ axis: Int, pixel: Int, longest: Int) throws -> Int {
        let product = try product(axis, pixel)
        return max(1, product / longest + (product % longest == 0 ? 0 : 1))
    }
    private static func aligned(_ n: Int, to alignment: Int) throws -> Int {
        let (sum, overflow) = n.addingReportingOverflow(alignment - 1)
        guard !overflow else { throw RasterFailure.overflow }; return sum & ~(alignment - 1)
    }
}

/// The concrete bridge charge releases an independent Rust budget account. It
/// must not retain Runtime, a session mailbox, a view, or this image.
protocol RasterBackingCharge: AnyObject, Sendable {}
protocol RasterSourceOwner: AnyObject, Sendable {}

/// An image's charge, owned by the `CGImage` itself (an associated object),
/// so every alias of it, a painter's or a framework's, stays charged. The
/// pixels are Core Graphics' own (see `decode`), not a provider of ours.
private final class RasterOwner {
    let charge: any RasterBackingCharge
    let sourceOwner: (any RasterSourceOwner)?
    init(charge: any RasterBackingCharge, sourceOwner: (any RasterSourceOwner)?) { self.charge = charge; self.sourceOwner = sourceOwner }
    private static var key: UInt8 = 0
    func own(_ image: CGImage) -> CGImage {
        objc_setAssociatedObject(image, &Self.key, self, .OBJC_ASSOCIATION_RETAIN)
        return image
    }
}

/// Immutable CG-only payload: safe to destroy on a worker. The `CGImage`, not
/// merely this wrapper, owns the charge so every alias stays charged. An
/// animated GIF or WebP carries its first frame here and the facts to play
/// the rest (`RasterAnimation`, LLP 1011.000).
final class RasterImage: @unchecked Sendable {
    // Cache identity: first frame, EXIF-transformed pixels, normalized sRGB RGBA8.
    static let variant: UInt32 = 1
    let image: CGImage
    let naturalSize: CGSize
    let residentBytes: Int
    let animation: RasterAnimation?
    private init(image: CGImage, natural: CGSize, bytes: Int, animation: RasterAnimation?) {
        self.image = image; naturalSize = natural; residentBytes = bytes; self.animation = animation
    }

    static func decode(_ bytes: Data, metadata: RasterMetadata, plan: RasterDecodePlan,
                       charge: any RasterBackingCharge, sourceOwner: (any RasterSourceOwner)? = nil,
                       url: URL? = nil) throws -> RasterImage {
        guard bytes.count == metadata.encodedBytes, bytes.count <= RasterMetadata.encodedLimit else { throw RasterFailure.encodedLimit }
        // A file can change between metadata inspection and worker admission.
        // Recheck the actual bytes before asking ImageIO for any pixel buffer.
        let actual = try RasterMetadata.read(prefix: bytes.prefix(RasterMetadata.headerLimit), encodedBytes: bytes.count)
        guard actual == metadata else { throw RasterFailure.reservation }
        let owner = RasterOwner(charge: charge, sourceOwner: sourceOwner)
        // This pool ends before the caller publishes the payload/completes its
        // permit. No ImageIO source, thumbnail, or CGContext enters the mailbox.
        return try autoreleasepool {
            guard let source = CGImageSourceCreateWithData(bytes as CFData,
                [kCGImageSourceShouldCache: false] as CFDictionary),
                let thumbnail = CGImageSourceCreateThumbnailAtIndex(source, 0, thumbnailOptions(plan)) else { throw RasterFailure.decode }
            let (staging, overflow) = thumbnail.bytesPerRow.multipliedReportingOverflow(by: thumbnail.height)
            guard !overflow, thumbnail.width <= plan.width, thumbnail.height <= plan.height,
                  staging <= plan.scratchBytes else { throw RasterFailure.reservation }
            // The reservation is charged whole: ImageIO's rows are never wider.
            guard let image = normalized(thumbnail, plan: plan) else { throw RasterFailure.decode }
            let animation = url.flatMap { RasterAnimation.read(source, url: $0, plan: plan, owner: sourceOwner) }
            return RasterImage(image: owner.own(image), natural: metadata.naturalSize, bytes: plan.outputBytes, animation: animation)
        }
    }

    /// ImageIO's reduced decode of one frame at the plan's longest side.
    static func thumbnailOptions(_ plan: RasterDecodePlan) -> CFDictionary {
        [kCGImageSourceCreateThumbnailFromImageAlways: true,
         kCGImageSourceCreateThumbnailWithTransform: true,
         kCGImageSourceThumbnailMaxPixelSize: plan.maxPixel,
         kCGImageSourceShouldCacheImmediately: true,
         kCGImageSourceShouldAllowFloat: false] as CFDictionary
    }

    /// A decoded frame as Core Animation shares it: an opaque sRGB thumbnail
    /// at the planned size is already the pixels a draw would make, and is
    /// kept; anything else is drawn into a scratch bitmap and Core Graphics'
    /// copy of it kept, as ImageIO does for a thumbnail — CG's image data is
    /// memory Core Animation shares with the render server as it is, where a
    /// bitmap of ours would be copied again at each first commit. BGRA,
    /// premultiplied: Core Animation's own layout, which it shares without
    /// converting.
    static func normalized(_ thumbnail: CGImage, plan: RasterDecodePlan) -> CGImage? {
        if isAdoptable(thumbnail, plan: plan) { return thumbnail }
        let space = CGColorSpace(name: CGColorSpace.sRGB)!
        let bitmap = CGImageAlphaInfo.premultipliedFirst.rawValue | CGBitmapInfo.byteOrder32Little.rawValue
        return withScratch(plan.outputBytes, { scratch -> CGImage? in
            guard let context = CGContext(data: scratch, width: plan.width, height: plan.height,
                bitsPerComponent: 8, bytesPerRow: plan.stride, space: space, bitmapInfo: bitmap) else { return nil }
            context.draw(thumbnail, in: CGRect(x: 0, y: 0, width: plan.width, height: plan.height))
            return context.makeImage()
        })
    }
}

/// Whether drawing the thumbnail into the planned sRGB bitmap would change
/// nothing but its bytes' layout: the planned size (no resample), 8-bit
/// opaque sRGB (no conversion; the skipped alpha byte reads as opaque),
/// rows no wider than the plan's. Otherwise the caller draws.
private func isAdoptable(_ thumbnail: CGImage, plan: RasterDecodePlan) -> Bool {
    thumbnail.width == plan.width && thumbnail.height == plan.height
        && thumbnail.bitsPerComponent == 8 && thumbnail.bitsPerPixel == 32
        && thumbnail.bytesPerRow <= plan.stride
        && thumbnail.bitmapInfo.subtracting(.alphaInfoMask).subtracting(.byteOrderMask).isEmpty
        && [.noneSkipFirst, .noneSkipLast].contains(thumbnail.alphaInfo)
        && thumbnail.colorSpace?.name == CGColorSpace.sRGB
}

/// Zeroed memory for a bitmap that lives only while `body` runs. A large
/// one is pages returned to the system as it ends: a freed large malloc
/// block can stay dirty in the allocator's cache, charged to the process,
/// until it is reused. A small one is not worth the system calls.
func withScratch<T>(_ count: Int, _ body: (UnsafeMutableRawPointer) throws -> T?) rethrows -> T? {
    guard count > 0 else { return nil }
    if count < 128 * 1024 {
        guard let small = calloc(count, 1) else { return nil }
        defer { free(small) }
        return try body(small)
    }
    guard let pages = mmap(nil, count, PROT_READ | PROT_WRITE, MAP_PRIVATE | MAP_ANON, -1, 0),
          pages != MAP_FAILED else { return nil }
    defer { munmap(pages, count) }
    return try body(pages)
}

enum RasterGeometry {
    static func rect(natural: CGSize, content: CGRect, fit: String) -> CGRect {
        guard natural.width > 0, natural.height > 0 else { return .zero }
        if fit == "fill" { return content }
        let x = content.width / natural.width, y = content.height / natural.height
        let scale: CGFloat
        switch fit {
        case "contain": scale = min(x, y)
        case "cover": scale = max(x, y)
        case "scale-down": scale = min(1, min(x, y))
        case "none": scale = 1
        default: return content
        }
        let size = CGSize(width: natural.width * scale, height: natural.height * scale)
        return CGRect(x: content.midX - size.width / 2, y: content.midY - size.height / 2, width: size.width, height: size.height)
    }

    /// The bitmap into `rect`, or with a `tint-color` as a template: its
    /// alpha masks the tint (LLP 1011 §4). The same decoded pixels either way.
    /// A layer composited source-in, not `clip(to:mask:)`, which reads an
    /// image with alpha wrongly (a transparent pixel came back half covered).
    static func draw(_ ctx: CGContext, _ image: CGImage, in rect: CGRect, tint: CGColor?) {
        guard let tint else { ctx.draw(image, in: rect); return }
        ctx.saveGState()
        ctx.beginTransparencyLayer(in: rect, auxiliaryInfo: nil)
        ctx.draw(image, in: rect)
        ctx.setBlendMode(.sourceIn)
        ctx.setFillColor(tint)
        ctx.fill(rect)
        ctx.endTransparencyLayer()
        ctx.restoreGState()
    }
}
