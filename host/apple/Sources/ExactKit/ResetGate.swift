// A restart's reset (Session `presentCommitted`: a dev loop's generation, a fresh boot from the dev menu) commits the
// new runtime, then tears the old tree's views down, then applies the new runtime's committed batch. What the
// teardown sets off (a cover cleared, a sheet giving focus back, an edit ended, a key window moving, a due timer, a
// list's report) once reached the new runtime first, and the stale committed batch then undid what it answered: feed
// rows the runtime kept mounted with no view (the Apple pass's blank rows, every dev-joined iOS launch that reloaded).
// So every batch-answering runtime call is held over the teardown at its one entry (ExactCore `Runtime.on`, `held`):
// it reaches nothing and is named. Here, per call, what becomes of it once the committed batch is in: DROPPED, being
// about the old tree, or REPLAYED, being work the new runtime must still see, recomputed from the live presenter (the
// held call's own arguments name the old tree's views) and run once each, in this order. Not held: answers that
// aren't batches (`on(busy:)` reads, Void `on` sets), which leave the runtime's tree as it was. Host state a teardown
// would move with no runtime to tell stays guarded where it lives, on the presenter's `resetting`: the focus
// FocusEvents last reported, the blocks OwnersIOS last told, the geometry ExactViewIOS `fit` last told.
#if canImport(UIKit)
import UIKit
#else
import AppKit
#endif
import ExactCore

enum ResetGate {
    /// What a replay does, in the order replays run.
    /// Each replay re-reads the host's state as it is once the new tree is in, never a held call's arguments (which
    /// name the old tree's views, and are discarded): `fit` the view's bounds, safe area and keyboard now;
    /// `activation` the pending activation, only if its generation is the current one; `advance` the runtime's own
    /// deadline; `covers` the live navigation's bars (the old tree's cleared covers never reach the new runtime);
    /// `page` the window, root font, zoom and scale now; `collections` the live lists' rows; `modules` the events
    /// still queued (a held delivery never took them), once this generation's data is up; `pump` what the network
    /// owes now. Work asked while the runtime is held whose host side has effects of its own waits too, rather
    /// than run half: a fit, AppKit's insets, the data activation, module delivery, the iOS native geometry pump.
    enum Replay: Int, CaseIterable {
        case fit, activation, advance, covers, page, collections, modules, pump
    }

    /// The held calls replayed after the batch, by `Runtime` method, and why.
    static let replayed: [String: (Replay, String)] = [
        "advance(now:untilRequest:)": (.advance, "a timer due over the teardown is the new tree's: it advances once its tree is in"),
        "tick(now:frame:)": (.advance, "a frame's tick is the clock's: the new runtime's due work advances once its tree is in"),
        "frame(now:wall:)": (.advance, "a display frame is the clock's, as a tick is"),
        "covers(_:)": (.covers, "the old tree's covers clearing is moot, but the new tree's bar covers its content: they are told again"),
        "view(_:)": (.fit, "a resize the teardown set off (a keyboard it ended) is the window's: the view fits again"),
        "resize(width:height:)": (.fit, "the AppKit view's size, as `view` is the UIKit one's: it fits again"),
        "insets(top:right:bottom:left:)": (.fit, "the AppKit view's insets, told with its size: it fits again"),
        "activateData(_:token:)": (.activation, "the data source's activation is the new runtime's: it activates after, its module events with it"),
        "deliverModuleEvents()": (.modules, "module events wait for the runner that hears them: they are delivered after, in order, once"),
        "regions(_:)": (.fit, "the window's layout regions, told with its size: it fits again"),
        "titlebarArea(_:)": (.fit, "the title-bar area, told with its size: it fits again"),
        "tickAsync(now:frame:done:)": (.advance, "a frame's tick sent to the owner, as `tick` is"),
        "collectionFeedbackAsync(_:now:done:)": (.collections, "a list's fill sent to the owner: the new list fills its own"),
        "pump(now:)": (.pump, "the replies the network owes, pumped at once, as `pumpAsync`'s"),
        "pumpAsync(now:done:)": (.pump, "the replies the network owes are the new runtime's: they are pumped again (the held pump answered cancelled)"),
        "later(_:done:)": (.page, "a notice's calls, as `tellSoon`'s"),
        "setPage(_:)": (.page, "a key window or activation the teardown moved (a sheet closing) is the page's: it is told again"),
        "setRootFontSize(_:)": (.page, "the page's root font, told with its key window: told again with it"),
        "setRootFontZoom(_:)": (.page, "the page's zoom, told with its key window: told again with it"),
        "setDisplayScale(_:)": (.page, "the page's display scale, told with its key window: told again with it"),
        "tellSoon(_:)": (.page, "a notice the teardown set off (a key window moving) is the page's: it is told again, at once"),
        "collectionFeedback(_:now:)": (.collections, "a list's report is of its old rows; the new list reports its own port and rows"),
    ]

    /// Held calls dropped, by `Runtime` method, and why; any other held call is dropped too (as about the old tree).
    static let dropped: [String: String] = [
        "holdBegin(_:property:now:)": "a gesture beginning on the old tree",
        "heightDragBegin(_:targetKey:now:)": "a height drag beginning on the old tree",
        "transformMotion(_:)": "a transform drag's packet for the old tree",
        "holdUpdate(_:x:y:now:)": "a gesture moving on the old tree",
        "holdEnd(_:cancel:measured:vx:vy:now:)": "a gesture ending as its view goes",
        "pointer(_:_:_:now:)": "a pointer event on the old tree",
        "pan(_:dx:dy:now:)": "a pan on the old tree",
        "panRelease(_:vx:vy:now:)": "a pan released as its view goes",
        "scrolled(_:left:top:)": "an old scroller's offset as its view goes",
        "canvasDrawAsync(done:)": "a draw of the old tree's canvases; the new tree's draw on mount",
        "intrinsics(_:)": "the old views' sizes; the new tree's views measure and report their own",
        "focus(_:now:)": "focus a teardown gives back (a dismissed sheet's) lands on the old tree",
        "blur(_:now:)": "the old tree's field losing focus as it goes",
        "focusEvent(_:entering:target:related:now:)": "focusin and focusout across the old tree",
        "selectionChange(_:_:start:end:rects:now:)": "a selection the teardown clears is the old paragraph's",
        "selection(_:json:now:)": "a Markdown editor refocused by a teardown reports the old editor's selection",
        "change(_:_:selection:now:)": "an edit a teardown ends is the old field's; its value lives in the old tree only",
        "input(_:_:selection:now:)": "typing the teardown finishes (a composition ending) is the old field's",
        "replacementInput(_:_:_:now:)": "a scripted replacement into the old field",
        "fieldEvent(_:_:_:now:)": "a field's beforeinput, input or change as the teardown ends its edit: the old field's",
        "submit(_:now:)": "a form the teardown submits (a return key's) is the old tree's",
        "clipboard(_:_:_:now:)": "a copy or a context menu line from the old tree",
        "blocks(_:placements:shows:)": "an old sheet's block or placement as it goes; the new tree's owners publish their own",
        "checked(_:_:commit:now:)": "a toggle's state from the old tree",
        "fieldSelect(_:_:_:now:)": "a field's selection from the old tree",
        "press(_:held:now:)": "a press on a view going away",
        "hover(_:over:now:)": "a pointer leaving views going away",
        "key(_:_:up:capture:now:)": "a key down on the old tree",
        "scroll(_:metrics:now:)": "an old scroller's offset",
    ]

    /// What a test sees: per reset, the calls that reached the runtime over it (none) and those held; per replay, what
    /// ran and the calls it made.
    enum Seen: Equatable { case reset(entered: Int, held: [String]), replayed([Replay], entered: Int) }
    nonisolated(unsafe) static var spy: ((Seen) -> Void)?

    /// `held` (a runtime's `heldCalls`) as what to replay, each once, in order; the rest dropped.
    static func replays(_ held: [String]) -> [Replay] {
        Set(held.compactMap { replayed[$0]?.0 }).sorted { $0.rawValue < $1.rawValue }
    }
}

extension ExactSession {
    /// The presenter's reset with this runtime held (the one it replaces the old tree's with): nothing the
    /// teardown sets off reaches it. Its calls wait in `heldCalls` for `replayHeld`, after the committed batch.
    func resetHeld() {
        let entered = runtime.entered
        runtime.held = true
        presenter.reset()
        runtime.held = false
        ResetGate.spy?(.reset(entered: runtime.entered - entered, held: runtime.heldCalls))
    }

    /// After the committed batch: the held calls ResetGate replays, each once, in order; the rest dropped.
    @discardableResult func replayHeld() -> [ResetGate.Replay] {
        let replays = ResetGate.replays(runtime.takeHeldCalls()), entered = runtime.entered
        defer { if !replays.isEmpty { ResetGate.spy?(.replayed(replays, entered: runtime.entered - entered)) } }
        for replay in replays {
            switch replay {
            case .advance: advanceIfDue()
            case .covers:
                #if os(iOS) || os(tvOS)
                presenter.navigation.reportCovers()
                #endif
                // (AppKit's presentations report the new tree's covers as they present it: `CoversMac`.)
            case .fit:
                #if os(iOS) || os(tvOS)
                view?.fit()
                #else
                view?.needsLayout = true
                #endif
            case .page: tellPage()
            case .collections: presenter.collections.flush()
            case .pump: pumpReplies()
            case .activation: if let held = pendingActivation, held.generation == generation { pendingActivation = nil; activateData(held.generation, token: held.token) } // (this turn: its batch, then its module events)
            case .modules: if dataGeneration == generation, !replays.contains(.activation) { deliverModuleEvents() } // (else this generation's activation drains them)
            }
        }
        return replays
    }
}
