// The platform's inspector for a frame's guest (@ref LLP 1020.002 D11),
// Electron's `<webview>.openDevTools()`. On macOS it is Safari's Web
// Inspector in a window of its own. WebKit has no public call that shows it,
// so this uses the private `-[WKWebView _inspector]` (`WKWebViewPrivate.h`)
// and `_WKInspector`'s `show` and `detach` (`_WKInspector.h`), as Tauri's
// wry does, with `-[WKPreferences _setDeveloperExtrasEnabled:]`
// (`WKPreferencesPrivate.h`), without which the inspector never connects
// (measured on macOS 27.2). A WebKit without them is refused by name.
import Foundation
import WebKit

extension WebArm {
    /// `{op: "openDevTools"}`: opens the inspector detached, as Electron's
    /// `mode: "detach"`, or brings an open one to the front, docked or not.
    /// The frame becomes inspectable from Safari › Develop too, and its
    /// context menu gains Inspect Element, in any build: the app asked for
    /// this frame's inspector.
    func openDevTools() {
        let refuse = { (why: String) in FileHandle.standardError.write(Data("exact: iframe \(self.id): openDevTools: \(why)\n".utf8)) }
        guard pdf == nil, let web = built else { return refuse("the frame shows no web page") }
        web.isInspectable = true
        #if os(macOS)
        let named = NSSelectorFromString, preferences = web.configuration.preferences
        guard preferences.responds(to: named("_setDeveloperExtrasEnabled:")), web.responds(to: named("_inspector")),
              let inspector = web.perform(named("_inspector"))?.takeUnretainedValue() as? NSObject,
              ["show", "detach", "isVisible", "inspectorWebView"].allSatisfy({ inspector.responds(to: named($0)) }) else {
            return refuse("this WebKit has no -[WKWebView _inspector]; inspect the frame from Safari › Develop")
        }
        typealias SetFlag = @convention(c) (AnyObject, Selector, Bool) -> Void
        typealias Flag = @convention(c) (AnyObject, Selector) -> Bool
        unsafeBitCast(preferences.method(for: named("_setDeveloperExtrasEnabled:")), to: SetFlag.self)(preferences, named("_setDeveloperExtrasEnabled:"), true)
        let visible = unsafeBitCast(inspector.method(for: named("isVisible")), to: Flag.self)(inspector, named("isVisible"))
        inspector.perform(named("show"))
        guard !visible else { return }
        // `show` loads the inspector's page asynchronously, then opens it
        // docked in the frame's window where the frame is large enough; a
        // `detach` before then is overridden. So the arm waits until the
        // inspector's view is in a window (at most 5 s) and detaches it when
        // that window is the frame's.
        let shown = { (inspector.perform(named("inspectorWebView"))?.takeUnretainedValue() as? NSView)?.window }
        func detachWhenOpen(_ tries: Int) {
            if let window = shown() {
                if window === web.window { inspector.perform(named("detach")) }
                return
            }
            guard tries > 0 else { return refuse("the inspector did not open within 5 s") }
            DispatchQueue.main.asyncAfter(deadline: .now() + 0.05) { detachWhenOpen(tries - 1) }
        }
        detachWhenOpen(100)
        #else
        refuse("iOS shows no inspector in the app; the frame is inspectable from a Mac's Safari › Develop")
        #endif
    }
}
