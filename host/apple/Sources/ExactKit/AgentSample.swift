// @ref LLP 1100 D12 — the agent's `sample`: the pixels under view points,
// from a float render of the view's layers, in extended linear sRGB, so
// what an 8-bit sRGB screenshot would clip reads outside 0–1.
import CoreGraphics
import QuartzCore
#if os(iOS) || os(tvOS)
import UIKit
#else
import AppKit
#endif

extension Agent {
    func sample(_ req: [String: Any]) -> [String: Any] {
        _ = settleForPicture()
        presenter.canvas2d.waitForReplays()
        let points = (req["points"] as? [[Any]] ?? []).compactMap { p -> CGPoint? in
            guard p.count == 2, let x = (p[0] as? NSNumber)?.doubleValue, let y = (p[1] as? NSNumber)?.doubleValue else { return nil }
            return CGPoint(x: x, y: y)
        }
        guard !points.isEmpty else { return ["error": "sample needs points: sample <x> <y> [<x> <y>…], in view points"] }
        let view = presenter.viewport
        #if os(iOS) || os(tvOS)
        let layer = view.layer
        #else
        guard let layer = view.layer else { return ["error": "the session's view has no layer"] }
        #endif
        let size = view.bounds.size
        let (w, h) = (Int(size.width.rounded(.up)), Int(size.height.rounded(.up)))
        guard w > 0, h > 0, let space = CGColorSpace(name: CGColorSpace.extendedLinearSRGB),
              let ctx = CGContext(data: nil, width: w, height: h, bitsPerComponent: 32, bytesPerRow: w * 16, space: space,
                                  bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue | CGBitmapInfo.floatComponents.rawValue
                                      | CGBitmapInfo.byteOrder32Little.rawValue),
              let data = ctx.data else { return ["error": "no extended-range context for the view"] }
        // Rows top-down, as view points are.
        ctx.translateBy(x: 0, y: CGFloat(h)); ctx.scaleBy(x: 1, y: -1)
        CATransaction.flush()
        layer.render(in: ctx)
        let px = data.assumingMemoryBound(to: Float.self)
        let values: [[Double]] = points.map { p in
            let (x, y) = (min(w - 1, max(0, Int(p.x))), min(h - 1, max(0, Int(p.y))))
            let i = (y * w + x) * 4
            let a = Double(px[i + 3])
            // Unpremultiplied.
            return (0..<3).map { Agent.r4(a > 0 ? Double(px[i + $0]) / a : 0) } + [Agent.r4(a)]
        }
        return ["space": "extended-linear-srgb", "values": values]
    }

    static func r4(_ v: Double) -> Double { (v * 10_000).rounded() / 10_000 }
}
