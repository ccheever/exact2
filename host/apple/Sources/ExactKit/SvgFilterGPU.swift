// @ref LLP 1055.000 D14 — a filter chain on the GPU, where Core Image says
// what the chain says. The island module runs every primitive in software
// (premultiplied `f32` images, a copy per step): a CSS chain over a full
// iPad scene was 387 ms of the main thread a frame and hundreds of MB while
// it ran. Blur, offset, flood, the Porter-Duff composites, merge, colour
// matrices, component transfers that are straight lines, and drop shadows
// run here instead, on Metal, in the chain's colour space; a chain with
// anything else still goes to the module.
import CoreGraphics
import CoreImage
import Foundation
import IOSurface
import Metal

enum SvgFilterGPU {
    private static let srgb = CGColorSpace(name: CGColorSpace.sRGB)!
    private static let linearSRGB = CGColorSpace(name: CGColorSpace.linearSRGB)!

    /// One context per working space: sRGB (CSS's filter functions and
    /// `color-interpolation-filters: sRGB`) or linear light.
    /// The device and the one queue the pictures' Core Animation renders
    /// and their chains run on, in that order.
    static let metal: (device: MTLDevice, queue: MTLCommandQueue)? = {
        guard let d = MTLCreateSystemDefaultDevice(), let q = d.makeCommandQueue() else { return nil }
        return (d, q)
    }()

    private static let contexts: (srgb: CIContext, linear: CIContext)? = {
        guard let metal else { return nil }
        // sRGB chains work in 8 bits a channel, as a browser's do (and at
        // half the memory of Core Image's half floats: F1's map held 4 of
        // them at 45 MB); linear light keeps half floats against banding.
        func make(_ working: CGColorSpace, _ format: CIFormat) -> CIContext {
            CIContext(mtlCommandQueue: metal.queue, options: [.workingColorSpace: working, .outputColorSpace: srgb,
                                                              .workingFormat: NSNumber(value: format.rawValue),
                                                              .cacheIntermediates: false,
                                                              // Its render tasks allocate about this much (MB), so a
                                                              // big island does not keep hundreds of MB.
                                                              CIContextOption(rawValue: "kCIContextMemoryLimit"): NSNumber(value: 64)])
        }
        return (make(srgb, .RGBA8), make(linearSRGB, .RGBAh))
    }()

    private enum In { case source, alpha, empty, result(Int) }

    private struct Reader {
        let v: [Float]
        var i = 0
        mutating func f() -> Float? { guard i < v.count else { return nil }; defer { i += 1 }; return v[i] }
        mutating func input() -> In? {
            guard let c = f() else { return nil }
            switch c {
            case -1: return .source
            case -2: return .alpha
            case -3: return .empty
            default: return c >= 0 && c == c.rounded() ? .result(Int(c)) : nil
            }
        }
    }

    /// Whether `run` takes this chain: a dry run over an empty source.
    static func runs(_ program: [Float]) -> Bool {
        chain(program, source: nil, w: 1, h: 1, origin: .zero, scale: CGSize(width: 1, height: 1)).ok
    }

    /// The chain `program` (`Filter::encode`) over `source` (premultiplied
    /// sRGB, `w × h`, its first row the region's top, whose origin is
    /// `origin` in user units at `scale` pixels per unit), or `nil` when the
    /// chain has a primitive this path does not run (the module then runs
    /// it) or there is no GPU.
    static func run(_ program: [Float], source cg: CGImage, origin: CGPoint, scale: CGSize) -> CGImage? {
        let (w, h) = (cg.width, cg.height)
        guard let (image, linear) = chain(program, source: CIImage(cgImage: cg), w: w, h: h, origin: origin, scale: scale).result,
              let contexts else { return nil }
        let bounds = CGRect(x: 0, y: 0, width: w, height: h)
        let context = linear ? contexts.linear : contexts.srgb
        return context.createCGImage(image, from: bounds, format: .RGBA8, colorSpace: srgb)
    }

    /// The chain's result over `source` (CI's space: y up, the region's top
    /// row at `h`), cropped to the island, and whether it works in linear
    /// light. With no source, only whether the chain parses.
    static func chain(_ program: [Float], source input: CIImage?, w: Int, h: Int, origin: CGPoint, scale: CGSize)
        -> (ok: Bool, result: (CIImage, Bool)?) {
        let no: (ok: Bool, result: (CIImage, Bool)?) = (false, nil)
        guard contexts != nil else { return no }
        var r = Reader(v: program)
        guard r.f() != nil, r.f() != nil, r.f() != nil, r.f() != nil, let count = r.f(), count >= 1, count < 4096 else { return no }
        let bounds = CGRect(x: 0, y: 0, width: w, height: h)
        let source = input ?? CIImage.empty()
        var results: [(CIImage, CGRect)] = []
        var linear: Bool?
        let clear = CIImage.empty()
        // CI's y runs up; the island's first row is its top.
        func px(_ v: Float, _ o: CGFloat, _ k: CGFloat, _ limit: Int) -> CGFloat {
            min(max(((CGFloat(v) - o) * k).rounded(), 0), CGFloat(limit))
        }
        func alphaOnly(_ i: CIImage) -> CIImage {
            i.applyingFilter("CIColorMatrix", parameters: ["inputRVector": CIVector(x: 0, y: 0, z: 0, w: 0),
                                                           "inputGVector": CIVector(x: 0, y: 0, z: 0, w: 0),
                                                           "inputBVector": CIVector(x: 0, y: 0, z: 0, w: 0),
                                                           "inputAVector": CIVector(x: 0, y: 0, z: 0, w: 1),
                                                           "inputBiasVector": CIVector(x: 0, y: 0, z: 0, w: 0)])
        }
        func image(_ i: In) -> CIImage {
            switch i {
            case .source: return source
            case .alpha: return alphaOnly(source)
            case .empty: return clear
            case .result(let n): return n < results.count ? results[n].0 : clear
            }
        }
        func blur(_ i: CIImage, _ s: Float) -> CIImage {
            s > 0 ? i.applyingGaussianBlur(sigma: Double(s)) : i
        }
        func offset(_ i: CIImage, _ dx: CGFloat, _ dy: CGFloat) -> CIImage {
            i.transformed(by: CGAffineTransform(translationX: dx.rounded(), y: -dy.rounded()))
        }
        func flood(_ c: [Float]) -> CIImage {
            let a = CGFloat(min(max(c[3], 0), 1))
            return CIImage(color: CIColor(red: CGFloat(c[0]), green: CGFloat(c[1]), blue: CGFloat(c[2]), alpha: a, colorSpace: srgb) ?? .clear)
        }
        func composite(_ name: String, _ a: CIImage, _ b: CIImage) -> CIImage {
            a.applyingFilter(name, parameters: [kCIInputBackgroundImageKey: b])
        }
        func matrix(_ i: CIImage, _ m: [Float]) -> CIImage {
            let v = { (r: Int) in CIVector(x: CGFloat(m[r * 5]), y: CGFloat(m[r * 5 + 1]), z: CGFloat(m[r * 5 + 2]), w: CGFloat(m[r * 5 + 3])) }
            return i.applyingFilter("CIColorMatrix", parameters: [
                "inputRVector": v(0), "inputGVector": v(1), "inputBVector": v(2), "inputAVector": v(3),
                "inputBiasVector": CIVector(x: CGFloat(m[4]), y: CGFloat(m[9]), z: CGFloat(m[14]), w: CGFloat(m[19])),
            ]).applyingFilter("CIColorClamp")
        }
        for _ in 0..<Int(count) {
            guard let code = r.f(), let a = r.input(), let b = r.input(),
                  let sx = r.f(), let sy = r.f(), let sw = r.f(), let sh = r.f(), let lin = r.f() else { return no }
            // One working space for the whole chain.
            if linear == nil { linear = lin != 0 } else if linear != (lin != 0) { return no }
            let (kx, ky) = (scale.width, scale.height)
            let x0 = px(sx, origin.x, kx, w), x1 = px(sx + sw, origin.x, kx, w)
            let y0 = px(sy, origin.y, ky, h), y1 = px(sy + sh, origin.y, ky, h)
            let sub = CGRect(x: x0, y: CGFloat(h) - y1, width: max(0, x1 - x0), height: max(0, y1 - y0))
            let out: CIImage
            switch Int(code) {
            case 0:
                guard let bx = r.f(), let by = r.f(), bx >= 0, bx == by else { return no }
                out = blur(image(a), bx * Float(kx + ky) / 2)
            case 1:
                guard let dx = r.f(), let dy = r.f() else { return no }
                out = offset(image(a), CGFloat(dx) * kx, CGFloat(dy) * ky)
            case 2:
                guard let c0 = r.f(), let c1 = r.f(), let c2 = r.f(), let c3 = r.f() else { return no }
                out = flood([c0, c1, c2, c3])
            case 3:
                guard let op = r.f(), r.f() != nil, r.f() != nil, r.f() != nil, r.f() != nil else { return no }
                let names = ["CISourceOverCompositing", "CISourceInCompositing", "CISourceOutCompositing", "CISourceAtopCompositing"]
                guard Int(op) < names.count else { return no }
                out = composite(names[Int(op)], image(a), image(b))
            case 4:
                guard let n = r.f(), n >= 0 else { return no }
                var acc = clear
                for _ in 0..<Int(n) {
                    guard let i = r.input() else { return no }
                    acc = composite("CISourceOverCompositing", image(i), acc)
                }
                out = acc
            case 5:
                var m: [Float] = []
                for _ in 0..<20 { guard let x = r.f() else { return no }; m.append(x) }
                out = matrix(image(a), m)
            case 6:
                guard let bx = r.f(), let by = r.f(), let dx = r.f(), let dy = r.f(),
                      let c0 = r.f(), let c1 = r.f(), let c2 = r.f(), let c3 = r.f(),
                      bx >= 0, bx == by else { return no }
                let src = image(a)
                let moved = offset(blur(alphaOnly(src), bx * Float(kx + ky) / 2), CGFloat(dx) * kx, CGFloat(dy) * ky)
                let shadow = composite("CISourceInCompositing", flood([c0, c1, c2, c3]), moved)
                out = composite("CISourceOverCompositing", src, shadow)
            case 9:
                // Identity, or a straight line (`linear`, or a two-entry
                // `table`), per channel: a diagonal colour matrix.
                var slope: [Float] = [], bias: [Float] = []
                for _ in 0..<4 {
                    guard let kind = r.f() else { return no }
                    switch kind {
                    case 0: slope.append(1); bias.append(0)
                    case 1:
                        guard let n = r.f(), n == 2, let v0 = r.f(), let v1 = r.f() else { return no }
                        slope.append(v1 - v0); bias.append(v0)
                    case 3:
                        guard let s = r.f(), let i = r.f() else { return no }
                        slope.append(s); bias.append(i)
                    default: return no
                    }
                }
                var m = [Float](repeating: 0, count: 20)
                for c in 0..<4 { m[c * 5 + c] = slope[c]; m[c * 5 + 4] = bias[c] }
                out = matrix(image(a), m)
            default:
                return no
            }
            results.append((out.cropped(to: sub), sub))
        }
        guard let last = results.last?.0, let linear else { return no }
        return input == nil ? (true, nil) : (true, (last.cropped(to: bounds), linear))
    }

    /// Start rendering a chain's result into `surface` on the GPU, in the
    /// order of `queue`'s work; the task to wait on, or `nil`.
    /// `size` is the picture's, drawn at the surface's top left.
    static func start(_ image: CIImage, linear: Bool, size: CGSize, into surface: IOSurface) -> CIRenderTask? {
        guard let contexts else { return nil }
        let destination = CIRenderDestination(ioSurface: surface)
        destination.colorSpace = srgb
        destination.isFlipped = true
        return try? (linear ? contexts.linear : contexts.srgb).startTask(toRender: image, from: CGRect(origin: .zero, size: size),
                                                                         to: destination, at: .zero)
    }

    /// Render a chain's result into `surface` on the GPU, in the order of
    /// `queue`'s work, and wait for it.
    static func render(_ image: CIImage, linear: Bool, into surface: IOSurface) -> Bool {
        guard let contexts else { return false }
        let destination = CIRenderDestination(ioSurface: surface)
        destination.colorSpace = srgb
        destination.isFlipped = true
        let context = linear ? contexts.linear : contexts.srgb
        guard let task = try? context.startTask(toRender: image, to: destination) else { return false }
        _ = try? task.waitUntilCompleted()
        return true
    }
}
