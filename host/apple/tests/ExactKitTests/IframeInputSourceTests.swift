import ObjectiveC
import WebKit
import XCTest
#if os(macOS)
import AppKit

/// A frame's web view reconnects its input context when the keyboard input
/// source changes while it has the keys (`ExactWebView`): another process
/// selecting a source can leave WebKit's context with no input method.
/// `NSTextInputContext`'s current context, selected source and activation
/// stand in for the system's: the test process is never the active app, so
/// AppKit's current context is nil.
final class IframeInputSourceTests: XCTestCase {
    private typealias EventFn = @convention(c) (UnsafeMutableRawPointer?, UInt32, UInt32, UnsafePointer<UInt8>?, UInt32) -> Void
    private typealias ReplyFn = @convention(c) (UnsafeMutableRawPointer?, UInt32, UInt32, UInt32, UnsafePointer<UInt8>?, UInt32) -> Void
    private typealias CreateFn = @convention(c) (UInt32, UnsafeMutableRawPointer?, EventFn?, ReplyFn?) -> UnsafeMutableRawPointer?
    private typealias ViewFn = @convention(c) (UnsafeMutableRawPointer?) -> UnsafeMutableRawPointer?
    private typealias SetFn = @convention(c) (UnsafeMutableRawPointer?, UnsafePointer<UInt8>?, UInt32, UInt32) -> Void
    private typealias PostFn = @convention(c) (UnsafeMutableRawPointer?, UnsafePointer<UInt8>?, UInt32) -> Void
    private typealias DestroyFn = @convention(c) (UnsafeMutableRawPointer?) -> Void

    private static let event: EventFn = { _, _, _, _, _ in }
    private static let reply: ReplyFn = { _, _, _, _, _, _ in }
    nonisolated(unsafe) private static var server: AutomationPages?

    private var window: NSWindow?
    private var restore: [() -> Void] = []
    private var source = "com.apple.keylayout.ABC"
    private var current: NSTextInputContext?
    private var activations = 0, deactivations = 0

    override func setUpWithError() throws {
        _ = NSApplication.shared
        if Self.server == nil { Self.server = try AutomationPages() }
    }

    override func tearDown() {
        restore.forEach { $0() }
        restore = []
        window?.close()
        window = nil
    }

    private func spin(until done: () -> Bool, _ seconds: TimeInterval = 10) {
        let deadline = Date(timeIntervalSinceNow: seconds)
        while !done() && Date() < deadline { RunLoop.main.run(mode: .default, before: Date(timeIntervalSinceNow: 0.02)) }
    }

    /// Replaces `name`'s implementation on `cls` (a class method on its
    /// metaclass) until the test ends.
    private func replace(_ cls: AnyClass, _ name: String, _ block: Any) {
        let method = class_getInstanceMethod(cls, NSSelectorFromString(name))!
        let original = method_setImplementation(method, imp_implementationWithBlock(block))
        restore.append { method_setImplementation(method, original) }
    }

    private func fakeInputMethod() {
        let context: AnyClass = NSTextInputContext.self
        let selected: @convention(block) (NSTextInputContext) -> String? = { [unowned self] _ in source }
        replace(context, "selectedKeyboardInputSource", selected)
        let activate: @convention(block) (NSTextInputContext) -> Void = { [unowned self] _ in activations += 1 }
        replace(context, "activate", activate)
        let deactivate: @convention(block) (NSTextInputContext) -> Void = { [unowned self] _ in deactivations += 1 }
        replace(context, "deactivate", deactivate)
        let currentContext: @convention(block) (AnyObject) -> NSTextInputContext? = { [unowned self] _ in current }
        replace(object_getClass(context)!, "currentInputContext", currentContext)
    }

    /// A partitioned frame on a page whose text field has the focus, its web
    /// view the window's first responder with an input context.
    private func mount() throws -> (WKWebView, () -> Void) {
        guard let path = ProcessInfo.processInfo.environment["EXACT_TEST_WEB_MODULE"] else {
            throw missing("module", "no WebKit arm: name a built one with EXACT_TEST_WEB_MODULE")
        }
        let library = try XCTUnwrap(dlopen(path, RTLD_NOW | RTLD_LOCAL), String(cString: dlerror()))
        func symbol<T>(_ name: String, _: T.Type) throws -> T { unsafeBitCast(try XCTUnwrap(dlsym(library, name), "missing \(name)"), to: T.self) }
        let create = try symbol("exact_web_create", CreateFn.self), view = try symbol("exact_web_platform_view", ViewFn.self)
        let setSrc = try symbol("exact_web_set_src", SetFn.self), configure = try symbol("exact_web_configure", PostFn.self)
        let destroy = try symbol("exact_web_destroy", DestroyFn.self)
        let handle = try XCTUnwrap(create(12, nil, Self.event, Self.reply))
        let options = try JSONSerialization.data(withJSONObject: ["element": "field", "partition": "memory-input-source"])
        options.withUnsafeBytes { configure(handle, $0.bindMemory(to: UInt8.self).baseAddress, UInt32(options.count)) }
        let web = Unmanaged<AnyObject>.fromOpaque(view(handle)!).takeUnretainedValue() as! WKWebView
        web.frame = CGRect(x: 0, y: 0, width: 400, height: 300)
        let window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 400, height: 300), styleMask: [.titled], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        window.contentView!.addSubview(web)
        window.makeKeyAndOrderFront(nil)
        self.window = window
        Array("http://127.0.0.1:\(Self.server!.port)/input".utf8).withUnsafeBufferPointer { setSrc(handle, $0.baseAddress, UInt32($0.count), 1) }
        var focused = false
        spin(until: {
            web.evaluateJavaScript("field.focus(); document.activeElement === field") { value, _ in focused = value as? Bool == true }
            return focused
        })
        window.makeFirstResponder(web)
        spin { web.inputContext != nil }
        XCTAssert(window.firstResponder === web)
        try XCTUnwrap(web.inputContext, "the focused field gave the web view no input context")
        return (web, { destroy(handle) })
    }

    private func select(_ name: String) {
        source = name
        NotificationCenter.default.post(name: NSTextInputContext.keyboardSelectionDidChangeNotification, object: nil)
    }

    /// Each change of source while the view has the keys and its context is
    /// the current one reconnects the context once; the same source again
    /// does not.
    func testAChangeOfSourceReconnectsTheContext() throws {
        let (web, unmount) = try mount()
        defer { unmount() }
        fakeInputMethod()
        current = web.inputContext
        window!.makeFirstResponder(nil)
        window!.makeFirstResponder(web)
        select("com.apple.inputmethod.Korean.2SetKorean")
        XCTAssertEqual(activations, 1)
        XCTAssertEqual(deactivations, 1)
        select("com.apple.inputmethod.Korean.2SetKorean")
        XCTAssertEqual(activations, 1, "the same source again reconnected the context")
        select("com.apple.keylayout.ABC")
        XCTAssertEqual(activations, 2)
        // A change while the page has no editable element (no context) leaves
        // the source unknown, so the next change with a context reconnects.
        web.evaluateJavaScript("field.blur()") { _, _ in }
        spin { web.inputContext == nil }
        XCTAssertNil(web.inputContext, "a blurred field left the web view an input context")
        select("com.apple.inputmethod.Korean.2SetKorean")
        web.evaluateJavaScript("field.focus()") { _, _ in }
        spin { web.inputContext != nil }
        current = web.inputContext
        select("com.apple.keylayout.ABC")
        XCTAssertEqual(activations, 3, "a change after the field was focused again did not reconnect")
    }

    /// A change while the page holds marked text waits: deactivating the
    /// context then may commit or drop it. The next notice reconnects.
    func testAChangeWhileTextIsMarkedWaits() throws {
        let (web, unmount) = try mount()
        defer { unmount() }
        fakeInputMethod()
        var marked = true
        let markedText: @convention(block) (AnyObject) -> Bool = { _ in marked }
        try XCTUnwrap(class_getInstanceMethod(WKWebView.self, NSSelectorFromString("hasMarkedText")), "WKWebView answers no hasMarkedText")
        replace(WKWebView.self, "hasMarkedText", markedText)
        current = web.inputContext
        window!.makeFirstResponder(nil)
        window!.makeFirstResponder(web)
        select("com.apple.inputmethod.Korean.2SetKorean")
        XCTAssertEqual(activations, 0, "a context holding marked text was reconnected")
        marked = false
        NotificationCenter.default.post(name: NSTextInputContext.keyboardSelectionDidChangeNotification, object: nil)
        XCTAssertEqual(activations, 1, "the change skipped for marked text was not reconnected at the next notice")
    }

    /// A change while another view has the keys, or while the view's context
    /// is not the current one, leaves the context alone: activating it would
    /// take the input method from the view that has it.
    func testAChangeAwayFromTheViewLeavesItsContext() throws {
        let (web, unmount) = try mount()
        defer { unmount() }
        fakeInputMethod()
        current = web.inputContext
        window!.makeFirstResponder(web)
        window!.makeFirstResponder(nil)
        select("com.apple.inputmethod.Korean.2SetKorean")
        XCTAssertEqual(activations, 0, "a view without the keys reconnected its context")
        current = nil
        window!.makeFirstResponder(web)
        select("com.apple.keylayout.ABC")
        XCTAssertEqual(activations, 0, "a context that was not the current one was reconnected")
    }
}
#endif
