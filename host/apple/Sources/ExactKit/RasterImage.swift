import Foundation
import CoreGraphics
import ImageIO
import ObjectiveC

enum RasterFailure: Error, CustomStringConvertible {
    case encodedLimit, headerLimit, dimensions, sourcePixels, overflow, tooLarge, decode, reservation
    case svgModule, invalidSvgDocument, svgDocumentLimit, unsupportedSvgFeature
    /// Bytes no decoder here reads (text served as `image/png`).
    case format
    /// A source that names nothing: another scheme, an asset that is not there.
    case unresolved
    /// What the image's `error` says (LLP 1011 §4) and the agent's `state` shows.
    var description: String {
        switch self {
        case .encodedLimit: return "over \(RasterMetadata.encodedLimit) encoded bytes"
        case .headerLimit: return "no image size in its first \(RasterMetadata.headerLimit) bytes"
        case .dimensions: return "invalid dimensions"
        case .sourcePixels: return "over \(RasterMetadata.pixelLimit) source pixels"
        case .overflow: return "overflow"
        case .tooLarge: return "too large"
        case .decode: return "decode failed"
        case .reservation: return "actual exceeds reservation"
        case .format: return "not an image format this host decodes"
        case .svgModule: return "SVG decoder module unavailable"
        case .invalidSvgDocument: return "invalid SVG document"
        case .svgDocumentLimit: return "SVG document exceeds limit"
        case .unsupportedSvgFeature: return "unsupported SVG image feature"
        case .unresolved: return "the source names no file this host loads"
        }
    }
}

/// How a decode keeps its pixels (LLP 1100 D7): `exact_raster::variant`'s
/// numbers, which the budget prices.
enum RasterVariant {
    /// 8-bit sRGB (the budget's; this host plans none).
    static let srgb8: UInt32 = 1
    /// 8-bit in the picture's own colour space.
    static let own8: UInt32 = 2
    /// 16-bit float in the picture's own colour space.
    static let deep: UInt32 = 3
    /// An HDR picture shown as HDR, tagged with its headroom (LLP 1100 D5).
    static let hdr: UInt32 = 4
    /// 8-bit, for a deep picture the budget can't hold at full depth.
    static let reduced8: UInt32 = 5
    static func bytesPerPixel(_ variant: UInt32) -> Int? {
        switch variant {
        case srgb8, own8, reduced8: return 4
        case deep, hdr: return 8
        default: return nil
        }
    }
}

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
    /// A self-contained SVG document decoded by the optional module.
    var svg = false
    /// More than 8 bits a channel, or float samples (LLP 1100 D4).
    var deep = false
    /// A PQ or HLG transfer, or a gain map, as the header says (LLP 1100 D4).
    var hdr = false
    /// The variant when nothing limits it. For an HDR source's SDR rendition
    /// the header's depth is a guess at ImageIO's, which it can't say.
    var variant: UInt32 { svg ? RasterVariant.srgb8 : deep ? RasterVariant.deep : RasterVariant.own8 }
    var naturalSize: CGSize {
        (5...8).contains(orientation)
            ? CGSize(width: sourceHeight, height: sourceWidth)
            : CGSize(width: sourceWidth, height: sourceHeight)
    }

    static func validated(width: Int, height: Int, orientation: Int, encodedBytes: Int, headerBytes: Int,
                          deep: Bool = false, hdr: Bool = false) throws -> Self {
        guard encodedBytes > 0, encodedBytes <= encodedLimit else { throw RasterFailure.encodedLimit }
        guard headerBytes > 0, headerBytes <= headerLimit, headerBytes <= encodedBytes else { throw RasterFailure.headerLimit }
        guard width > 0, height > 0, (1...8).contains(orientation) else { throw RasterFailure.dimensions }
        let (pixels, overflow) = width.multipliedReportingOverflow(by: height)
        guard !overflow, pixels <= pixelLimit else { throw RasterFailure.sourcePixels }
        return Self(sourceWidth: width, sourceHeight: height, orientation: orientation,
                    encodedBytes: encodedBytes, headerBytes: headerBytes, deep: deep, hdr: hdr)
    }

    static func read(prefix: Data, encodedBytes: Int) throws -> Self {
        guard encodedBytes > 0, encodedBytes <= encodedLimit else { throw RasterFailure.encodedLimit }
        guard !prefix.isEmpty, prefix.count <= headerLimit, prefix.count <= encodedBytes else { throw RasterFailure.headerLimit }
        if looksLikeXML(prefix) {
            guard prefix.count == encodedBytes else { throw RasterFailure.svgDocumentLimit }
            guard let module = SvgRasterModule.imageDecoder else { throw RasterFailure.svgModule }
            var width: UInt32 = 0, height: UInt32 = 0
            let result = prefix.withUnsafeBytes { module.documentSize($0.bindMemory(to: UInt8.self).baseAddress, prefix.count, &width, &height) }
            try svgResult(result)
            var metadata = try validated(width: Int(width), height: Int(height), orientation: 1, encodedBytes: encodedBytes, headerBytes: prefix.count)
            metadata.svg = true
            return metadata
        }
        // ImageIO reads WebP only whole; its size is in the first chunk, so a
        // prefix of a large WebP (the Bluesky CDN serves every image as one)
        // answers from the header itself.
        if prefix.count < encodedBytes, let (width, height) = webpSize(prefix) {
            return try validated(width: width, height: height, orientation: 1,
                                 encodedBytes: encodedBytes, headerBytes: prefix.count)
        }
        let source = CGImageSourceCreateIncremental([kCGImageSourceShouldCache: false] as CFDictionary)
        CGImageSourceUpdateData(source, prefix as CFData, prefix.count == encodedBytes)
        // ImageIO answers nothing from a partial JPEG that carries a gain map
        // (an iPhone HDR export, an Android Ultra HDR photo), where it does for
        // a plain one; its frame header says what the plan needs.
        if prefix.count < encodedBytes,
           (CGImageSourceCopyPropertiesAtIndex(source, 0, nil) as? [CFString: Any])?[kCGImagePropertyPixelWidth] == nil,
           let jpeg = jpegHeader(prefix) {
            return try validated(width: jpeg.width, height: jpeg.height, orientation: jpeg.orientation,
                                 encodedBytes: encodedBytes, headerBytes: prefix.count, deep: jpeg.depth > 8,
                                 hdr: hasGainMap(source, prefix: prefix))
        }
        guard let properties = CGImageSourceCopyPropertiesAtIndex(source, 0, nil) as? [CFString: Any],
              let width = properties[kCGImagePropertyPixelWidth] as? NSNumber,
              let height = properties[kCGImagePropertyPixelHeight] as? NSNumber else {
            // The whole file and still no size: no decoder here reads it.
            throw prefix.count == encodedBytes ? RasterFailure.format : RasterFailure.headerLimit
        }
        let depth = (properties[kCGImagePropertyDepth] as? NSNumber)?.intValue ?? 8
        let float = (properties[kCGImagePropertyIsFloat] as? NSNumber)?.boolValue ?? false
        let hdr = CGImageSourceCreateImageAtIndex(source, 0, nil)?.colorSpace.map(isHDRSpace) ?? false
            || hasGainMap(source, prefix: prefix)
        return try validated(width: width.intValue, height: height.intValue,
            orientation: (properties[kCGImagePropertyOrientation] as? NSNumber)?.intValue ?? 1,
            encodedBytes: encodedBytes, headerBytes: prefix.count, deep: depth > 8 || float, hdr: hdr)
    }
}

/// A gain map (LLP 1100 D4): ImageIO's auxiliary data when the prefix shows
/// it, else the gain map's metadata, which a JPEG carries in its first
/// segments though the map is at the end (ISO 21496-1, `hdrgm`, Apple's).
func hasGainMap(_ source: CGImageSource, prefix: Data) -> Bool {
    if CGImageSourceCopyAuxiliaryDataInfoAtIndex(source, 0, kCGImageAuxiliaryDataTypeHDRGainMap) != nil { return true }
    if #available(iOS 18, macOS 15, tvOS 18, *),
       CGImageSourceCopyAuxiliaryDataInfoAtIndex(source, 0, kCGImageAuxiliaryDataTypeISOGainMap) != nil { return true }
    return gainMapMarkers.contains { prefix.range(of: $0) != nil }
}
private let gainMapMarkers = ["urn:iso:std:iso:ts:21496", "hdrgm:Version", "http://ns.apple.com/HDRGainMap/"].map { Data($0.utf8) }

/// A PQ or HLG transfer (LLP 1100 D4).
func isHDRSpace(_ space: CGColorSpace) -> Bool {
    #if os(tvOS)
    // The PQ/HLG-specific queries have no tvOS availability annotation.
    // This query covers both BT.2100 transfers and is available on tvOS.
    return CGColorSpaceUsesITUR_2100TF(space)
    #else
    return CGColorSpaceIsPQBased(space) || CGColorSpaceIsHLGBased(space)
    #endif
}

/// A colour space's name as the agent reports it (LLP 1100 D1): CSS's where
/// CSS has one, a dashed standard name otherwise, `icc` for anything else.
/// An extended form has its space's name.
func colorSpaceName(_ space: CGColorSpace) -> String {
    let names: [CFString: String] = [
        CGColorSpace.sRGB: "srgb", CGColorSpace.extendedSRGB: "srgb",
        CGColorSpace.linearSRGB: "srgb-linear", CGColorSpace.extendedLinearSRGB: "srgb-linear",
        CGColorSpace.displayP3: "display-p3", CGColorSpace.extendedDisplayP3: "display-p3",
        CGColorSpace.linearDisplayP3: "display-p3-linear", CGColorSpace.extendedLinearDisplayP3: "display-p3-linear",
        CGColorSpace.adobeRGB1998: "a98-rgb", CGColorSpace.rommrgb: "prophoto-rgb",
        CGColorSpace.itur_2020: "rec2020", CGColorSpace.extendedITUR_2020: "rec2020",
        CGColorSpace.itur_2100_PQ: "rec2100-pq", CGColorSpace.itur_2100_HLG: "rec2100-hlg",
        CGColorSpace.dcip3: "--dci-p3", CGColorSpace.itur_709: "--rec709", CGColorSpace.acescgLinear: "--aces-cg",
        CGColorSpace.genericGrayGamma2_2: "--gray-gamma-2.2", CGColorSpace.linearGray: "--gray-linear"]
    if let name = space.name, let standard = names[name] { return standard }
    if #available(iOS 18, macOS 15, tvOS 18, *) {
        // Nil for a space with no base (device RGB), whatever the overlay says.
        let base: CGColorSpace? = CGColorSpaceCopyBaseColorSpace(space)
        if let base, base != space { return colorSpaceName(base) }
    }
    return "icc"
}

/// A JPEG's size, sample precision and EXIF orientation from its markers
/// (ITU T.81 B.2.2; the TIFF tag 0x0112 in an APP1 Exif segment), or nil
/// when the prefix is not a JPEG or ends before its frame header.
func jpegHeader(_ data: Data) -> (width: Int, height: Int, depth: Int, orientation: Int)? {
    let b = [UInt8](data)
    guard b.count > 4, b[0] == 0xff, b[1] == 0xd8 else { return nil }
    var orientation = 1, i = 2
    while i + 4 <= b.count {
        guard b[i] == 0xff else { return nil }
        let marker = b[i + 1]
        if marker == 0xff { i += 1; continue }
        let length = Int(b[i + 2]) << 8 | Int(b[i + 3])
        guard length >= 2, i + 2 + length <= b.count else { return nil }
        let body = i + 4
        if marker == 0xe1, length >= 16, b[body..<body + 6].elementsEqual([0x45, 0x78, 0x69, 0x66, 0, 0]) {
            orientation = exifOrientation(Array(b[body + 6..<i + 2 + length])) ?? orientation
        }
        // SOF0–SOF15 except DHT (C4), JPG (C8) and DAC (CC): P, Y, X.
        if (0xc0...0xcf).contains(marker), ![0xc4, 0xc8, 0xcc].contains(marker), length >= 8 {
            let height = Int(b[body + 1]) << 8 | Int(b[body + 2]), width = Int(b[body + 3]) << 8 | Int(b[body + 4])
            return (width, height, Int(b[body]), orientation)
        }
        if marker == 0xda { return nil }
        i += 2 + length
    }
    return nil
}

/// The orientation tag of a TIFF structure's first IFD, if present.
private func exifOrientation(_ t: [UInt8]) -> Int? {
    guard t.count >= 8 else { return nil }
    let little = t[0] == 0x49
    func u16(_ k: Int) -> Int { little ? Int(t[k]) | Int(t[k + 1]) << 8 : Int(t[k]) << 8 | Int(t[k + 1]) }
    func u32(_ k: Int) -> Int { little ? u16(k) | u16(k + 2) << 16 : u16(k) << 16 | u16(k + 2) }
    let ifd = u32(4)
    guard ifd + 2 <= t.count else { return nil }
    for n in 0..<u16(ifd) {
        let entry = ifd + 2 + n * 12
        guard entry + 12 <= t.count else { return nil }
        if u16(entry) == 0x0112 { let v = u16(entry + 8); return (1...8).contains(v) ? v : nil }
    }
    return nil
}

/// Raster signatures exit on the first byte; XML sniffing needs no UTF-8
/// conversion or String allocation, and stops within the bounded prefix.
private func looksLikeXML(_ data: Data) -> Bool {
    data.withUnsafeBytes { raw in
        let b = raw.bindMemory(to: UInt8.self)
        var i = b.count >= 3 && b[0] == 0xef && b[1] == 0xbb && b[2] == 0xbf ? 3 : 0
        while i < min(b.count, 256) {
            if ![9, 10, 13, 32].contains(b[i]) { return b[i] == 60 }
            i += 1
        }
        return false
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
    /// The storage (`RasterVariant`); it sets the bytes a pixel costs.
    let variant: UInt32
    let bytesPerPixel: Int
    var deep: Bool { bytesPerPixel == 8 }
    let stride: Int
    let outputBytes: Int
    /// Conservative allowance for the reduced ImageIO thumbnail and conversion.
    /// This is a reservation policy, not a bound on opaque codec RSS. The final
    /// provider storage is exact; no app-owned full-source bitmap is created.
    let scratchBytes: Int
    var peakBytes: Int { outputBytes + scratchBytes }

    init(metadata: RasterMetadata, maxPixel: Int, variant: UInt32? = nil) throws {
        guard maxPixel > 0 else { throw RasterFailure.dimensions }
        self.variant = variant ?? metadata.variant
        guard let bytes = RasterVariant.bytesPerPixel(self.variant) else { throw RasterFailure.reservation }
        guard !metadata.svg || self.variant == RasterVariant.srgb8 else { throw RasterFailure.reservation }
        bytesPerPixel = bytes
        let natural = metadata.naturalSize
        let longest = Int(max(natural.width, natural.height))
        self.maxPixel = metadata.svg ? maxPixel : min(maxPixel, longest)
        // Keep the longest axis exact: floating ceil can add one pixel and
        // make a worker reconstruct a different reservation from this size.
        width = try Self.scaled(Int(natural.width), pixel: self.maxPixel, longest: longest)
        height = try Self.scaled(Int(natural.height), pixel: self.maxPixel, longest: longest)
        stride = try Self.aligned(try Self.product(width, bytes), to: 64)
        outputBytes = try Self.product(stride, height)
        if metadata.svg {
            // SVG paints directly into Core Graphics-owned storage. Reserve one
            // output-sized allowance for makeImage's possible copy; no ImageIO
            // thumbnail or separate RGBA conversion bitmap exists on this path.
            let (scratch, overflow) = outputBytes.addingReportingOverflow(64 * 1024)
            guard !overflow else { throw RasterFailure.overflow }
            scratchBytes = scratch
        } else {
            // ImageIO does not expose a contractual internal allocator ceiling.
            let stagingStride = try Self.aligned(try Self.product(width, 16), to: 4096)
            let staging = try Self.product(stagingStride, height)
            let (scratch, overflow) = staging.addingReportingOverflow(64 * 1024)
            guard !overflow else { throw RasterFailure.overflow }
            scratchBytes = scratch
        }
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
    // Cache identity: first frame, EXIF-transformed pixels, in the plan's variant.
    let image: CGImage
    let naturalSize: CGSize
    let residentBytes: Int
    let animation: RasterAnimation?
    /// An HDR bitmap's headroom over SDR white, else 0.
    let headroom: Float
    private init(image: CGImage, natural: CGSize, bytes: Int, animation: RasterAnimation?, headroom: Float = 0) {
        self.image = image; naturalSize = natural; residentBytes = bytes; self.animation = animation; self.headroom = headroom
    }
    var isHDR: Bool { headroom > 1 }

    static func decode(_ bytes: Data, metadata: RasterMetadata, plan: RasterDecodePlan,
                       charge: any RasterBackingCharge, sourceOwner: (any RasterSourceOwner)? = nil,
                       url: URL? = nil) throws -> RasterImage {
        guard bytes.count == metadata.encodedBytes, bytes.count <= RasterMetadata.encodedLimit else { throw RasterFailure.encodedLimit }
        // SVG validates its actual viewport in the render call, with the same
        // parsed tree that paints it. Raster formats still recheck ImageIO facts.
        if !metadata.svg {
            let actual = try RasterMetadata.read(prefix: bytes.prefix(RasterMetadata.headerLimit), encodedBytes: bytes.count)
            guard actual == metadata else { throw RasterFailure.reservation }
        }
        let owner = RasterOwner(charge: charge, sourceOwner: sourceOwner)
        // This pool ends before the caller publishes the payload/completes its
        // permit. No ImageIO source, thumbnail, or CGContext enters the mailbox.
        return try autoreleasepool {
            if metadata.svg {
                guard let module = SvgRasterModule.imageDecoder else { throw RasterFailure.svgModule }
                let info = CGImageAlphaInfo.premultipliedFirst.rawValue | CGBitmapInfo.byteOrder32Little.rawValue
                guard let context = CGContext(data: nil, width: plan.width, height: plan.height, bitsPerComponent: 8,
                    bytesPerRow: plan.stride, space: CGColorSpace(name: CGColorSpace.sRGB)!, bitmapInfo: info),
                    context.bytesPerRow == plan.stride, let pixels = context.data else { throw RasterFailure.decode }
                let result = bytes.withUnsafeBytes { module.documentRender($0.bindMemory(to: UInt8.self).baseAddress, bytes.count,
                    pixels.assumingMemoryBound(to: UInt8.self), UInt32(plan.width), UInt32(plan.height), plan.stride,
                    UInt32(metadata.sourceWidth), UInt32(metadata.sourceHeight)) }
                try svgResult(result)
                guard let image = context.makeImage() else { throw RasterFailure.decode }
                return RasterImage(image: owner.own(image), natural: metadata.naturalSize, bytes: plan.outputBytes, animation: nil)
            }
            guard let source = CGImageSourceCreateWithData(bytes as CFData,
                [kCGImageSourceShouldCache: false] as CFDictionary),
                var thumbnail = CGImageSourceCreateThumbnailAtIndex(source, 0, thumbnailOptions(plan, hdr: metadata.hdr)) else { throw RasterFailure.decode }
            // An HDR transfer the header didn't name: decode its SDR rendition.
            if !metadata.hdr, plan.variant != RasterVariant.hdr, let space = thumbnail.colorSpace, isHDRSpace(space) {
                guard let sdr = CGImageSourceCreateThumbnailAtIndex(source, 0, thumbnailOptions(plan, hdr: true))
                else { throw RasterFailure.decode }
                thumbnail = sdr
            }
            let (staging, overflow) = thumbnail.bytesPerRow.multipliedReportingOverflow(by: thumbnail.height)
            // ImageIO may round a reduced side up by a pixel (macOS 27's HDR
            // decode of a 256×64 PQ HEIC at 100 is 100×26, the plan 100×25);
            // the staging bound below still holds it to the reservation.
            guard !overflow, thumbnail.width <= plan.width + 1, thumbnail.height <= plan.height + 1,
                  staging <= plan.scratchBytes else { throw RasterFailure.reservation }
            // The reservation is charged whole: ImageIO's rows are never wider.
            guard var image = normalized(thumbnail, plan: plan) else { throw RasterFailure.decode }
            var headroom: Float = 0
            if plan.variant == RasterVariant.hdr {
                // The decode's headroom, tagged on the stored bitmap too.
                headroom = Self.headroom(of: thumbnail)
                if headroom > 0, #available(iOS 18, macOS 15, tvOS 18, *), let tagged = CGImageCreateCopyWithContentHeadroom(headroom, image) { image = tagged }
            }
            let animation = url.flatMap { RasterAnimation.read(source, url: $0, plan: plan, owner: sourceOwner) }
            return RasterImage(image: owner.own(image), natural: metadata.naturalSize, bytes: plan.outputBytes,
                               animation: animation, headroom: headroom)
        }
    }

    /// An image's headroom, where the system says one (iOS 18 / macOS 15);
    /// before that, PQ's and HLG's default (1000 / 203 cd/m²).
    static func headroom(of image: CGImage) -> Float {
        if #available(iOS 18, macOS 15, tvOS 18, *) { return headroom(reported: image.contentHeadroom, space: image.colorSpace) }
        return headroom(reported: 0, space: image.colorSpace)
    }

    /// Zero is unknown, not SDR. Preserve a known tag; otherwise BT.2100
    /// supplies its reference default, and other spaces remain untagged.
    static func headroom(reported: Float, space: CGColorSpace?) -> Float {
        if reported > 0 { return reported }
        return space.map(isHDRSpace) == true ? 1000 / 203 : 0
    }

    /// ImageIO's reduced decode of one frame at the plan's longest side; an
    /// HDR source's HDR picture for an HDR plan, else its SDR rendition.
    static func thumbnailOptions(_ plan: RasterDecodePlan, hdr: Bool = false) -> CFDictionary {
        var options: [CFString: Any] = [
            kCGImageSourceCreateThumbnailFromImageAlways: true,
            kCGImageSourceCreateThumbnailWithTransform: true,
            kCGImageSourceThumbnailMaxPixelSize: plan.maxPixel,
            kCGImageSourceShouldCacheImmediately: true,
            kCGImageSourceShouldAllowFloat: plan.deep]
        if plan.variant == RasterVariant.hdr {
            options[kCGImageSourceDecodeRequest] = kCGImageSourceDecodeToHDR
        } else if hdr {
            options[kCGImageSourceDecodeRequest] = kCGImageSourceDecodeToSDR
        }
        return options as CFDictionary
    }

    /// A decoded frame as Core Animation shares it: a thumbnail at the
    /// planned size and storage is already the pixels a draw would make, and
    /// is kept; anything else is drawn into a scratch bitmap and Core
    /// Graphics' copy of it kept, as ImageIO does for a thumbnail — CG's image
    /// data is memory Core Animation shares with the render server as it is,
    /// where a bitmap of ours would be copied again at each first commit.
    /// BGRA8 or RGBA16F premultiplied: Core Animation's own layouts, which it
    /// shares without converting.
    static func normalized(_ thumbnail: CGImage, plan: RasterDecodePlan) -> CGImage? {
        // An HDR decode in a BT.2100 space is kept as ImageIO made it: Core
        // Animation tone-maps that to the layer's range, which it doesn't do
        // for extended linear content (LLP 1100 D5, D8).
        // ImageIO's one-pixel rounding (`decode`) keeps it so too, within the
        // resident charge: drawing it to the plan's size would make it extended linear.
        if plan.variant == RasterVariant.hdr, let source = thumbnail.colorSpace, CGColorSpaceUsesITUR_2100TF(source),
           thumbnail.width <= plan.width + 1, thumbnail.height <= plan.height + 1,
           thumbnail.bytesPerRow * thumbnail.height <= plan.outputBytes { return thumbnail }
        let space = storageSpace(thumbnail.colorSpace, plan: plan)
        if isAdoptable(thumbnail, plan: plan, space: space) { return thumbnail }
        let bitmap = plan.deep
            ? CGImageAlphaInfo.premultipliedLast.rawValue | CGBitmapInfo.floatComponents.rawValue | CGBitmapInfo.byteOrder16Little.rawValue
            : CGImageAlphaInfo.premultipliedFirst.rawValue | CGBitmapInfo.byteOrder32Little.rawValue
        return withScratch(plan.outputBytes, { scratch -> CGImage? in
            guard let context = CGContext(data: scratch, width: plan.width, height: plan.height,
                bitsPerComponent: plan.deep ? 16 : 8, bytesPerRow: plan.stride, space: space, bitmapInfo: bitmap) else { return nil }
            context.draw(thumbnail, in: CGRect(x: 0, y: 0, width: plan.width, height: plan.height))
            return context.makeImage()
        })
    }

    /// The colour space a decode is kept in (LLP 1100 D5): an RGB picture's
    /// own; sRGB for gray; Display P3, which contains print gamuts, for CMYK,
    /// Lab and the rest (Core Animation shows only RGB contents).
    static func storageSpace(_ source: CGColorSpace?, plan: RasterDecodePlan) -> CGColorSpace {
        let srgb = CGColorSpace(name: CGColorSpace.sRGB)!
        // Extended: light above SDR white and colours outside P3 alike.
        if plan.variant == RasterVariant.hdr { return CGColorSpace(name: CGColorSpace.extendedLinearDisplayP3)! }
        guard let source else { return srgb }
        let own: CGColorSpace
        switch source.model {
        case .rgb: own = source
        case .monochrome: own = srgb
        default: own = CGColorSpace(name: CGColorSpace.displayP3)!
        }
        // A deep picture in its space's standard-range form: values above 1
        // handed to Core Animation untagged (an OpenEXR's) show bright or not
        // as the OS chooses, whatever `dynamic-range-limit` says.
        guard plan.deep, let name = own.name else { return own }
        let standard: [CFString: CFString] = [
            CGColorSpace.extendedSRGB: CGColorSpace.sRGB, CGColorSpace.extendedDisplayP3: CGColorSpace.displayP3,
            CGColorSpace.extendedITUR_2020: CGColorSpace.itur_2020, CGColorSpace.extendedLinearSRGB: CGColorSpace.linearSRGB,
            CGColorSpace.extendedLinearDisplayP3: CGColorSpace.linearDisplayP3]
        return standard[name].flatMap { CGColorSpace(name: $0) } ?? own
    }
}

private func svgResult(_ result: Int32) throws {
    switch result {
    case 0: return
    case 2: throw RasterFailure.svgDocumentLimit
    case 3: throw RasterFailure.unsupportedSvgFeature
    case 4: throw RasterFailure.reservation
    default: throw RasterFailure.invalidSvgDocument
    }
}

/// Whether drawing the thumbnail into the planned bitmap would change
/// nothing but its bytes' layout: the planned size (no resample), 8-bit
/// opaque pixels already in the storage space (no conversion; the skipped
/// alpha byte reads as opaque), rows no wider than the plan's. A deep plan
/// always draws: ImageIO hands back 16-bit integers, and Core Animation
/// shares half floats. Otherwise the caller draws.
private func isAdoptable(_ thumbnail: CGImage, plan: RasterDecodePlan, space: CGColorSpace) -> Bool {
    !plan.deep
        && thumbnail.width == plan.width && thumbnail.height == plan.height
        && thumbnail.bitsPerComponent == 8 && thumbnail.bitsPerPixel == 32
        && thumbnail.bytesPerRow <= plan.stride
        && thumbnail.bitmapInfo.subtracting(.alphaInfoMask).subtracting(.byteOrderMask).isEmpty
        && [.noneSkipFirst, .noneSkipLast].contains(thumbnail.alphaInfo)
        && thumbnail.colorSpace.map { $0 == space } == true
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

    /// The bitmap into `rect`, or with a `-exact-tint-color` as a template: its
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
