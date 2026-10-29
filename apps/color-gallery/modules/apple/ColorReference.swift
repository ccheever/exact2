import Foundation
#if os(iOS)
import UIKit
import QuartzCore
#endif

final class ColorReference: ExactModule {
    override func call(_ request: [String: Any]) throws -> [String: Any] {
        guard request["op"] as? String == "colorReference" else { throw ExactNativeRefusal("unknown color reference operation") }
        #if os(iOS)
        if #available(iOS 26, *) { return ["available": true] }
        #endif
        return ["available": false]
    }

    #if os(iOS)
    private struct Probe { weak var view: UIView?; let diagnostics: ExactDiagnostics; let kind: String; let limit: String }
    private var probes: [String: Probe] = [:]
    private var reports: [String: [String: Any]] = [:]
    private var ticket: ExactTicket?
    private var lastSample = -Double.infinity
    #endif
    override func element(_ element: ExactElement) {
        #if os(iOS)
        guard #available(iOS 26, *), let view = element.view else { return }
        let limit = element.data[.limit] ?? "no-limit"
        if element.hatch == .hdrReference, element.isNew {
            let reference = UIView(frame: CGRect(x: 0, y: 0, width: 116, height: 112))
            let gradient = CAGradientLayer()
            gradient.frame = CGRect(x: 0, y: 0, width: 116, height: 32)
            gradient.startPoint = CGPoint(x: 0, y: 0.5)
            gradient.endPoint = CGPoint(x: 1, y: 0.5)
            let space = CGColorSpace(name: CGColorSpace.extendedLinearSRGB)!
            gradient.colors = [CGFloat(1), 4].map {
                CGColor(headroom: Float($0), colorSpace: space, red: $0, green: $0, blue: $0, alpha: 1)!
            }
            gradient.preferredDynamicRange = limit == "standard" ? .standard : limit == "constrained" ? .constrainedHigh : .high
            gradient.toneMapMode = .ifSupported
            reference.layer.addSublayer(gradient)
            let image = UIImageView(frame: CGRect(x: 0, y: 40, width: 116, height: 32))
            image.contentMode = .scaleToFill
            image.preferredImageDynamicRange = limit == "standard" ? .standard : limit == "constrained" ? .constrainedHigh : .high
            if let url = Bundle.main.url(forResource: "pq", withExtension: "heic", subdirectory: "assets") {
                image.image = UIImage(contentsOfFile: url.path)
            }
            reference.addSubview(image)
            let shadowBox = CALayer()
            shadowBox.frame = CGRect(x: 0, y: 80, width: 116, height: 32)
            shadowBox.backgroundColor = UIColor.black.cgColor
            let caster = CALayer()
            caster.frame = CGRect(x: 28, y: 10, width: 60, height: 12)
            caster.backgroundColor = UIColor.black.cgColor
            caster.shadowColor = CGColor(headroom: 4, colorSpace: space, red: 4, green: 4, blue: 4, alpha: 1)
            caster.shadowPath = CGPath(rect: caster.bounds.insetBy(dx: -4, dy: -4), transform: nil)
            caster.shadowOpacity = 1
            caster.shadowOffset = .zero
            caster.shadowRadius = 5
            caster.preferredDynamicRange = gradient.preferredDynamicRange
            shadowBox.addSublayer(caster)
            reference.layer.addSublayer(shadowBox)
            view.addSubview(reference)
            element.owns(view: reference, "UIKit color reference")
        }
        guard context.agent, element.hatch == .hdrProbe || element.hatch == .hdrReference else { return }
        let kind = element.data[.kind] ?? "native"
        // Diagnostics allow 16 names per session; one paint kind holds its three limits.
        probes["\(kind)-\(limit)"] = Probe(view: view, diagnostics: element.diagnostics, kind: kind, limit: limit)
        if ticket == nil {
            ticket = context.frames { [weak self] frame in
                guard let self, frame.now - self.lastSample >= 500 else { return }
                self.lastSample = frame.now
                CATransaction.flush()
                for probe in self.probes.values {
                    guard let view = probe.view, let window = view.window,
                          view.convert(view.bounds, to: window).intersects(window.bounds) else { continue }
                    self.reports[probe.kind, default: [:]][probe.limit] = ["layers": Self.layers(view.layer), "sample": Self.sample(view.layer)]
                    probe.diagnostics.publish(probe.kind, self.reports[probe.kind]!)
                }
            }
        }
        #endif
    }

    #if os(iOS)
    @available(iOS 26, *)
    private static func layers(_ layer: CALayer) -> [[String: Any]] {
        var colors = [layer.backgroundColor, layer.borderColor, layer.shadowColor]
        if let shape = layer as? CAShapeLayer { colors += [shape.fillColor, shape.strokeColor] }
        if let gradient = layer as? CAGradientLayer { colors += (gradient.colors as? [CGColor] ?? []).map { $0 } }
        let own: [String: Any] = ["type": String(describing: type(of: layer)),
            "range": layer.preferredDynamicRange.rawValue, "headroom": layer.contentsHeadroom,
            "format": layer.contentsFormat.rawValue,
            "colors": colors.compactMap { $0?.components }, "sample": sample(layer)]
        let paintsExtended = layer.contents != nil || layer.contentsHeadroom > 1 || colors.contains {
            $0?.components?.prefix(3).contains(where: { $0 > 1 || $0 < 0 }) == true
        }
        return (paintsExtended ? [own] : []) + (layer.sublayers ?? []).flatMap { layers($0) }
    }

    // The same floating-point capture as the agent, before display tone mapping.
    private static func sample(_ layer: CALayer) -> [String: Any] {
        let w = Int(layer.bounds.width.rounded(.up)), h = Int(layer.bounds.height.rounded(.up))
        guard w > 0, h > 0, w * h <= 100_000,
              let space = CGColorSpace(name: CGColorSpace.extendedLinearSRGB),
              let ctx = CGContext(data: nil, width: w, height: h, bitsPerComponent: 32, bytesPerRow: w * 16,
                                  space: space, bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue |
                                  CGBitmapInfo.floatComponents.rawValue | CGBitmapInfo.byteOrder32Little.rawValue),
              let data = ctx.data else { return [:] }
        layer.render(in: ctx)
        let pixels = data.assumingMemoryBound(to: Float.self)
        let values = (0..<w * h).map { pixels[$0 * 4] }
        return ["maxRed": values.max() ?? 0,
                "middleRed": [0.125, 0.375, 0.625, 0.875].map { values[(h / 2) * w + Int(Double(w) * $0)] }]
    }
    #endif
}

let exactModule: ExactModule.Type = ColorReference.self
