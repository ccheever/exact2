// The Tamber binding's sample module (LLP 1121 D12, stage 5): `exact-swatch`
// fills itself with the design's `swatch.fill` through `ExactTamber`, and
// says each colour it applies as a `message` (`swatch #rrggbbaa`), so a drive
// reads what it follows: a mode, a pin above it, the root font size (Dynamic
// Type), a live edit. It asks once it is in a window, where its place is
// known, and again when its observation fires or its appearance changes.
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

    func apply() {
        let fill = ExactTamber(self).color(ExactTamberToken("swatch.fill"))
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
        said?("#" + parts.map { String(format: "%02x", Int(($0 * 255).rounded())) }.joined())
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
