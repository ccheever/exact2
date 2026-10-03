// Writes the color fixtures of LLP 1100 §10 into this directory (or the
// argument's), each with a `<name>.json` of what it should decode to.
//
//   swift scripts/fixtures/color/make.swift
//
// Run by hand on macOS 15 or later. The expected values come from the
// standards' numbers below, never from Core Graphics, so a test holds the
// platform to the standard rather than to itself.
import CoreGraphics
import Foundation
import ImageIO

let out = URL(fileURLWithPath: CommandLine.arguments.count > 1 ? CommandLine.arguments[1]
    : URL(fileURLWithPath: #filePath).deletingLastPathComponent().path)

// MARK: - The standards' numbers

typealias Vec = [Double]
typealias Mat = [[Double]]
func mul(_ m: Mat, _ v: Vec) -> Vec { m.map { r in zip(r, v).map(*).reduce(0, +) } }
func mul(_ a: Mat, _ b: Mat) -> Mat { a.map { r in (0..<3).map { j in (0..<3).map { r[$0] * b[$0][j] }.reduce(0, +) } } }
func inv(_ m: Mat) -> Mat {
    let a = m[0][0], b = m[0][1], c = m[0][2], d = m[1][0], e = m[1][1], f = m[1][2], g = m[2][0], h = m[2][1], i = m[2][2]
    let det = a * (e * i - f * h) - b * (d * i - f * g) + c * (d * h - e * g)
    return [[(e * i - f * h) / det, (c * h - b * i) / det, (b * f - c * e) / det],
            [(f * g - d * i) / det, (a * i - c * g) / det, (c * d - a * f) / det],
            [(d * h - e * g) / det, (b * g - a * h) / det, (a * e - b * d) / det]]
}
/// RGB → XYZ from CIE xy primaries and white point.
func toXYZ(_ p: [(Double, Double)], white w: (Double, Double)) -> Mat {
    let xyz = p.map { [$0.0 / $0.1, 1, (1 - $0.0 - $0.1) / $0.1] }
    let m: Mat = (0..<3).map { r in xyz.map { $0[r] } }
    let s = mul(inv(m), [w.0 / w.1, 1, (1 - w.0 - w.1) / w.1])
    return m.map { r in (0..<3).map { r[$0] * s[$0] } }
}
let d65 = (0.3127, 0.3290), d50 = (0.3457, 0.3585)
/// Bradford D50 → D65, as CSS Color 4's sample code.
let d50to65: Mat = [[0.955473421488075, -0.02309845494876471, 0.06325924320057072],
                    [-0.0283697093338637, 1.0099953980813041, 0.021041441191917323],
                    [0.012314014864481998, -0.020507649298898964, 1.330365926242124]]
let srgbXYZ = toXYZ([(0.64, 0.33), (0.30, 0.60), (0.15, 0.06)], white: d65)

struct Space {
    let name: String            // LLP 1100 D1's name
    let cg: CFString
    let toXYZ65: Mat
    let decode: (Double) -> Double   // encoded → linear
}
func srgbEOTF(_ v: Double) -> Double {
    let a = abs(v); let l = a <= 0.04045 ? a / 12.92 : pow((a + 0.055) / 1.055, 2.4); return v < 0 ? -l : l
}
let spaces: [String: Space] = [
    "srgb": Space(name: "srgb", cg: CGColorSpace.sRGB, toXYZ65: srgbXYZ, decode: srgbEOTF),
    "display-p3": Space(name: "display-p3", cg: CGColorSpace.displayP3,
        toXYZ65: toXYZ([(0.680, 0.320), (0.265, 0.690), (0.150, 0.060)], white: d65), decode: srgbEOTF),
    "display-p3-linear": Space(name: "display-p3-linear", cg: CGColorSpace.extendedLinearDisplayP3,
        toXYZ65: toXYZ([(0.680, 0.320), (0.265, 0.690), (0.150, 0.060)], white: d65), decode: { $0 }),
    "a98-rgb": Space(name: "a98-rgb", cg: CGColorSpace.adobeRGB1998,
        toXYZ65: toXYZ([(0.64, 0.33), (0.21, 0.71), (0.15, 0.06)], white: d65),
        decode: { v in (v < 0 ? -1 : 1) * pow(abs(v), 563.0 / 256.0) }),
    "prophoto-rgb": Space(name: "prophoto-rgb", cg: CGColorSpace.rommrgb,
        toXYZ65: mul(d50to65, toXYZ([(0.734699, 0.265301), (0.159597, 0.840403), (0.036598, 0.000105)], white: d50)),
        decode: { v in let a = abs(v); let l = a <= 16.0 / 512.0 ? a / 16 : pow(a, 1.8); return v < 0 ? -l : l }),
    "rec2020": Space(name: "rec2020", cg: CGColorSpace.itur_2020,
        toXYZ65: toXYZ([(0.708, 0.292), (0.170, 0.797), (0.131, 0.046)], white: d65),
        decode: { v in
            // BT.2020's OETF inverse, as CSS Color 4 defines `rec2020`.
            let a = 1.09929682680944, b = 0.018053968510807, x = abs(v)
            let l = x < b * 4.5 ? x / 4.5 : pow((x + a - 1) / a, 1 / 0.45); return v < 0 ? -l : l }),
]
/// Encoded components in `space` → extended linear sRGB (D65), rounded.
func linearSRGB(_ c: Vec, in space: Space) -> Vec {
    mul(inv(srgbXYZ), mul(space.toXYZ65, c.map(space.decode))).map { ($0 * 1e6).rounded() / 1e6 }
}
func grayLinear(_ g: Double) -> Vec { let l = pow(g, 2.2); return [l, l, l].map { ($0 * 1e6).rounded() / 1e6 } }
/// BT.2100 PQ: cd/m² → signal. 203 cd/m² (BT.2408 reference white) is
/// linear 1.0 in the expectations.
func pqEncode(_ nits: Double) -> Double {
    let m1 = 2610.0 / 16384, m2 = 2523.0 / 4096 * 128, c1 = 3424.0 / 4096, c2 = 2413.0 / 4096 * 32, c3 = 2392.0 / 4096 * 32
    let y = pow(nits / 10000, m1); return pow((c1 + c2 * y) / (1 + c3 * y), m2)
}

// MARK: - Pictures

struct Patch: Encodable { let x: Int; let y: Int; let w: Int; let h: Int; let source: Vec; let linearSRGB: Vec?; let nits: Double? }
struct Expectation: Encodable {
    let file: String
    let id: String
    let `class`: String         // standard | wide | deep | hdr
    let variant: String         // what an SDR display stores (LLP 1100 D7)
    let space: String           // the stored space, by D1's name
    let bitsPerComponent: Int
    let note: String
    let patches: [Patch]
    /// Allowed difference per channel, in sRGB-encoded 1/255 steps, where the
    /// default (2 lossless, 6 lossy) doesn't hold for a stated reason.
    var tolerance: Double? = nil
}
var written: [Expectation] = []

let W = 256, H = 64
/// Equal columns across `width`, the last taking the remainder.
func columns(_ colors: [Vec], width: Int = W, height: Int = H) -> [(CGRect, Vec)] {
    let w = width / colors.count
    return colors.enumerated().map { i, c in
        (CGRect(x: i * w, y: 0, width: i == colors.count - 1 ? width - i * w : w, height: height), c)
    }
}
func context(_ space: CGColorSpace, bpc: Int = 8, alpha: Bool = false, float: Bool = false, width: Int = W, height: Int = H) -> CGContext {
    let components = space.numberOfComponents + (alpha || space.model == .rgb ? 1 : 0)
    let info: UInt32
    if space.model == .monochrome { info = CGImageAlphaInfo.none.rawValue }
    else if space.model == .cmyk { info = CGImageAlphaInfo.none.rawValue }
    else if float { info = CGImageAlphaInfo.premultipliedLast.rawValue | CGBitmapInfo.floatComponents.rawValue | CGBitmapInfo.byteOrder16Little.rawValue }
    else if bpc == 16 { info = (alpha ? CGImageAlphaInfo.premultipliedLast : .noneSkipLast).rawValue | CGBitmapInfo.byteOrder16Little.rawValue }
    else { info = (alpha ? CGImageAlphaInfo.premultipliedLast : .noneSkipLast).rawValue }
    let perPixel = space.model == .monochrome ? 1 : (space.model == .cmyk ? 4 : components)
    return CGContext(data: nil, width: width, height: height, bitsPerComponent: bpc, bytesPerRow: width * perPixel * bpc / 8,
                     space: space, bitmapInfo: info)!
}
func paint(_ ctx: CGContext, _ space: CGColorSpace, _ cells: [(CGRect, Vec)], alpha: Double = 1) {
    for (rect, c) in cells {
        ctx.setFillColor(CGColor(colorSpace: space, components: (c + [alpha]).map { CGFloat($0) })!)
        ctx.fill(rect)
    }
}
@discardableResult
func write(_ image: CGImage, _ file: String, _ type: String, properties: [CFString: Any] = [:], options: [CFString: Any] = [:]) -> Data {
    let data = NSMutableData()
    let dst = CGImageDestinationCreateWithData(data, type as CFString, 1, options as CFDictionary)!
    CGImageDestinationAddImage(dst, image, properties as CFDictionary)
    precondition(CGImageDestinationFinalize(dst), "encode \(file)")
    try! (data as Data).write(to: out.appendingPathComponent(file))
    return data as Data
}
func expect(_ e: Expectation) { written.append(e) }
func patches(_ cells: [(CGRect, Vec)], _ space: Space?) -> [Patch] {
    cells.map { r, c in
        Patch(x: Int(r.minX), y: Int(r.minY), w: Int(r.width), h: Int(r.height), source: c,
              linearSRGB: space.map { linearSRGB(c, in: $0) } ?? (c.count == 1 ? grayLinear(c[0]) : nil), nits: nil)
    }
}

let primaries: [Vec] = [[1, 0, 0], [0, 1, 0], [0, 0, 1], [0.5, 0.4, 0.3], [1, 1, 1], [0.5, 0.5, 0.5], [0.2, 0.6, 0.8], [0, 0, 0]]
let srgb = spaces["srgb"]!, p3 = spaces["display-p3"]!

// F1 — 8-bit sRGB, the regression baseline.
do {
    let s = CGColorSpace(name: srgb.cg)!, cells = columns(primaries), ctx = context(s); paint(ctx, s, cells)
    write(ctx.makeImage()!, "srgb-ramp.png", "public.png")
    expect(Expectation(file: "srgb-ramp.png", id: "F1", class: "standard", variant: "own8", space: "srgb", bitsPerComponent: 8,
        note: "8-bit sRGB: stored as today, byte for byte", patches: patches(cells, srgb)))
}
// F2 — no profile: CSS says sRGB.
do {
    let s = CGColorSpace(name: srgb.cg)!, cells = columns(primaries), ctx = context(s); paint(ctx, s, cells)
    let png = write(ctx.makeImage()!, "untagged.png", "public.png")
    var b = [UInt8](png), i = 8, kept = [UInt8](png.prefix(8))
    while i < b.count {
        let n = Int(b[i]) << 24 | Int(b[i + 1]) << 16 | Int(b[i + 2]) << 8 | Int(b[i + 3])
        let tag = String(bytes: b[i + 4..<i + 8], encoding: .ascii)!
        if !["iCCP", "sRGB", "gAMA", "cHRM", "cICP"].contains(tag) { kept += b[i..<i + 12 + n] }
        i += 12 + n
    }
    b = kept
    try! Data(b).write(to: out.appendingPathComponent("untagged.png"))
    expect(Expectation(file: "untagged.png", id: "F2", class: "standard", variant: "own8", space: "srgb", bitsPerComponent: 8,
        note: "no iCCP, sRGB, gAMA, cHRM or cICP chunk: untagged is sRGB (CSS Color 4 §10.1)", patches: patches(cells, srgb)))
}
// F3, F4 — Display P3, 8-bit: wide, kept in P3, adopted.
for (file, type, id) in [("p3-primaries.png", "public.png", "F3"), ("p3-camera.jpg", "public.jpeg", "F4"), ("p3.heic", "public.heic", "F4")] {
    let s = CGColorSpace(name: p3.cg)!, cells = columns(primaries), ctx = context(s); paint(ctx, s, cells)
    write(ctx.makeImage()!, file, type, properties: type == "public.png" ? [:] : [kCGImageDestinationLossyCompressionQuality: 1.0])
    expect(Expectation(file: file, id: id, class: "wide", variant: "own8", space: "display-p3", bitsPerComponent: 8,
        note: type == "public.png" ? "lossless: patches exact to 8 bits" : "lossy at quality 1: patches within a few codes",
        patches: patches(cells, p3)))
}
// F5 — P3 with alpha: redrawn, still in P3.
do {
    let s = CGColorSpace(name: p3.cg)!, cells = columns(primaries), ctx = context(s, alpha: true); paint(ctx, s, cells, alpha: 0.5)
    write(ctx.makeImage()!, "p3-alpha.png", "public.png")
    expect(Expectation(file: "p3-alpha.png", id: "F5", class: "wide", variant: "own8", space: "display-p3", bitsPerComponent: 8,
        note: "50% alpha, so not adoptable: redrawn into an 8-bit Display P3 bitmap; patch values are unpremultiplied", patches: patches(cells, p3)))
}
// F6, F6a — 16 bits: deep, kept at 16-bit float.
do {
    let s = CGColorSpace(name: p3.cg)!, ctx = context(s, bpc: 16, width: 1024, height: 16)
    for x in 0..<1024 {
        ctx.setFillColor(CGColor(colorSpace: s, components: [0, CGFloat(x) / 1023, 0, 1])!)
        ctx.fill(CGRect(x: x, y: 0, width: 1, height: 16))
    }
    write(ctx.makeImage()!, "p3-16bit-ramp.png", "public.png")
    let ramp = stride(from: 0, to: 1024, by: 128).map { x in
        Patch(x: x, y: 0, w: 1, h: 16, source: [0, Double(x) / 1023, 0], linearSRGB: linearSRGB([0, Double(x) / 1023, 0], in: p3), nits: nil)
    }
    expect(Expectation(file: "p3-16bit-ramp.png", id: "F6", class: "deep", variant: "deep", space: "display-p3", bitsPerComponent: 16,
        note: "1024 steps of P3 green, one a column: every column distinct when stored deep (8 bits would merge neighbours)", patches: ramp))
    let sr = CGColorSpace(name: srgb.cg)!, cells = columns(primaries), c16 = context(sr, bpc: 16); paint(c16, sr, cells)
    write(c16.makeImage()!, "srgb-16bit.png", "public.png")
    expect(Expectation(file: "srgb-16bit.png", id: "F6a", class: "deep", variant: "deep", space: "srgb", bitsPerComponent: 16,
        note: "a 16-bit sRGB PNG is deep, not standard: bits follow the source", patches: patches(cells, srgb)))
}
// F7, F8 — Adobe RGB and ProPhoto: wide, in their own spaces.
do {
    let a98 = spaces["a98-rgb"]!, s = CGColorSpace(name: a98.cg)!, cells = columns(primaries), ctx = context(s); paint(ctx, s, cells)
    write(ctx.makeImage()!, "adobe-rgb.jpg", "public.jpeg", properties: [kCGImageDestinationLossyCompressionQuality: 1.0])
    expect(Expectation(file: "adobe-rgb.jpg", id: "F7", class: "wide", variant: "own8", space: "a98-rgb", bitsPerComponent: 8,
        note: "Adobe RGB (1998)", patches: patches(cells, a98)))
    let pro = spaces["prophoto-rgb"]!, ps = CGColorSpace(name: pro.cg)!, pc = context(ps, bpc: 16); paint(pc, ps, cells)
    write(pc.makeImage()!, "prophoto-16.tif", "public.tiff")
    expect(Expectation(file: "prophoto-16.tif", id: "F8", class: "deep", variant: "deep", space: "prophoto-rgb", bitsPerComponent: 16,
        note: "ProPhoto at 16 bits: deep, kept in ROMM RGB; its green lies outside P3", patches: patches(cells, pro)))
}
// F9 — CMYK: converted by Core Graphics to Display P3.
do {
    let s = CGColorSpace(name: CGColorSpace.genericCMYK)!, ctx = context(s)
    let inks: [Vec] = [[1, 0, 0, 0], [0, 1, 0, 0], [0, 0, 1, 0], [0, 0, 0, 1], [0, 0, 0, 0], [0.5, 0.2, 0.1, 0.1], [0.2, 0.8, 0, 0], [0.6, 0.6, 0.6, 1]]
    let cells = columns(inks); paint(ctx, s, cells)
    write(ctx.makeImage()!, "cmyk.jpg", "public.jpeg", properties: [kCGImageDestinationLossyCompressionQuality: 1.0])
    expect(Expectation(file: "cmyk.jpg", id: "F9", class: "wide", variant: "own8", space: "srgb", bitsPerComponent: 8,
        note: "CMYK: ImageIO's thumbnail converts it to 8-bit sRGB itself (measured 2026-10-03); the platform's conversion, no CMS here",
        patches: cells.map { r, c in Patch(x: Int(r.minX), y: 0, w: Int(r.width), h: H, source: c, linearSRGB: nil, nits: nil) }))
}
// F10 — gray with a gamma 2.2 profile: standard, in sRGB.
do {
    let s = CGColorSpace(name: CGColorSpace.genericGrayGamma2_2)!, ctx = context(s)
    let grays: [Vec] = [[0], [0.25], [0.5], [0.75], [1]]
    let cells = columns(grays); paint(ctx, s, cells)
    write(ctx.makeImage()!, "gray-gamma22.png", "public.png")
    expect(Expectation(file: "gray-gamma22.png", id: "F10", class: "standard", variant: "own8", space: "srgb", bitsPerComponent: 8,
        note: "gray: stored as sRGB, the same colours in the layout Core Animation shares. Apple's Generic Gray Gamma 2.2 profile differs from a pure 2.2 power by up to 2.5 codes at 25% (measured 2026-10-03), so 4 codes",
        patches: patches(cells, nil), tolerance: 4))
}
// F12, F12p, F13 — PQ and HLG: HDR. On an SDR display, ImageIO's SDR rendition (8-bit Display P3).
let nits: [Double] = [0, 100, 203, 400, 1000, 4000]
for (file, type, cg, id) in [("pq.heic", "public.heic", CGColorSpace.itur_2100_PQ, "F12"), ("pq.avif", "public.avif", CGColorSpace.itur_2100_PQ, "F12"),
                             ("pq-cicp.png", "public.png", CGColorSpace.itur_2100_PQ, "F12p"), ("hlg.heic", "public.heic", CGColorSpace.itur_2100_HLG, "F13")] {
    let s = CGColorSpace(name: cg)!, ctx = context(s, bpc: 16)
    let pq = cg == CGColorSpace.itur_2100_PQ
    let values: [Vec] = pq ? nits.map { let e = pqEncode($0); return [e, e, e] } : [[0], [0.25], [0.5], [0.75], [1]].map { [$0[0], $0[0], $0[0]] }
    let cells = columns(values); paint(ctx, s, cells)
    // Tag PQ with its 4000 cd/m² peak (headroom 19.7), which HEIC and AVIF
    // keep; untagged, ImageIO assumes 1000. ImageIO reads no headroom from
    // PNG (it ignores `cLLi` and `mDCV`, macOS 27), so the PNG's 4000 cd/m²
    // patch shows at 1000.
    let image = ctx.makeImage()!
    write(pq ? CGImageCreateCopyWithContentHeadroom(Float(nits.max()! / 203), image) ?? image : image, file, type)
    let deep = type == "public.png"   // ImageIO's SDR rendition of a 16-bit PNG stays 16-bit; HEIC's is 8-bit
    expect(Expectation(file: file, id: id, class: "hdr", variant: "deep", space: "display-p3", bitsPerComponent: deep ? 16 : 8,
        note: (deep ? "16-bit PNG: SDR rendition 16-bit. " : "10-bit: SDR rendition 8-bit, kept in a deep plan reserved from the header's depth. ") + (pq ? "PQ neutrals at \(nits.map { Int($0) }) cd/m²; HDR display (stage 3): linear = nits / 203. SDR: ImageIO's tone map"
                 : "HLG neutrals at 0, 25, 50, 75, 100% signal (75% is reference white); SDR: ImageIO's tone map"),
        patches: zip(cells, nits).map { cell, n in
            Patch(x: Int(cell.0.minX), y: 0, w: Int(cell.0.width), h: H, source: cell.1,
                  linearSRGB: pq ? [n / 203, n / 203, n / 203] : nil, nits: pq ? n : nil) }))
}
// F11, F17, F20 — ISO 21496-1 gain maps, made from an HDR original.
func hdrOriginal(width: Int = W, height: Int = H) -> (CGImage, [(CGRect, Vec)]) {
    let s = CGColorSpace(name: CGColorSpace.extendedLinearDisplayP3)!
    let ctx = context(s, bpc: 16, float: true, width: width, height: height)
    let levels: [Vec] = [[0.18, 0.18, 0.18], [1, 1, 1], [2, 2, 2], [4, 4, 4], [1, 0.2, 0.1], [3, 0.6, 0.3]]
    let cells = columns(levels, width: width, height: height)
    paint(ctx, s, cells)
    return (ctx.makeImage()!.copy(colorSpace: s)!, cells)
}
for (file, type, id, size, orientation) in [("gainmap-iso.jpg", "public.jpeg", "F11", W, 1), ("gainmap-iso.heic", "public.heic", "F11a", W, 1),
                                           ("gainmap-huge.jpg", "public.jpeg", "F17", 4096, 1), ("gainmap-orient6.heic", "public.heic", "F20", W, 6)] {
    let (image, cells) = hdrOriginal(width: size, height: size == W ? H : size)
    let tagged = CGImageCreateCopyWithContentHeadroom(4, image) ?? image
    write(tagged, file, type, properties: [kCGImagePropertyOrientation: orientation, kCGImageDestinationLossyCompressionQuality: 0.95,
                                           kCGImageDestinationEncodeRequest: kCGImageDestinationEncodeToISOGainmap])
    expect(Expectation(file: file, id: id, class: "hdr", variant: "own8", space: "display-p3", bitsPerComponent: 8,
        note: "ISO 21496-1 gain map, headroom 4; SDR base shown on an SDR display; HDR (stage 3) linear values as the patches say"
            + (orientation == 6 ? "; EXIF orientation 6" : "") + (size > W ? "; 4096² so a 16-bit HDR plan exceeds the 32 MiB ceiling" : ""),
        patches: cells.map { r, c in Patch(x: Int(r.minX), y: 0, w: Int(r.width), h: Int(r.height), source: c, linearSRGB: linearSRGB(c, in: spaces["display-p3-linear"]!), nits: nil) }))
}
// F14 — OpenEXR, linear float: HDR.
do {
    let s = CGColorSpace(name: CGColorSpace.extendedLinearSRGB)!, ctx = context(s, bpc: 16, float: true)
    let cells = columns([[0.5, 0.5, 0.5], [1, 1, 1], [2, 2, 2], [8, 8, 8]]); paint(ctx, s, cells)
    write(ctx.makeImage()!, "linear.exr", "com.ilm.openexr-image")
    expect(Expectation(file: "linear.exr", id: "F14", class: "deep", variant: "deep", space: "srgb-linear", bitsPerComponent: 16,
        note: "linear float samples above 1: deep at 16-bit float, its precision kept and its light clipped to SDR white (it is not marked HDR, so nothing above 1 reaches the display)",
        patches: cells.map { r, c in Patch(x: Int(r.minX), y: 0, w: Int(r.width), h: H, source: c, linearSRGB: c.map { min($0, 1) }, nits: nil) }))
}
// F15 — an animated GIF: standard.
do {
    let data = NSMutableData()
    let dst = CGImageDestinationCreateWithData(data, "com.compuserve.gif" as CFString, 2, nil)!
    CGImageDestinationSetProperties(dst, [kCGImagePropertyGIFDictionary: [kCGImagePropertyGIFLoopCount: 0]] as CFDictionary)
    let s = CGColorSpace(name: srgb.cg)!
    for c in [[1.0, 0, 0], [0.0, 1, 0]] {
        let ctx = context(s); paint(ctx, s, [(CGRect(x: 0, y: 0, width: W, height: H), c)])
        CGImageDestinationAddImage(dst, ctx.makeImage()!, [kCGImagePropertyGIFDictionary: [kCGImagePropertyGIFDelayTime: 0.1]] as CFDictionary)
    }
    precondition(CGImageDestinationFinalize(dst))
    try! (data as Data).write(to: out.appendingPathComponent("anim.gif"))
    expect(Expectation(file: "anim.gif", id: "F15", class: "standard", variant: "own8", space: "srgb", bitsPerComponent: 8,
        note: "two frames, red then green", patches: []))
}
// F18 — a gain map whose metadata is cut short: the SDR base, no error.
do {
    // Cut at the last embedded JPEG, the gain map image; the primary's
    // metadata still names it.
    let b = [UInt8](try! Data(contentsOf: out.appendingPathComponent("gainmap-iso.jpg")))
    let starts = (2..<b.count - 3).filter { b[$0] == 0xff && b[$0 + 1] == 0xd8 && b[$0 + 2] == 0xff }
    try! Data(b[0..<(starts.last ?? b.count)]).write(to: out.appendingPathComponent("gainmap-broken.jpg"))
    expect(Expectation(file: "gainmap-broken.jpg", id: "F18", class: "wide", variant: "own8", space: "display-p3", bitsPerComponent: 8,
        note: "the gain map image is cut off while the metadata still names it: shows the SDR base, never fails", patches: []))
}
// F19 — a corrupt ICC tag table: sRGB, as CSS treats an invalid profile.
do {
    let s = CGColorSpace(name: p3.cg)!, cells = columns(primaries), ctx = context(s); paint(ctx, s, cells)
    var b = [UInt8](write(ctx.makeImage()!, "profile-broken.jpg", "public.jpeg", properties: [kCGImageDestinationLossyCompressionQuality: 1.0]))
    if let at = (0..<b.count - 4).first(where: { b[$0] == 0x61 && b[$0 + 1] == 0x63 && b[$0 + 2] == 0x73 && b[$0 + 3] == 0x70 }) {
        // 'acsp' is at offset 36 of the profile; its tag count is at 128.
        let count = at - 36 + 128
        for k in count..<count + 4 { b[k] = 0xff }
    }
    try! Data(b).write(to: out.appendingPathComponent("profile-broken.jpg"))
    expect(Expectation(file: "profile-broken.jpg", id: "F19", class: "standard", variant: "own8", space: "srgb", bitsPerComponent: 8,
        note: "the ICC tag count is corrupt: the profile is invalid, so the picture is sRGB (CSS Color 4 §10.1)", patches: []))
}
// F20 — P3 JPEG with EXIF orientation 6.
do {
    let s = CGColorSpace(name: p3.cg)!, cells = columns(primaries), ctx = context(s); paint(ctx, s, cells)
    write(ctx.makeImage()!, "p3-orient6.jpg", "public.jpeg", properties: [kCGImagePropertyOrientation: 6, kCGImageDestinationLossyCompressionQuality: 1.0])
    expect(Expectation(file: "p3-orient6.jpg", id: "F20", class: "wide", variant: "own8", space: "display-p3", bitsPerComponent: 8,
        note: "EXIF orientation 6: natural size 64×256; patches given in the source's unrotated coordinates", patches: patches(cells, p3)))
}

let encoder = JSONEncoder(); encoder.outputFormatting = [.prettyPrinted, .sortedKeys]
for e in written {
    let name = (e.file as NSString).deletingPathExtension + "." + (e.file as NSString).pathExtension + ".json"
    try! encoder.encode(e).write(to: out.appendingPathComponent(name))
}
print("wrote \(written.count) fixtures to \(out.path)")
