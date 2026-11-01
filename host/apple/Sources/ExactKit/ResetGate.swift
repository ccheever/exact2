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
        "frame(now:wall:admitted:)": (.advance, "a display frame is the clock's, as a tick is"),
        "covers(_:)": (.covers, "the old tree's covers clearing is moot, but the new tree's bar covers its content: they are told again"),
        "view(_:)": (.fit, "a resize the teardown set off (a keyboard it ended) is the window's: the view fits again"),
        "resize(width:height:)": (.fit, "the AppKit view's size, as `view` is the UIKit one's: it fits again"),
        "insets(top:right:bottom:left:)": (.fit, "the AppKit view's insets, told with its size: it fits again"),
        "activateData(_:token:)": (.activation, "the data source's activation is the new runtime's: it activates after, its module events with it"),
        "deliverModuleEvents()": (.modules, "module events wait for the runner that hears them: they are delivered after, in order, once"),
        "regions(_:)": (.fit, "the window's layout regions, told with its size: it fits again"),
        "titlebarArea(_:)": (.fit, "the title-bar area, told with its size: it fits again"),
        "tickAsync(now:frame:capture:measureFrame:done:)": (.advance, "a frame's tick sent to the owner, as `tick` is"),
        "advanceAsync(now:frame:admitted:capture:measureFrame:done:)": (.advance, "a timer's advance sent to the owner, as `advance` is"),
        "landThenFirst()": (.advance, "the `then`s due when a batch landed: the new runtime's due work advances once its tree is in"),
        "landThen()": (.advance, "the agent's `then`s of an input, as `landThenFirst`'s"),
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
        "input(_:_:selection:now:)": "typing the teardown finishes (a composition ending) is the old field's",
        "replacementInput(_:_:_:now:)": "a scripted replacement into the old field",
        "fieldEvent(_:_:_:now:)": "a field's beforeinput, input or change as the teardown ends its edit: the old field's",
        "submit(_:now:)": "a form the teardown submits (a return key's) is the old tree's",
        "clipboard(_:_:_:now:)": "a copy or a context menu line from the old tree",
        "blocks(_:placements:shows:)": "an old sheet's block or placement as it goes; the new tree's owners publish their own",
        "checked(_:_:commit:now:)": "a toggle's state from the old tree",
        "fieldSelect(_:_:_:now:)": "a field's selection from the old tree",
        "press(_:held:at:now:)": "a press on a view going away",
        "hover(_:over:now:)": "a pointer leaving views going away",
        "key(_:_:up:capture:now:)": "a key down on the old tree",
        "scroll(_:metrics:now:)": "an old scroller's offset",
        // #1144's timer calls: each recovers by itself, so a replay would fire it twice.
        "fireNativeTimer(_:now:)": "a task's native firing answered `held` stays at the front of its queue, which asks for it again a frame later (NativeTaskTimers `complete(retry:)`); a new generation's rows re-arm the task at its own deadline",
        "fireNativeTimerAsync(_:now:done:)": "a task's native firing sent to the owner, as `fireNativeTimer`'s: kept by its queue, asked for again",
        "timerWaits(kinds:cutoff:mode:)": "a timer admission check answered `held` leaves nothing pending (TimerAdmissions); the next frame or update checks again, against the new runtime's waits",
        "timerWaitsAsync(kinds:cutoff:done:)": "an admission check sent to the owner, as `timerWaits`'s: nothing left pending, the next check admits",
        // Events on the old tree's views: as the gestures and inputs above.
        "contextmenu(_:now:)": "a context menu asked of the old tree",
        "dblclick(_:now:)": "a double click on the old tree",
        "swiperight(_:now:)": "a swipe on the old tree",
        "scrollend(_:now:)": "an old scroller's end",
        "refresh(_:now:)": "a pull to refresh on the old tree's list",
        "intoView(_:key:block:inline:)": "the agent's scroll into an old list",
        "heightDragUpdate(_:height:now:)": "a height drag moving on the old tree",
        "heightDragRelease(_:height:now:)": "a height drag released as its view goes",
        "reorderBegin(_:scrollTop:now:)": "a reorder beginning on the old tree",
        "reorderMove(_:dy:scrollTop:inside:now:)": "a reorder moving on the old tree",
        "reorderEnd(_:drop:dy:scrollTop:inside:velocity:now:)": "a reorder ending as its list goes",
        "groupBegin(_:scrollTop:ghost:now:)": "a grouped reorder beginning on the old tree",
        "moveInto(_:target:contentY:scrollTop:inside:now:)": "a grouped reorder moving on the old tree",
        "groupStep(_:step:now:)": "a grouped reorder's keyboard step on the old tree",
        "groupEnd(_:drop:now:)": "a grouped reorder ending as its list goes",
        "groupFinish(_:now:)": "a grouped reorder finishing as its list goes",
        "media(_:event:payload:now:)": "an old media view's event as it goes",
        "load(_:now:)": "an old image's or frame's load",
        "frameNavigation(_:event:record:now:)": "an old web frame's navigation",
        "message(_:_:now:)": "an old web frame's message",
        "dialogClose(_:now:)": "an old dialog closing as its view goes",
        "pickerCancel(_:now:)": "an old picker dismissed as its view goes",
        "hostBack(_:now:)": "a back gesture the teardown set off is the old navigation's",
        "hostBack(from:to:now:)": "a back the teardown set off is the old navigation's",
        "beforeunload(_:now:)": "the old page's unload as it goes",
        "surfaceRecord(_:_:)": "an old canvas publisher released as the teardown ends it: the new runner starts with no surface records (none is carried), and the new tree's publishers record their own",
        "regionComplete(_:)": "an old generation's region artifact: `RegionController` drops one from another generation before it is told, and the new tree asks its own",
        "fulfillSurface(_:kind:body:now:)": "an old-tree surface completing as its view goes: `completeSurface` drops a completion from another generation before it is told",
        // Told again before the reset returns (Session `presentCommitted`, `applyTogether`): moot.
        "setPreferences(_:)": "the person's preferences: `tellTime` tells the new runtime them again (`tellPreferences`)",
        "reportColors(tint:)": "the window's tint: `treeIsIn` flushes `colorsOwed`, and `tellPreferences` reports it again",
        "scheme(dark:)": "the window's colour scheme: the view's `rebooted` tells the new runtime it again",
        "viewScheme(_:dark:)": "a view's colour scheme: told again with the window's (`rebooted`)",
        "segments(_:)": "the posture and viewport segments: the view's `rebooted` tells them again",
        "listMargins(_:)": "UIKit's cell margins: the presenter's reset clears what was told (`RowCells.reset`), and the new rows report their own",
        "groupedMargins(_:)": "UIKit's grouped-card margins: the new tree's grouped lists measure and report their own",
        "resizeBox(_:width:height:)": "a resize grip's drag on the old tree: a restart takes the size back to the plan's, as a reload does the browser's",
        // Never reached while held: their entry points wait for the replay first.
        "dataReady()": "never held: `activateData` holds first and is replayed (.activation), which asks it",
        "landShare()": "never held: a cold share lands from `activateData`, which holds first and is replayed",
        "moduleEvent(_:)": "never held: `deliverModuleEvents` holds first, keeps the events and is replayed (.modules); the session's end journals its own",
        // The session's own steps, which run outside the hold: never set off by a teardown.
        "boot(width:height:)": "the session's boot, outside any hold",
        "bootPlan(_:width:height:)": "the session's boot, outside any hold",
        "bootEntry(_:kind:props:width:height:)": "a widget entry's boot, outside any hold",
        "preparePlan(_:width:height:token:)": "a restart's own preparation, before its reset",
        "prepareModule(_:module:token:width:height:letGo:)": "a restart's own preparation, before its reset",
        "commitPlan()": "a restart's own commit, before its reset",
        "startOnFrame(_:at:)": "a setting the session gives at boot, outside the hold",
        "setTime(epochAtZero:utcOffset:)": "the clock's epoch, told at boot and by the agent, outside the hold",
        "setPlace(locale:timeZone:seed:formatLocale:)": "the place, told at boot, by the agent and when the Region or the hour cycle changes, outside the hold",
        "setLaunchPlace(locale:timeZone:seed:formatLocale:)": "the launch place, told at boot, outside the hold",
    ]

    /// Held calls dropped though what they carry is lost: an arrival from outside the tree whose content the new
    /// runtime never hears until its source says it again. Kept apart from the drops above, which are moot over a
    /// reset, and none may be added (host/apple tests/it/reset_gate.rs). Triaged 2026-10-29: most come from the main
    /// queue, which the hold never drains (the teardown is synchronous and turns the run loop in none of the common
    /// modes, even ending an open sheet or restarting several windows together: ResetHoldTests), so they can't reach a
    /// held runtime. They stay
    /// listed so the scan stays honest, and become real losses if ResetHoldTests ever fails.
    static let knownLoss: [String: String] = [
        "storeChanged(retryOnly:)": "a peer's persisted write: the window shows the old value until the store changes again",
        "canvasImage(_:width:height:ok:)": "a Canvas 2D image decoded (or not): the canvas never hears its size",
        "canvasDisplay(scale:memory:)": "the display's scale and memory for Canvas 2D: the runner keeps the old ones",
        "textReady(index:generation:revision:)": "a text's asynchronous shaping finished: its measure stays provisional",
        "activities(_:)": "an activity snapshot or a send's completion from the arm: not heard",
        "midi(_:)": "a MIDI message or port change: not heard",
        "preferExtension(_:)": "an extension preference: not heard",
        "designRows(_:)": "a live design frame: the old design shows until the next",
        "setModes(_:check:)": "mode pins: not applied",
        "navigate(_:_:now:)": "a URL opened from outside (`openIncoming`), or a navigation from the old tree: the URL is not followed",
        "hostGo(_:now:)": "a forward the platform delivers, or the old navigation's: not taken",
        "share(_:now:)": "an incoming share: its inbox is deleted after delivery (`ShareReceive.claimInbox`), so it is gone",
        "picked(_:_:now:)": "a picker's chosen files, copied before the pick is told: told once, not again",
        "change(_:_:selection:now:)": "an edit the teardown ends (the Mac commits it as its field goes) is the old field's, moot; but a document's delivered file paths ride it too (`ExactDocuments.deliver`), and are lost",
        "deliverySync()": "the delivery facts after an app-level event: `applyTogether` asks again after its resets, but a dev-menu fresh boot or a single session's `apply` does not",
        "controlTextChanged()": "the platform's control text (Dynamic Type, appearance): the measurement caches took the new traits, so no later call tells the runtime",
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
