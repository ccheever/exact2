// DOM's `beforeunload` on the Mac (studio diary R17): the app that owns a
// window asks its session before the window closes or the app quits.
// Every mounted node that hears `beforeunload` does, as every listener on
// the web's window does; one that calls `preventDefault()` keeps the window
// open, and the app asks its own question ("Save changes?") and closes with
// `close()` once it is answered. The web's prevented `beforeunload` is the
// browser's own "Leave site?"; a Mac app has no such generic sheet, so the
// app's is the one shown.
#if os(macOS)
import AppKit

extension ExactSession {
    /// Whether the window may close: `false` when a `beforeunload` handler
    /// called `preventDefault()`. A session not yet booted, or destroyed,
    /// has nothing to keep.
    public func beforeUnload() -> Bool {
        guard state != .destroyed, booted else { return true }
        let listeners = presenter.views.values.filter { $0.handlers.contains("beforeunload") }.map(\.id).sorted()
        var prevented = false
        for id in listeners where presenter.views[id] != nil {
            presenter.defaultPrevented = false
            apply(runtime.beforeunload(id, now: now()))
            prevented = prevented || presenter.defaultPrevented
        }
        presenter.defaultPrevented = false
        if prevented { log("beforeunload: kept open") }
        return !prevented
    }
}
#endif
