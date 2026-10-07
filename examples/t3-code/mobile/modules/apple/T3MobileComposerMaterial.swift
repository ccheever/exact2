#if os(iOS)
// @ref llp/1107.005-composer-and-transcript.decision.md#composer-material
// Pinned365aa87982 ComposerSurface/GlassSurface and installed Expo glass/blur lifecycle.
// This passive leaf owns only its effect and tint views; Contract owns the editor and card.
import UIKit

struct T3ComposerMaterialConfiguration: Equatable {
    let dark: Bool
    let radius: Double
    let surface: String
    let border: String

    init(_ props: [String: String]) throws {
        guard let appearance = props["material-appearance"], ["light", "dark"].contains(appearance),
              let radius = Double(props["material-radius"] ?? ""), radius.isFinite, radius >= 0,
              let surface = props["material-surface"], let border = props["material-border"],
              Self.channels(surface) != nil, Self.channels(border) != nil else {
            throw ExactNativeRefusal("Invalid composer material configuration")
        }
        dark = appearance == "dark"; self.radius = radius; self.surface = surface; self.border = border
    }

    static func channels(_ text: String) -> [Double]? {
        if text.hasPrefix("#"), text.count == 7 || text.count == 9,
           let hex = UInt64(text.dropFirst(), radix: 16) {
            let rgb = text.count == 9 ? hex >> 8 : hex
            return [Double((rgb >> 16) & 255) / 255, Double((rgb >> 8) & 255) / 255,
                    Double(rgb & 255) / 255, text.count == 9 ? Double(hex & 255) / 255 : 1]
        }
        if text.hasPrefix("rgba("), text.hasSuffix(")") {
            let parts = text.dropFirst(5).dropLast().split(separator: ",", omittingEmptySubsequences: false)
            let values = parts.compactMap { Double($0.trimmingCharacters(in: .whitespaces)) }
            if parts.count == 4, values.count == 4, values.allSatisfy({ $0.isFinite }),
               values.prefix(3).allSatisfy({ (0...255).contains($0) }), (0...1).contains(values[3]) {
                return [values[0] / 255, values[1] / 255, values[2] / 255, values[3]]
            }
        }
        return nil
    }
}

// The source waits until its glass can render, including inherited opacity.
struct T3ComposerMaterialMount {
    private(set) var alive = true
    private(set) var installed = false
    mutating func invalidate() { installed = false }
    mutating func destroy() { alive = false; installed = false }
    mutating func install(attached: Bool, width: Double, height: Double, opacity: Double, hidden: Bool) -> Bool {
        guard alive, !installed, attached, width > 0, height > 0, !hidden, opacity > 0.02 else { return false }
        installed = true
        return true
    }
}

final class T3MobileComposerMaterial: ExactNativeInstance {
    static let factory = ExactNativeFactory { props, events in
        let instance = T3MobileComposerMaterial(events: events)
        try instance.setProps(props)
        return instance
    }
    private let material = T3ComposerMaterialView()
    override var view: UIView { material }
    override func setProps(_ props: [String: String]) throws {
        material.configure(try T3ComposerMaterialConfiguration(props))
    }
    override func destroy() { material.destroy() }
}

private final class T3ComposerMaterialDisplayLink: NSObject {
    weak var view: T3ComposerMaterialView?
    init(_ view: T3ComposerMaterialView) { self.view = view }
    @objc func tick() { view?.visibilityTick() }
}

private final class T3ComposerMaterialView: UIView {
    private let effectView = UIVisualEffectView()
    private let tintView = UIView()
    private let nativeGlass: Bool
    private var configuration: T3ComposerMaterialConfiguration?
    private var mount = T3ComposerMaterialMount()
    private var visibilityLink: CADisplayLink?
    private var blurAnimator: UIViewPropertyAnimator?

    init() {
        if #available(iOS 26.0, *),
           let type = NSClassFromString("UIGlassEffect") as? NSObject.Type {
            nativeGlass = type.responds(to: NSSelectorFromString("effectWithStyle:"))
        } else { nativeGlass = false }
        super.init(frame: .zero)
        isUserInteractionEnabled = false
        isAccessibilityElement = false
        accessibilityElementsHidden = true
        backgroundColor = .clear
        clipsToBounds = true
        for child in [effectView, tintView] {
            child.isUserInteractionEnabled = false
            child.isAccessibilityElement = false
            addSubview(child)
        }
    }
    required init?(coder: NSCoder) { nil }
    override func hitTest(_ point: CGPoint, with event: UIEvent?) -> UIView? { nil }

    func configure(_ next: T3ComposerMaterialConfiguration) {
        guard mount.alive, next != configuration else { return }
        let changedAppearance = configuration?.dark != next.dark
        configuration = next
        overrideUserInterfaceStyle = next.dark ? .dark : .light
        effectView.overrideUserInterfaceStyle = overrideUserInterfaceStyle
        if changedAppearance {
            mount.invalidate()
            stopBlur()
        }
        tintView.backgroundColor = nativeGlass ? .clear : color(next.surface)
        tintView.layer.borderColor = nativeGlass ? UIColor.clear.cgColor : color(next.border).cgColor
        tintView.layer.borderWidth = nativeGlass ? 0 : 1
        setNeedsLayout()
    }

    override func layoutSubviews() {
        super.layoutSubviews()
        guard mount.alive, let configuration else { return }
        effectView.frame = bounds
        tintView.frame = bounds
        let radius = CGFloat(configuration.radius)
        layer.cornerRadius = radius
        tintView.layer.cornerRadius = radius
        if #available(iOS 26.0, *), nativeGlass {
            let corner = UICornerRadius(floatLiteral: radius)
            effectView.cornerConfiguration = .corners(topLeftRadius: corner, topRightRadius: corner,
                bottomLeftRadius: corner, bottomRightRadius: corner)
        } else {
            effectView.layer.cornerRadius = radius
            effectView.clipsToBounds = true
        }
        guard !mount.installed else { return }
        let visibility = inheritedVisibility
        if mount.install(attached: window != nil, width: Double(bounds.width), height: Double(bounds.height),
                         opacity: nativeGlass ? visibility.opacity : 1, hidden: nativeGlass && visibility.hidden) {
            stopWaiting()
            installEffect(configuration)
        } else if window != nil, nativeGlass, visibilityLink == nil {
            let link = CADisplayLink(target: T3ComposerMaterialDisplayLink(self), selector: #selector(T3ComposerMaterialDisplayLink.tick))
            link.add(to: .main, forMode: .common)
            visibilityLink = link
        }
    }

    private var inheritedVisibility: (opacity: Double, hidden: Bool) {
        var opacity: CGFloat = 1, hidden = false
        var ancestor: UIView? = self
        while let current = ancestor {
            hidden = hidden || current.isHidden
            opacity *= current.alpha
            ancestor = current.superview
        }
        return (Double(opacity), hidden)
    }
    fileprivate func visibilityTick() {
        guard mount.alive, window != nil, bounds.width > 0, bounds.height > 0 else { return }
        let visibility = inheritedVisibility
        if !visibility.hidden && visibility.opacity > 0.02 { setNeedsLayout() }
    }

    private func installEffect(_ configuration: T3ComposerMaterialConfiguration) {
        stopBlur()
        if #available(iOS 26.0, *), nativeGlass {
            // Expo clears the stale effect before a layout-time installation or re-entry.
            effectView.effect = UIVisualEffect()
            let effect = UIGlassEffect(style: .regular)
            effect.tintColor = .clear
            effect.isInteractive = false
            effectView.effect = effect
        } else {
            effectView.effect = nil
            let blur = UIBlurEffect(style: configuration.dark ? .dark : .regular)
            let animator = UIViewPropertyAnimator(duration: 1, curve: .linear) { [weak effectView] in
                effectView?.effect = blur
            }
            animator.fractionComplete = 0.8
            blurAnimator = animator
        }
    }

    override func didMoveToWindow() {
        super.didMoveToWindow()
        guard mount.alive else { return }
        if window == nil {
            mount.invalidate()
            stopWaiting()
            stopBlur()
            effectView.effect = nil
        } else { setNeedsLayout() }
    }

    private func color(_ text: String) -> UIColor {
        // Configuration validates both colors before any view is changed.
        let channels = T3ComposerMaterialConfiguration.channels(text)!
        return UIColor(red: channels[0], green: channels[1], blue: channels[2], alpha: channels[3])
    }
    private func stopWaiting() { visibilityLink?.invalidate(); visibilityLink = nil }
    private func stopBlur() { blurAnimator?.stopAnimation(true); blurAnimator = nil }
    func destroy() {
        mount.destroy()
        stopWaiting()
        stopBlur()
        effectView.effect = nil
    }
    deinit { visibilityLink?.invalidate(); blurAnimator?.stopAnimation(true) }
}
#endif
