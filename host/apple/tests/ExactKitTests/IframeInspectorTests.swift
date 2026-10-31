import WebKit
import XCTest
#if os(macOS)
import AppKit

/// `openDevTools(id)` through the web arm's C ABI (@ref LLP 1020.002 D11):
/// Safari's Web Inspector for the frame's guest opens detached, in a window
/// of its own, and a second call brings the same one forward.
final class IframeInspectorTests: XCTestCase {
    private typealias EventFn = @convention(c) (UnsafeMutableRawPointer?, UInt32, UInt32, UnsafePointer<UInt8>?, UInt32) -> Void
    private typealias ReplyFn = @convention(c) (UnsafeMutableRawPointer?, UInt32, UInt32, UInt32, UnsafePointer<UInt8>?, UInt32) -> Void
    private typealias CreateFn = @convention(c) (UInt32, UnsafeMutableRawPointer?, EventFn?, ReplyFn?) -> UnsafeMutableRawPointer?
    private typealias ViewFn = @convention(c) (UnsafeMutableRawPointer?) -> UnsafeMutableRawPointer?
    private typealias SetFn = @convention(c) (UnsafeMutableRawPointer?, UnsafePointer<UInt8>?, UInt32, UInt32) -> Void
    private typealias PostFn = @convention(c) (UnsafeMutableRawPointer?, UnsafePointer<UInt8>?, UInt32) -> Void
    private typealias DestroyFn = @convention(c) (UnsafeMutableRawPointer?) -> Void
    private typealias Flag = @convention(c) (AnyObject, Selector) -> Bool

    private static let event: EventFn = { _, _, _, _, _ in }
    private static let reply: ReplyFn = { _, _, _, _, _, _ in }
    private var window: NSWindow?

    override func tearDown() { window?.close(); window = nil }

    private func spin(until done: () -> Bool, _ seconds: TimeInterval = 15) {
        let deadline = Date(timeIntervalSinceNow: seconds)
        while !done() && Date() < deadline { RunLoop.main.run(mode: .default, before: Date(timeIntervalSinceNow: 0.02)) }
    }

    private func post(_ value: [String: Any], _ fn: PostFn, _ handle: UnsafeMutableRawPointer) {
        let data = try! JSONSerialization.data(withJSONObject: value)
        data.withUnsafeBytes { fn(handle, $0.bindMemory(to: UInt8.self).baseAddress, UInt32(data.count)) }
    }

    func testOpenDevToolsShowsTheGuestsInspectorDetachedAndOnce() throws {
        guard let path = ProcessInfo.processInfo.environment["EXACT_TEST_WEB_MODULE"] else {
            throw missing("module", "no WebKit arm: name a built one with EXACT_TEST_WEB_MODULE")
        }
        let library = try XCTUnwrap(dlopen(path, RTLD_NOW | RTLD_LOCAL), String(cString: dlerror()))
        func symbol<T>(_ name: String, _: T.Type) throws -> T { unsafeBitCast(try XCTUnwrap(dlsym(library, name), "missing \(name)"), to: T.self) }
        let create = try symbol("exact_web_create", CreateFn.self), view = try symbol("exact_web_platform_view", ViewFn.self)
        let setSrc = try symbol("exact_web_set_src", SetFn.self), configure = try symbol("exact_web_configure", PostFn.self)
        let command = try symbol("exact_web_command", PostFn.self), destroy = try symbol("exact_web_destroy", DestroyFn.self)

        let handle = try XCTUnwrap(create(41, nil, Self.event, Self.reply))
        defer { destroy(handle) }
        post(["partition": "inspector-test"], configure, handle)
        let web = try XCTUnwrap(Unmanaged<AnyObject>.fromOpaque(view(handle)!).takeUnretainedValue() as? WKWebView)
        web.frame = CGRect(x: 0, y: 0, width: 400, height: 300)
        let window = NSWindow(contentRect: web.frame, styleMask: [.titled], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        window.contentView!.addSubview(web)
        window.orderFrontRegardless()
        self.window = window
        Array("about:blank".utf8).withUnsafeBufferPointer { setSrc(handle, $0.baseAddress, UInt32($0.count), 1) }
        spin { !web.isLoading && web.url != nil }

        guard web.responds(to: NSSelectorFromString("_inspector")),
              let inspector = web.perform(NSSelectorFromString("_inspector"))?.takeUnretainedValue() as? NSObject else {
            throw missing("WebKit", "this WebKit has no -[WKWebView _inspector]")
        }
        let isVisible = NSSelectorFromString("isVisible")
        let visible = { unsafeBitCast(inspector.method(for: isVisible), to: Flag.self)(inspector, isVisible) }
        let inspectorWindow = { (inspector.perform(NSSelectorFromString("inspectorWebView"))?.takeUnretainedValue() as? NSView)?.window }
        defer { inspector.perform(NSSelectorFromString("close")) }

        XCTAssertFalse(visible())
        post(["op": "openDevTools"], command, handle)
        spin { visible() && inspectorWindow() != nil }
        XCTAssertTrue(web.isInspectable)
        XCTAssertTrue(visible())
        let first = try XCTUnwrap(inspectorWindow(), "the inspector has no window")
        XCTAssertFalse(first === window, "the inspector is docked in the frame's window, not detached")

        // Open already: the same window again, none added.
        let windows = NSApplication.shared.windows.count
        post(["op": "openDevTools"], command, handle)
        spin(until: { false }, 0.5)
        XCTAssertTrue(visible())
        XCTAssertTrue(inspectorWindow() === first)
        XCTAssertEqual(NSApplication.shared.windows.count, windows)
    }
}
#endif
