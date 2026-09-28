// @ref LLP 1056 D9 — Canvas 2D images and pixels on Apple, and the
// presenter's canvases. An image handle is the string an `image` node's
// `src` takes: an app asset, or an http(s) URL. The runner names the handles
// its draws asked for (the batch's `canvasImages`); the host decodes each
// off the main thread and answers `exact_canvas_image`, which redraws the
// canvases that asked. `putImageData` writes raw backing pixels: no
// transform, clip, alpha, compositing or shadow.
import CoreGraphics
import Foundation
import ImageIO
import QuartzCore

/// No implicit animations: new pixels appear with the batch that drew them.
private final class Instant: NSObject, CALayerDelegate {
    static let shared = Instant()
    func action(for layer: CALayer, forKey event: String) -> CAAction? { NSNull() }
}

extension Canvas2DReplayer {
    /// `drawImage`: `image sx sy sw sh dx dy dw dh`, the source already
    /// clipped to the image; the destination in user space.
    func drawImage(_ c: CGContext, _ n: [Double]) {
        guard let src = imageSources[UInt32(n[0])], let image = env?.canvasImage(src) else { return }
        let (sx, sy, sw, sh) = (n[1], n[2], n[3], n[4])
        let dest = CGRect(x: n[5], y: n[6], width: n[7], height: n[8])
        guard sw > 0, sh > 0 else { return }
        let (kx, ky) = (dest.width / sw, dest.height / sh)
        let whole = CGRect(x: dest.minX - sx * kx, y: dest.minY - sy * ky,
                           width: CGFloat(image.width) * kx, height: CGFloat(image.height) * ky)
        render(c) { c in
            c.concatenate(state.author)
            c.clip(to: dest)
            // Upright in the y-down space.
            c.translateBy(x: 0, y: whole.minY + whole.maxY)
            c.scaleBy(x: 1, y: -1)
            c.draw(image, in: whole)
        }
    }

    /// `putImageData`: `x y w h` in backing pixels, then w × h RGBA pixels.
    func putImageData(_ c: CGContext, _ n: [Double], _ count: Int) {
        let (x, y, w, h) = (Int(n[0]), Int(n[1]), Int(n[2]), Int(n[3]))
        guard w > 0, h > 0, count >= 4 + w * h else { return }
        var bytes = [UInt8](repeating: 0, count: w * h * 4)
        for i in 0..<(w * h) {
            let v = UInt32(n[4 + i])
            bytes[i * 4] = UInt8(v >> 24); bytes[i * 4 + 1] = UInt8((v >> 16) & 0xff)
            bytes[i * 4 + 2] = UInt8((v >> 8) & 0xff); bytes[i * 4 + 3] = UInt8(v & 0xff)
        }
        guard let provider = CGDataProvider(data: Data(bytes) as CFData),
              let image = CGImage(width: w, height: h, bitsPerComponent: 8, bitsPerPixel: 32, bytesPerRow: w * 4, space: canvas2DSRGB,
                                  bitmapInfo: CGBitmapInfo(rawValue: CGImageAlphaInfo.last.rawValue), provider: provider,
                                  decode: nil, shouldInterpolate: false, intent: .defaultIntent) else { return }
        c.saveGState()
        c.resetClip()
        c.concatenate(c.ctm.inverted())
        c.setBlendMode(.copy)
        c.setAlpha(1)
        c.setShadow(offset: .zero, blur: 0, color: nil)
        c.interpolationQuality = .none
        c.draw(image, in: CGRect(x: x, y: height - y - h, width: w, height: h))
        c.restoreGState()
    }
}

/// A presenter's 2D canvases, by view id: the `canvas2d` op, the decoded
/// image handles they draw, and cleanup when a view goes. The bitmap shows
/// in a sublayer below the view's children, framed to the content box.
final class Canvas2DHost: Canvas2DEnv {
    private var replayers: [UInt32: Canvas2DReplayer] = [:]
    private var layers: [UInt32: CALayer] = [:]
    /// Lists that could not be read, for `logs`.
    var errors: [String] = []
    /// The views' real scale differs from what the canvases were drawn at.
    var onScale: ((CGFloat) -> Void)?
    private var reported: CGFloat = 0
    /// The session's text engine (LLP 1056 D8).
    var textEngine: (() -> TextEngine?)?
    /// An app asset's bytes.
    var assetBytes: ((String) -> Data?)?
    /// A handle decoded (its image) or not (nil): the session tells the runtime.
    var onImage: ((String, CGImage?) -> Void)?
    private var images: [String: CGImage] = [:]
    private var loading: Set<String> = []

    var canvasText: CanvasText? { textEngine?()?.canvasText }
    func canvasImage(_ src: String) -> CGImage? { images[src] }
    /// Handles being decoded: the agent's settle waits for them.
    var loadingCount: Int { loading.count }

    /// Decode the handles the runner asked for, off the main thread.
    func load(_ srcs: [String]) {
        for src in srcs where images[src] == nil && !loading.contains(src) {
            loading.insert(src)
            let remote = URL(string: src).flatMap { $0.scheme == "http" || $0.scheme == "https" ? $0 : nil }
            let local = remote == nil ? assetBytes?(src.hasPrefix("/") ? String(src.dropFirst()) : src) : nil
            DispatchQueue.global(qos: .userInitiated).async { [weak self] in
                var bytes = local
                if let remote {
                    let done = DispatchSemaphore(value: 0)
                    var request = URLRequest(url: remote); request.timeoutInterval = 20
                    URLSession.shared.dataTask(with: request) { data, response, _ in
                        if let http = response as? HTTPURLResponse, (200..<300).contains(http.statusCode) { bytes = data }
                        done.signal()
                    }.resume()
                    done.wait()
                }
                let image = bytes.flatMap { CGImageSourceCreateWithData($0 as CFData, nil) }
                    .flatMap { CGImageSourceCreateImageAtIndex($0, 0, [kCGImageSourceShouldCacheImmediately: true] as CFDictionary) }
                DispatchQueue.main.async {
                    guard let self else { return }
                    self.loading.remove(src)
                    if let image { self.images[src] = image }
                    self.onImage?(src, image)
                }
            }
        }
    }

    func apply(_ id: UInt32, _ payload: [String: Any], layer parent: CALayer?) {
        guard let parent else { return }
        let num = { (key: String) -> Double in (payload[key] as? NSNumber)?.doubleValue ?? 0 }
        let lifetime = UInt64(num("lifetime")), generation = UInt32(num("generation"))
        if (payload["fresh"] as? NSNumber)?.boolValue == true {
            replayers[id] = Canvas2DReplayer(width: Int(num("w")), height: Int(num("h")), scale: num("scale"),
                                             lifetime: lifetime, generation: generation)
        }
        guard let r = replayers[id], r.lifetime == lifetime, r.generation == generation else { return }
        r.env = self
        for case let text as String in payload["lists"] as? [Any] ?? [] {
            guard let data = Data(base64Encoded: text), r.apply(data) else {
                errors.append("canvas \(id): unreadable list"); continue
            }
        }
        let layer = layers[id] ?? {
            let l = CALayer(); l.delegate = Instant.shared; l.contentsGravity = .resize
            l.magnificationFilter = .linear; l.minificationFilter = .linear
            layers[id] = l; return l
        }()
        if layer.superlayer !== parent { parent.insertSublayer(layer, at: 0) }
        let box = (payload["box"] as? [Any])?.compactMap { ($0 as? NSNumber)?.doubleValue } ?? []
        if box.count == 4 { layer.frame = CGRect(x: box[0], y: box[1], width: box[2], height: box[3]) }
        // A rounded canvas clips its bitmap to the content edge's curve, as
        // the web clips replaced content.
        let radii = (payload["radii"] as? [Any])?.compactMap { ($0 as? NSNumber).map { CGFloat($0.doubleValue) } } ?? []
        if radii.count == 4, radii.contains(where: { $0 > 0 }) {
            let mask = (layer.mask as? CAShapeLayer) ?? CAShapeLayer()
            mask.path = Canvas2DHost.rounded(CGRect(origin: .zero, size: layer.bounds.size), radii)
            layer.mask = mask
        } else {
            layer.mask = nil
        }
        layer.contentsScale = max(1, num("scale"))
        layer.contents = r.image()
        let actual = parent.contentsScale
        if (payload["stretch"] as? NSNumber)?.boolValue != true, actual >= 1, abs(actual - num("scale")) > 0.01, actual != reported {
            reported = actual
            onScale?(actual)
        }
    }

    /// A rectangle with four corner radii (top-left, top-right, bottom-right,
    /// bottom-left), in a y-down layer.
    static func rounded(_ r: CGRect, _ radii: [CGFloat]) -> CGPath {
        let p = CGMutablePath(), (x0, y0, x1, y1) = (r.minX, r.minY, r.maxX, r.maxY)
        let f = min(1, r.width / max(radii[0] + radii[1], radii[2] + radii[3], 1e-9), r.height / max(radii[0] + radii[3], radii[1] + radii[2], 1e-9))
        let (tl, tr, br, bl) = (radii[0] * f, radii[1] * f, radii[2] * f, radii[3] * f)
        p.move(to: CGPoint(x: x0 + tl, y: y0))
        p.addLine(to: CGPoint(x: x1 - tr, y: y0))
        p.addArc(tangent1End: CGPoint(x: x1, y: y0), tangent2End: CGPoint(x: x1, y: y0 + tr), radius: tr)
        p.addLine(to: CGPoint(x: x1, y: y1 - br))
        p.addArc(tangent1End: CGPoint(x: x1, y: y1), tangent2End: CGPoint(x: x1 - br, y: y1), radius: br)
        p.addLine(to: CGPoint(x: x0 + bl, y: y1))
        p.addArc(tangent1End: CGPoint(x: x0, y: y1), tangent2End: CGPoint(x: x0, y: y1 - bl), radius: bl)
        p.addLine(to: CGPoint(x: x0, y: y0 + tl))
        p.addArc(tangent1End: CGPoint(x: x0, y: y0), tangent2End: CGPoint(x: x0 + tl, y: y0), radius: tl)
        p.closeSubpath()
        return p
    }

    func forget(_ id: UInt32) {
        replayers.removeValue(forKey: id)
        layers.removeValue(forKey: id)?.removeFromSuperlayer()
    }
}
