// 3 shader — an MTKView per shader cell drawing the wave + duotone over the photo texture every frame
// while the cell is displayed (MTKView's own display link, 120 Hz). The photo is decoded off the main
// thread at the box's pixel size (the same loader and key as a photo) and uploaded as a texture off main.
import MetalKit
import UIKit

final class MetalShared {
    static let shared = MetalShared()
    let device = MTLCreateSystemDefaultDevice()!
    lazy var queue = device.makeCommandQueue()!
    lazy var pipeline: MTLRenderPipelineState? = {
        // default.metallib is compiled at build time; the source is the fallback (see README).
        var lib = device.makeDefaultLibrary()
        if lib?.makeFunction(name: "xhFragment") == nil,
           let url = Bundle.main.url(forResource: "Shader", withExtension: "metal"),
           let src = try? String(contentsOf: url, encoding: .utf8) {
            lib = try? device.makeLibrary(source: src, options: nil)
        }
        guard let lib, let v = lib.makeFunction(name: "xhVertex"), let f = lib.makeFunction(name: "xhFragment") else { return nil }
        let d = MTLRenderPipelineDescriptor()
        d.vertexFunction = v
        d.fragmentFunction = f
        d.colorAttachments[0].pixelFormat = .bgra8Unorm
        return try? device.makeRenderPipelineState(descriptor: d)
    }()
    let textures = NSCache<NSString, MTLTexture>()
    let uploadQueue = DispatchQueue(label: "xheavy.texture", qos: .userInitiated)
    init() { textures.countLimit = 8 }
}

/// The decoded photo as an rgba8Unorm texture holding its sRGB-encoded bytes (the math runs on encoded values).
func makeTexture(_ cg: CGImage) -> MTLTexture? {
    let w = cg.width, h = cg.height, row = w * 4
    var bytes = [UInt8](repeating: 0, count: row * h)
    let ok = bytes.withUnsafeMutableBytes { p -> Bool in
        guard let ctx = CGContext(data: p.baseAddress, width: w, height: h, bitsPerComponent: 8, bytesPerRow: row,
                                  space: CGColorSpace(name: CGColorSpace.sRGB)!,
                                  bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue) else { return false }
        ctx.draw(cg, in: CGRect(x: 0, y: 0, width: w, height: h))
        return true
    }
    guard ok else { return nil }
    let d = MTLTextureDescriptor.texture2DDescriptor(pixelFormat: .rgba8Unorm, width: w, height: h, mipmapped: false)
    d.usage = .shaderRead
    d.storageMode = .shared
    guard let t = MetalShared.shared.device.makeTexture(descriptor: d) else { return nil }
    t.replace(region: MTLRegionMake2D(0, 0, w, h), mipmapLevel: 0, withBytes: bytes, bytesPerRow: row)
    return t
}

struct ShaderUniforms {
    var t: Float, pad: Float = 0, size: SIMD2<Float>, a: SIMD4<Float>, b: SIMD4<Float>, uvScale: SIMD2<Float>, uvOffset: SIMD2<Float>
}

func rgb4(_ s: String) -> SIMD4<Float> {
    let v = UInt32(s.dropFirst(), radix: 16) ?? 0
    return SIMD4(Float((v >> 16) & 0xFF) / 255, Float((v >> 8) & 0xFF) / 255, Float(v & 0xFF) / 255, 1)
}

final class ShaderView: MTKView, MTKViewDelegate {
    private var texture: MTLTexture?
    private var texAspect: Float = 1
    private var a = SIMD4<Float>(0, 0, 0, 1), b = SIMD4<Float>(1, 1, 1, 1)
    private var key = ""
    private var token = 0
    private var visible = false

    init() {
        super.init(frame: .zero, device: MetalShared.shared.device)
        colorPixelFormat = .bgra8Unorm
        (layer as? CAMetalLayer)?.colorspace = CGColorSpace(name: CGColorSpace.sRGB)
        framebufferOnly = true
        clearColor = MTLClearColor(red: 0xE5 / 255.0, green: 0xE5 / 255.0, blue: 0xEA / 255.0, alpha: 1)
        backgroundColor = .hairline
        preferredFramesPerSecond = 120
        isPaused = true
        enableSetNeedsDisplay = true
        delegate = self
    }
    required init(coder: NSCoder) { fatalError() }

    func load(photo: String, duoA: String, duoB: String, size: CGSize) {
        a = rgb4(duoA); b = rgb4(duoB)
        let k = "\(photo)@\(size.width)x\(size.height)"
        guard k != key else { return }
        key = k
        ImageLoader.shared.cancel(token)
        token = 0
        texture = MetalShared.shared.textures.object(forKey: k as NSString)
        if let texture { texAspect = Float(texture.width) / Float(texture.height); refresh(); return }
        refresh()
        token = ImageLoader.shared.load(photo, size) { [weak self] img in
            guard let self, self.key == k, let cg = img.cgImage else { return }
            self.token = 0
            MetalShared.shared.uploadQueue.async {
                let tex = makeTexture(cg)
                DispatchQueue.main.async {
                    guard let tex else { return }
                    MetalShared.shared.textures.setObject(tex, forKey: k as NSString)
                    guard self.key == k else { return }
                    self.texture = tex
                    self.texAspect = Float(tex.width) / Float(tex.height)
                    self.refresh()
                }
            }
        }
    }

    func setVisible(_ v: Bool) { visible = v; refresh() }

    private func refresh() {
        let animate = visible && !freeze && texture != nil
        isPaused = !animate
        enableSetNeedsDisplay = !animate
        if !animate { setNeedsDisplay() }
    }

    func mtkView(_ view: MTKView, drawableSizeWillChange size: CGSize) {}

    func draw(in view: MTKView) {
        let S = MetalShared.shared
        guard let rpd = currentRenderPassDescriptor, let drawable = currentDrawable,
              let cb = S.queue.makeCommandBuffer(), let enc = cb.makeRenderCommandEncoder(descriptor: rpd) else { return }
        if let texture, let pipe = S.pipeline {
            let W = Float(bounds.width), H = Float(bounds.height)
            let boxAspect = W / max(H, 1)
            var scale = SIMD2<Float>(1, 1)
            if texAspect > boxAspect { scale.x = boxAspect / texAspect } else { scale.y = texAspect / boxAspect }
            var u = ShaderUniforms(t: Float(freeze ? 1.25 : motionTime()), size: SIMD2(W, H), a: a, b: b,
                                   uvScale: scale, uvOffset: (SIMD2<Float>(1, 1) - scale) / 2)
            enc.setRenderPipelineState(pipe)
            enc.setFragmentBytes(&u, length: MemoryLayout<ShaderUniforms>.stride, index: 0)
            enc.setFragmentTexture(texture, index: 0)
            enc.drawPrimitives(type: .triangle, vertexStart: 0, vertexCount: 3)
        }
        enc.endEncoding()
        cb.present(drawable)
        cb.commit()
    }
}

final class ShaderCell: FeedCell {
    let caption = label(sys(15), lines: 0)
    let shader = ShaderView()
    override func setup() {
        rounded(shader, 12)
        contentView.addSubview(caption)
        contentView.addSubview(shader)
    }
    override func configureBody(_ r: Row) {
        caption.text = r.caption
        shader.load(photo: r.photo!.src, duoA: r.duoA!, duoB: r.duoB!, size: CGSize(width: Geo.C, height: (Geo.C * 9 / 16).rounded()))
    }
    override func visibilityChanged() { shader.setVisible(visible) }
    override func layoutBody(x: CGFloat, y: CGFloat, C: CGFloat) -> CGFloat {
        let y1 = layoutCaption(caption, x: x, y: y, C: C)
        let h = (C * 9 / 16).rounded()
        shader.frame = CGRect(x: x, y: y1, width: C, height: h)
        return y1 + h - y
    }
}
