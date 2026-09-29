// @ref LLP 1055.000 D14 — a live filter picture's chain in Metal (iOS).
// Core Image builds, tiles and binds a graph for every render: at a frame
// each, F3's CSS chain (blur, drop-shadow, saturate, hue-rotate) cost 5 ms of
// CPU a draw on the M1 iPad Pro and held ~300 MB of intermediates. Here the
// chain is a fixed list of steps over textures kept at the picture's size:
// a Gaussian blur (Metal Performance Shaders, transparent past the edge),
// whole-pixel offsets, colour matrices over unpremultiplied colour, and a
// drop shadow, in sRGB as CSS's filter functions and `feColorMatrix`'s
// defaults for them run. A chain with anything else stays on Core Image.
import CoreGraphics
import Foundation
import IOSurface
import Metal
import MetalPerformanceShaders

final class SvgFilterMetal {
    enum Step {
        case blur(Float)
        case offset(Int32, Int32)
        case matrix([Float])
        case shadow(sigma: Float, dx: Int32, dy: Int32, color: SIMD4<Float>)
    }

    static let shared: SvgFilterMetal? = SvgFilterMetal()

    let device: MTLDevice
    private let matrixPipe: MTLComputePipelineState
    private let offsetPipe: MTLComputePipelineState
    private let shadowPipe: MTLComputePipelineState
    /// Working textures by size (three, ping-ponged), kept while the size holds.
    private var work: [MTLTexture] = []

    private static let source = """
    #include <metal_stdlib>
    using namespace metal;
    kernel void colorMatrix(texture2d<float, access::read> src [[texture(0)]], texture2d<float, access::write> dst [[texture(1)]],
                            constant float *m [[buffer(0)]], uint2 g [[thread_position_in_grid]]) {
        if (g.x >= dst.get_width() || g.y >= dst.get_height()) return;
        float4 p = src.read(g);
        float4 c = p.a > 0 ? float4(p.rgb / p.a, p.a) : float4(0);
        float4 o;
        for (int r = 0; r < 4; r++) o[r] = m[r*5] * c.r + m[r*5+1] * c.g + m[r*5+2] * c.b + m[r*5+3] * c.a + m[r*5+4];
        o = clamp(o, 0.0, 1.0);
        dst.write(float4(o.rgb * o.a, o.a), g);
    }
    kernel void offset(texture2d<float, access::read> src [[texture(0)]], texture2d<float, access::write> dst [[texture(1)]],
                       constant int2 &d [[buffer(0)]], uint2 g [[thread_position_in_grid]]) {
        if (g.x >= dst.get_width() || g.y >= dst.get_height()) return;
        int2 s = int2(g) - d;
        bool in = s.x >= 0 && s.y >= 0 && s.x < int(src.get_width()) && s.y < int(src.get_height());
        dst.write(in ? src.read(uint2(s)) : float4(0), g);
    }
    kernel void dropShadow(texture2d<float, access::read> src [[texture(0)]], texture2d<float, access::read> blurred [[texture(1)]],
                           texture2d<float, access::write> dst [[texture(2)]], constant float4 &color [[buffer(0)]],
                           constant int2 &d [[buffer(1)]], uint2 g [[thread_position_in_grid]]) {
        if (g.x >= dst.get_width() || g.y >= dst.get_height()) return;
        int2 s = int2(g) - d;
        bool in = s.x >= 0 && s.y >= 0 && s.x < int(blurred.get_width()) && s.y < int(blurred.get_height());
        float a = in ? blurred.read(uint2(s)).a : 0;
        float4 shadow = float4(color.rgb * color.a, color.a) * a;
        float4 p = src.read(g);
        dst.write(p + shadow * (1 - p.a), g);
    }
    """

    private init?() {
        guard let d = SvgFilterGPU.metal?.device, MPSSupportsMTLDevice(d),
              let lib = try? d.makeLibrary(source: SvgFilterMetal.source, options: nil),
              let m = lib.makeFunction(name: "colorMatrix").flatMap({ try? d.makeComputePipelineState(function: $0) }),
              let o = lib.makeFunction(name: "offset").flatMap({ try? d.makeComputePipelineState(function: $0) }),
              let s = lib.makeFunction(name: "dropShadow").flatMap({ try? d.makeComputePipelineState(function: $0) }) else { return nil }
        device = d; matrixPipe = m; offsetPipe = o; shadowPipe = s
    }

    /// The chain `program` (`Filter::encode`) as steps at `scale` pixels
    /// per unit, or `nil` when it is not a straight chain of what this runs
    /// (each primitive reading the one before, over the whole region, in
    /// sRGB).
    static func steps(_ program: [Float], scale: CGFloat) -> [Step]? {
        var i = 0
        func f() -> Float? { guard i < program.count else { return nil }; defer { i += 1 }; return program[i] }
        guard let rx = f(), let ry = f(), let rw = f(), let rh = f(), let n = f(), n >= 1, n < 64 else { return nil }
        let region = CGRect(x: CGFloat(rx), y: CGFloat(ry), width: CGFloat(rw), height: CGFloat(rh))
        var out: [Step] = []
        for k in 0..<Int(n) {
            guard let code = f(), let a = f(), let b = f(), let sx = f(), let sy = f(), let sw = f(), let sh = f(), let lin = f(),
                  a == (k == 0 ? -1 : Float(k - 1)), b == -3, lin == 0,
                  CGRect(x: CGFloat(sx), y: CGFloat(sy), width: CGFloat(sw), height: CGFloat(sh)).insetBy(dx: -0.5, dy: -0.5).contains(region)
            else { return nil }
            let px = { (v: Float) in Int32((CGFloat(v) * scale).rounded()) }
            switch Int(code) {
            case 0:
                guard let x = f(), let y = f(), x == y, x >= 0 else { return nil }
                out.append(.blur(x * Float(scale)))
            case 1:
                guard let x = f(), let y = f() else { return nil }
                out.append(.offset(px(x), px(y)))
            case 5:
                var m: [Float] = []
                for _ in 0..<20 { guard let v = f() else { return nil }; m.append(v) }
                out.append(.matrix(m))
            case 6:
                guard let x = f(), let y = f(), let dx = f(), let dy = f(), let r = f(), let g = f(), let bl = f(), let al = f(), x == y, x >= 0 else { return nil }
                out.append(.shadow(sigma: x * Float(scale), dx: px(dx), dy: px(dy), color: SIMD4(r, g, bl, al)))
            case 9:
                // Straight-line transfers only: a diagonal matrix.
                var m = [Float](repeating: 0, count: 20)
                for c in 0..<4 {
                    guard let kind = f() else { return nil }
                    switch kind {
                    case 0: m[c * 5 + c] = 1
                    case 1:
                        guard let cnt = f(), cnt == 2, let v0 = f(), let v1 = f() else { return nil }
                        m[c * 5 + c] = v1 - v0; m[c * 5 + 4] = v0
                    case 3:
                        guard let s = f(), let o = f() else { return nil }
                        m[c * 5 + c] = s; m[c * 5 + 4] = o
                    default: return nil
                    }
                }
                out.append(.matrix(m))
            default: return nil
            }
        }
        return i == program.count ? out : nil
    }

    private func texture(_ w: Int, _ h: Int, _ i: Int) -> MTLTexture? {
        if work.first.map({ $0.width != w || $0.height != h }) ?? false { work = [] }
        while work.count <= i {
            let d = MTLTextureDescriptor.texture2DDescriptor(pixelFormat: .rgba8Unorm, width: w, height: h, mipmapped: false)
            d.usage = [.shaderRead, .shaderWrite]
            d.storageMode = .private
            guard let t = device.makeTexture(descriptor: d) else { return nil }
            work.append(t)
        }
        return work[i]
    }

    /// Encode `steps` from `source` into `surface` (BGRA, the picture's
    /// size) on `cb`. Whether it could.
    /// The working textures are shared by every picture: one encode at a
    /// time (a picture's first draw is on the main thread, the rest on the
    /// pictures' queue). The GPU runs them in order on the one queue.
    private let lock = NSLock()
    /// Blur kernels by σ (their weights are made once).
    private var blurs: [Float: MPSImageGaussianBlur] = [:]

    func encode(_ steps: [Step], source: MTLTexture, into surface: IOSurface, on cb: MTLCommandBuffer) -> Bool {
        lock.lock()
        defer { lock.unlock() }
        let (w, h) = (source.width, source.height)
        let d = MTLTextureDescriptor.texture2DDescriptor(pixelFormat: .bgra8Unorm, width: w, height: h, mipmapped: false)
        d.usage = [.shaderWrite]
        guard let target = device.makeTexture(descriptor: d, iosurface: surface, plane: 0) else { return false }
        var current = source
        /// A working texture that is none of `avoid`.
        func pick(_ avoid: [MTLTexture]) -> MTLTexture? {
            for i in 0..<3 { if let t = texture(w, h, i), !avoid.contains(where: { $0 === t }) { return t } }
            return nil
        }
        func dispatch(_ pipe: MTLComputePipelineState, _ set: (MTLComputeCommandEncoder) -> Void) {
            guard let e = cb.makeComputeCommandEncoder() else { return }
            e.setComputePipelineState(pipe)
            set(e)
            let tw = pipe.threadExecutionWidth, th = max(1, pipe.maxTotalThreadsPerThreadgroup / tw)
            e.dispatchThreadgroups(MTLSize(width: (w + tw - 1) / tw, height: (h + th - 1) / th, depth: 1),
                                   threadsPerThreadgroup: MTLSize(width: tw, height: th, depth: 1))
            e.endEncoding()
        }
        func blur(_ from: MTLTexture, _ to: MTLTexture, _ sigma: Float) {
            let s = max(sigma, 0.0001)
            let k = blurs[s] ?? {
                let k = MPSImageGaussianBlur(device: device, sigma: s)
                k.edgeMode = .zero
                if blurs.count > 16 { blurs = [:] }
                blurs[s] = k
                return k
            }()
            k.encode(commandBuffer: cb, sourceTexture: from, destinationTexture: to)
        }
        for (k, step) in steps.enumerated() {
            let last = k == steps.count - 1
            guard let out = last ? target : pick([current]) else { return false }
            switch step {
            case .blur(let s):
                blur(current, out, s)
            case .offset(let dx, let dy):
                dispatch(offsetPipe) { e in var dd = SIMD2<Int32>(dx, dy); e.setTexture(current, index: 0); e.setTexture(out, index: 1); e.setBytes(&dd, length: 8, index: 0) }
            case .matrix(let m):
                dispatch(matrixPipe) { e in e.setTexture(current, index: 0); e.setTexture(out, index: 1); e.setBytes(m, length: 80, index: 0) }
            case .shadow(let s, let dx, let dy, let color):
                guard let blurred = pick([current, out]) else { return false }
                blur(current, blurred, s)
                dispatch(shadowPipe) { e in
                    var c = color, dd = SIMD2<Int32>(dx, dy)
                    e.setTexture(current, index: 0); e.setTexture(blurred, index: 1); e.setTexture(out, index: 2)
                    e.setBytes(&c, length: 16, index: 0); e.setBytes(&dd, length: 8, index: 1)
                }
            }
            current = out
        }
        return true
    }
}
