// @ref LLP 1104 D4: the font and chrome cache precede the first layout.
#if os(macOS)
import AppKit
import CExact
extension ExactSession {
    func installControlText() { installControlText(on: text) }
    func installControlText(on engine: TextEngine) {
        // Line boxes round to the window's device pixels, as Chrome's do (LLP 1008 §3).
        engine.setLineScale(backingScale)
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
    /// The device pixels a point of the session's window, else the main display's.
    var backingScale: CGFloat {
        let surface = view ?? presenter.viewport
        return surface.window?.backingScaleFactor ?? NSScreen.main?.backingScaleFactor ?? 1
    }
    func primeControlText() {
        let surface = view ?? presenter.viewport
        _ = fieldChrome.configure(surface.effectiveAppearance, scale: backingScale)
        _ = buttonMeasurements.configure(surface.effectiveAppearance, scale: backingScale)
        text.setLineScale(backingScale)
    }
    func controlTextChanged() {
        guard state != .destroyed else { return }
        let surface = view ?? presenter.viewport
        let scale = backingScale
        let fields = fieldChrome.configure(surface.effectiveAppearance, scale: scale)
        let buttons = buttonMeasurements.configure(surface.effectiveAppearance, scale: scale)
        if (fields || buttons), booted {
            apply(runtime.on { runtime.read(exact_control_text_changed(runtime.rt)) })
        }
        // A window on a display of another scale: every paragraph is measured again (LLP 1008 §3).
        if text.setLineScale(scale), booted {
            apply(runtime.on { runtime.read(exact_text_scale_changed(runtime.rt)) })
        }
    }
}
#endif
