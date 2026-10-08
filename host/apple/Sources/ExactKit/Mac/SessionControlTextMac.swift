// @ref LLP 1104 D4: the font and chrome cache precede the first layout.
#if os(macOS)
import AppKit
import CExact
extension ExactSession {
    func installControlText() { installControlText(on: text) }
    func installControlText(on engine: TextEngine) {
        engine.fieldChrome = fieldChrome
        Owner.shared.sync { engine.measuring.fieldChrome = fieldChrome }
        runtime.on { exact_set_control_text(runtime.rt, TextEngine.controlText, TextEngine.fieldChromeMeasure) }
    }
    func primeControlText() {
        let surface = view ?? presenter.viewport
        _ = fieldChrome.configure(surface.effectiveAppearance, scale: surface.window?.backingScaleFactor ?? NSScreen.main?.backingScaleFactor ?? 1)
    }
    func controlTextChanged() {
        guard state != .destroyed else { return }
        let surface = view ?? presenter.viewport
        if fieldChrome.configure(surface.effectiveAppearance, scale: surface.window?.backingScaleFactor ?? NSScreen.main?.backingScaleFactor ?? 1), booted {
            apply(runtime.on { runtime.read(exact_control_text_changed(runtime.rt)) })
        }
    }
}
#endif
