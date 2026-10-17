// The Tamber binding's sample module (LLP 1121 D12, stage 5): `exact-swatch`
// fills itself with the design's `swatch.fill` through `ExactTamber`, and
// since stage 6 G sets a label in `type.label` inset by `swatch.inset`, and
// says each colour, inset and font it applies as a `message` (`swatch
// #rrggbbaa inset 16 font 17 <face>`), so a drive reads what it follows: a
// mode, a pin above it, the root font size (Dynamic Type), Bold Text, a
// live edit. It asks once it is in a window, where its place is known, and
// again when its observation fires or its appearance changes.
import Foundation
#if os(macOS)
import AppKit
#else
import UIKit
#endif

let exactModule: ExactModule.Type = SwatchModule.self

final class SwatchModule: ExactModule {
    override class var views: [String: ExactNativeFactory] {
        ["exact-swatch": ExactNativeFactory { _, events in Swatch(events: events) }]
    }
}

final class Swatch: ExactNativeInstance {
    private let box = SwatchView(frame: .zero)
    override var view: ExactNativeView { box }
    override init(events: ExactNativeEvents) {
        super.init(events: events)
        box.said = { [weak self] hex in self?.events.message("swatch \(hex)") }
    }
}

final class SwatchView: ExactNativeView {
    var said: ((String) -> Void)?
    private var observation: ExactTamberObservation?
    private var applied = false
    #if os(macOS)
    private let label = NSTextField(labelWithString: "Aa")
    #else
    private let label = UILabel()
    #endif

    func apply() {
        let tamber = ExactTamber(self)
        let fill = tamber.color(ExactTamberToken("swatch.fill"))
        let inset = tamber.length(ExactTamberToken("swatch.inset"), fallback: 0)
        #if os(macOS)
        let font = tamber.font(ExactTamberToken("type.label"), fallback: .systemFont(ofSize: 9))
        if label.superview == nil { addSubview(label) }
        label.font = font
        label.frame = NSRect(x: inset, y: inset, width: max(0, bounds.width - 2 * inset), height: font.pointSize * 1.4)
        #else
        let font = tamber.font(ExactTamberToken("type.label"), fallback: .systemFont(ofSize: 9))
        if label.superview == nil { label.text = "Aa"; addSubview(label) }
        label.font = font
        label.frame = CGRect(x: inset, y: inset, width: max(0, bounds.width - 2 * inset), height: font.lineHeight)
        #endif
        #if os(macOS)
        wantsLayer = true
        var cg: CGColor?
        effectiveAppearance.performAsCurrentDrawingAppearance { cg = fill.cgColor }
        layer?.backgroundColor = cg
        let rgba = cg.flatMap { NSColor(cgColor: $0)?.usingColorSpace(.sRGB) }
        let parts = rgba.map { [$0.redComponent, $0.greenComponent, $0.blueComponent, $0.alphaComponent] } ?? []
        #else
        let resolved = fill.resolvedColor(with: traitCollection)
        layer.backgroundColor = resolved.cgColor
        var r: CGFloat = 0, g: CGFloat = 0, b: CGFloat = 0, a: CGFloat = 0
        resolved.getRed(&r, green: &g, blue: &b, alpha: &a)
        let parts = [r, g, b, a]
        #endif
        applied = true
        // Said each time it applies: a moved input that keeps the colour shows too.
        let hex = "#" + parts.map { String(format: "%02x", Int(($0 * 255).rounded())) }.joined()
        #if os(macOS)
        let weight = NSFontManager.shared.weight(of: font)
        #else
        let weight = Int(((font.fontDescriptor.object(forKey: .traits) as? [UIFontDescriptor.TraitKey: Any])?[.weight] as? CGFloat ?? 0) * 1000)
        #endif
        // A system stack's design (`ui-monospace`) is the module library's to keep.
        let code = tamber.font(ExactTamberToken("type.code"), fallback: .systemFont(ofSize: 9))
        #if os(macOS)
        let fixed = code.isFixedPitch
        #else
        let fixed = code.fontDescriptor.symbolicTraits.contains(.traitMonoSpace)
        #endif
        said?("\(hex) inset \(Int(inset.rounded())) font \(Int(font.pointSize.rounded())) \(font.fontName) w\(weight) code \(fixed ? "fixed" : "proportional")")
    }

    private func start() {
        guard window != nil else { return }
        if observation == nil { observation = ExactTamber(self).observe { [weak self] in self?.apply() } }
        apply()
    }

    #if os(macOS)
    override func viewDidMoveToWindow() { super.viewDidMoveToWindow(); start() }
    override func viewDidChangeEffectiveAppearance() { super.viewDidChangeEffectiveAppearance(); if applied { apply() } }
    #else
    override func didMoveToWindow() { super.didMoveToWindow(); start() }
    override func traitCollectionDidChange(_ previous: UITraitCollection?) {
        super.traitCollectionDidChange(previous)
        if applied, previous?.hasDifferentColorAppearance(comparedTo: traitCollection) ?? true { apply() }
    }
    #endif
}
