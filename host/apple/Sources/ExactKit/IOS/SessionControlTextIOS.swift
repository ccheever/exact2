// @ref LLP 1104 D4: fonts/chrome are ready before the first runtime layout.
#if os(iOS) || os(tvOS)
import UIKit
import CExact
extension ExactSession {
    func installControlText() { installControlText(on: text) }
    func installControlText(on engine: TextEngine) {
        engine.fieldChrome = fieldChrome
        Owner.shared.sync { engine.measuring.fieldChrome = fieldChrome }
        runtime.on { exact_set_control_text(runtime.rt, TextEngine.controlText, TextEngine.fieldChromeMeasure) }
    }
    func primeControlText() {
        _ = fieldChrome.configure(view?.traitCollection ?? presenter.viewport.traitCollection)
    }
    func controlTextChanged() {
        guard state != .destroyed else { return }
        if fieldChrome.configure(view?.traitCollection ?? presenter.viewport.traitCollection), booted {
            apply(runtime.on { runtime.read(exact_control_text_changed(runtime.rt)) })
        }
    }
}
#endif
