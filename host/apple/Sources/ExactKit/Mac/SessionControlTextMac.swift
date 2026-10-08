// @ref LLP 1104 D4: the font and chrome cache precede the first layout.
#if os(macOS)
import AppKit
import CExact
extension ExactSession {
    func installControlText() { installControlText(on: text) }
    func installControlText(on engine: TextEngine) {
        engine.fieldChrome = fieldChrome
        engine.buttonMeasurements = buttonMeasurements
        Owner.shared.sync {
            engine.measuring.fieldChrome = fieldChrome
            engine.measuring.buttonMeasurements = buttonMeasurements
        }
        runtime.on { () -> Void in
            exact_set_control_text(runtime.rt, TextEngine.controlText, TextEngine.fieldChromeMeasure)
            exact_set_button_measure(runtime.rt, TextEngine.buttonMeasure)
        }
    }
    func primeControlText() {
        let surface = view ?? presenter.viewport
        _ = fieldChrome.configure(surface.effectiveAppearance, scale: surface.window?.backingScaleFactor ?? NSScreen.main?.backingScaleFactor ?? 1)
        _ = buttonMeasurements.configure(surface.effectiveAppearance, scale: surface.window?.backingScaleFactor ?? NSScreen.main?.backingScaleFactor ?? 1)
    }
    func controlTextChanged() {
        guard state != .destroyed else { return }
        let surface = view ?? presenter.viewport
        let scale = surface.window?.backingScaleFactor ?? NSScreen.main?.backingScaleFactor ?? 1
        let fields = fieldChrome.configure(surface.effectiveAppearance, scale: scale)
        let buttons = buttonMeasurements.configure(surface.effectiveAppearance, scale: scale)
        if (fields || buttons), booted {
            apply(runtime.on { runtime.read(exact_control_text_changed(runtime.rt)) })
        }
    }
}
#endif
