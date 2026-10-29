// System notices (the app became active, a window moved, a preference
// changed) told to the runtime without the main thread waiting.
// @ref LLP 1027.007 P1
#if canImport(UIKit)
import UIKit
#else
import AppKit
#endif
import ExactCore
import QuartzCore

extension ExactSession {
    func pumpReplies() {
        guard state != .destroyed else { return }
        let began = CACurrentMediaTime()
        let captured = generation, time = now()
        repliesInFlight += 1
        // Keep parsing, settlement and batch decoding on the existing owner.
        // Publication joins fills and ticks before any later synchronous batch.
        runtime.pumpAsync(now: time) { [weak self] batch in
            self?.publish(batch == nil ? .cancelledReply : .reply, batch ?? Batch(ops: [], timers: false, motion: false, clock: nil, error: nil), captured)
        }
        replySubmissions.append((CACurrentMediaTime() - began) * 1000)
    }

    func retireReply() -> Double {
        repliesInFlight -= 1
        return replySubmissions.removeFirst()
    }

    func retryActivation(generation replyGeneration: Int) {
        if let p = pendingActivation, p.generation == replyGeneration {
            pendingActivation = nil
            if p.generation == generation {
                activatedGeneration = nil
                firstDrawn(generation: p.generation, token: p.token)
            }
        }
    }

    /// What a system notice tells the runtime (the app became active, a
    /// window moved, a preference changed) while the owner activates the data
    /// source, sent there without main waiting (LLP 1027.007 P1): on a cold
    /// launch iOS posts these at first pixel. The batches land in the owner's
    /// order, before any batch made after them (T4). At any other time a
    /// notice is told at once, so it has taken effect when it returns.
    func tellSoon(_ calls: [(Runtime) -> Batch]) {
        guard !runtime.holds("tellSoon(_:)") else { return } // (a notice a restart's reset sets off: held before it is counted or queued, ResetGate)
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
        var calls: [(Runtime) -> Batch] = [{ $0.setPage(bits) }, { $0.setRootFontSize(font) }, { $0.setDisplayScale(scale) }]
        #if os(macOS)
        let zoom = rootZoom
        if zoom != 1 { calls.insert({ $0.setRootFontZoom(zoom) }, at: 2) } // View ▸ Zoom In (ZoomMac.swift)
        #endif
        tellSoon(calls)
    }

    /// @ref LLP 1061 D5 — told after every boot, as the date is, and on each
    /// change: in the same main-thread turn as the boot batch, so the first
    /// frame on screen already reads the user's preferences.
    func tellPreferences() {
        guard booted, state != .destroyed else { return }
        apply(runtime.setPreferences(preferenceBits()))
        // Increased Contrast changes what every platform colour resolves to; the build channel reports it (LLP 1122 D4).
        reportColors(); app.scheduleStatus()
        #if os(macOS)
        controlTextChanged()
        #endif
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
