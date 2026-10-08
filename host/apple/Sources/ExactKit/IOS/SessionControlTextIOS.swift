// @ref LLP 1104 D4: fonts/chrome are ready before the first runtime layout.
#if os(iOS) || os(tvOS)
import UIKit
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
        let traits = view?.traitCollection ?? presenter.viewport.traitCollection
        _ = fieldChrome.configure(traits)
        _ = buttonMeasurements.configure(traits, in: view ?? presenter.viewport)
    }
    func controlTextChanged() {
        guard state != .destroyed else { return }
        let traits = view?.traitCollection ?? presenter.viewport.traitCollection
        let fields = fieldChrome.configure(traits), buttons = buttonMeasurements.configure(traits, in: view ?? presenter.viewport)
        if (fields || buttons), booted {
            apply(runtime.on { runtime.read(exact_control_text_changed(runtime.rt)) })
        }
    }
}
#endif
