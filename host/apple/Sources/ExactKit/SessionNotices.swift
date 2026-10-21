// System notices (the app became active, a window moved, a preference
// changed) told to the runtime without the main thread waiting.
// @ref LLP 1027.007 P1
#if canImport(UIKit)
import UIKit
#else
import AppKit
#endif
import ExactCore

extension ExactSession {
    /// What a system notice tells the runtime (the app became active, a
    /// window moved, a preference changed) while the owner activates the data
    /// source, sent there without main waiting (LLP 1027.007 P1): on a cold
    /// launch iOS posts these at first pixel. The batches land in the owner's
    /// order, before any batch made after them (T4). At any other time a
    /// notice is told at once, so it has taken effect when it returns.
    func tellSoon(_ calls: [(Runtime) -> Batch]) {
        noticesInFlight += calls.count
        let captured = generation
        runtime.later(calls) { [weak self] batch in self?.publish(.notice, batch, captured) }
    }

    /// `tellPage` for a notice; at once unless the data source is activating
    /// and this generation's `onLine` reading is known.
    func tellPageSoon() {
        guard booted, state != .destroyed else { return }
        guard dataPreparing, let reads = readsOnLineKnown, reads.generation == generation, !Owner.shared.isServingMain else { return tellPage() }
        #if canImport(UIKit)
        if view?.window?.windowScene?.activationState != .foregroundActive { presenter.supersedePress() }
        #endif
        let bits = PageFacts.bits(pageWindow, page: view?.window, readsOnLine: reads.value)
        let font = PageFacts.rootFontSize, scale = PageFacts.displayScale(presenter.viewport)
        tellSoon([{ $0.setPage(bits) }, { $0.setRootFontSize(font) }, { $0.setDisplayScale(scale) }])
    }

    /// `tellPreferences` for a notice; at once unless the data source is
    /// activating. The colours go with it unless a batch is being applied, as
    /// `reportColors` decides.
    func tellPreferencesSoon() {
        guard booted, state != .destroyed else { return }
        guard dataPreparing, !Owner.shared.isServingMain else { return tellPreferences() }
        let bits = preferenceBits()
        var calls: [(Runtime) -> Batch] = [{ $0.setPreferences(bits) }]
        #if os(iOS) || os(tvOS)
        let reports = view?.window != nil
        #else
        let reports = true
        #endif
        if reports, applying || awaitingTree { colorsOwed = true } else if reports {
            let tint = SystemColor.appTint.flatMap { SystemColor.tintPair($0) }
            calls.append { $0.reportColors(tint: tint) }
        }
        tellSoon(calls)
        app.scheduleStatus()
        #if os(macOS)
        controlTextChanged()
        #endif
    }
}
