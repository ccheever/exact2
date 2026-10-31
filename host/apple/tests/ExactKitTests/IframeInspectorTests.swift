import PDFKit
import WebKit
import XCTest
#if os(macOS)
import AppKit

/// `openDevTools(id)` through the web arm's C ABI (@ref LLP 1020.002 D11):
/// Safari's Web Inspector for the frame's guest opens detached, in a window
/// of its own, and a second call brings the same one forward. A frame with
/// no web page, or a WebKit without the inspector's selectors, is refused in
/// the log and opens nothing.
final class IframeInspectorTests: XCTestCase {
    private typealias EventFn = @convention(c) (UnsafeMutableRawPointer?, UInt32, UInt32, UnsafePointer<UInt8>?, UInt32) -> Void
    private typealias ReplyFn = @convention(c) (UnsafeMutableRawPointer?, UInt32, UInt32, UInt32, UnsafePointer<UInt8>?, UInt32) -> Void
    private typealias CreateFn = @convention(c) (UInt32, UnsafeMutableRawPointer?, EventFn?, ReplyFn?) -> UnsafeMutableRawPointer?
    private typealias ViewFn = @convention(c) (UnsafeMutableRawPointer?) -> UnsafeMutableRawPointer?
    private typealias SetFn = @convention(c) (UnsafeMutableRawPointer?, UnsafePointer<UInt8>?, UInt32, UInt32) -> Void
    private typealias PostFn = @convention(c) (UnsafeMutableRawPointer?, UnsafePointer<UInt8>?, UInt32) -> Void
    private typealias DestroyFn = @convention(c) (UnsafeMutableRawPointer?) -> Void
    private typealias Flag = @convention(c) (AnyObject, Selector) -> Bool

    private struct Arm {
        let create: CreateFn, view: ViewFn, setSrc: SetFn, setDocument: SetFn, configure: PostFn, command: PostFn, destroy: DestroyFn
    }

    private static let event: EventFn = { _, _, _, _, _ in }
    private static let reply: ReplyFn = { _, _, _, _, _, _ in }
    private var window: NSWindow?

    override func tearDown() { window?.close(); window = nil }

    private func arm() throws -> Arm {
        guard let path = ProcessInfo.processInfo.environment["EXACT_TEST_WEB_MODULE"] else {
            throw missing("module", "no WebKit arm: name a built one with EXACT_TEST_WEB_MODULE")
        }
        let library = try XCTUnwrap(dlopen(path, RTLD_NOW | RTLD_LOCAL), String(cString: dlerror()))
        func symbol<T>(_ name: String, _: T.Type) throws -> T { unsafeBitCast(try XCTUnwrap(dlsym(library, name), "missing \(name)"), to: T.self) }
        return Arm(create: try symbol("exact_web_create", CreateFn.self), view: try symbol("exact_web_platform_view", ViewFn.self),
                   setSrc: try symbol("exact_web_set_src", SetFn.self), setDocument: try symbol("exact_web_set_document", SetFn.self),
                   configure: try symbol("exact_web_configure", PostFn.self), command: try symbol("exact_web_command", PostFn.self),
                   destroy: try symbol("exact_web_destroy", DestroyFn.self))
    }

    private func spin(until done: () -> Bool, _ seconds: TimeInterval = 15) {
        let deadline = Date(timeIntervalSinceNow: seconds)
        while !done() && Date() < deadline { RunLoop.main.run(mode: .default, before: Date(timeIntervalSinceNow: 0.02)) }
    }

    /// Waits until the app's window count holds for 15 turns of the run loop
    /// (about 300 ms): that a call opened no window shows only as none coming.
    private func settledWindowCount() -> Int {
        var count = NSApplication.shared.windows.count, still = 0
        spin(until: {
            let now = NSApplication.shared.windows.count
            still = now == count ? still + 1 : 0
            count = now
            return still >= 15
        }, 5)
        return count
    }

    private func post(_ value: [String: Any], _ fn: PostFn, _ handle: UnsafeMutableRawPointer) {
        let data = try! JSONSerialization.data(withJSONObject: value)
        data.withUnsafeBytes { fn(handle, $0.bindMemory(to: UInt8.self).baseAddress, UInt32(data.count)) }
    }

    /// What the arm writes to standard error while `body` runs.
    private func standardError(during body: () -> Void) -> String {
        let pipe = Pipe(), saved = dup(STDERR_FILENO)
        fflush(stderr)
        dup2(pipe.fileHandleForWriting.fileDescriptor, STDERR_FILENO)
        body()
        fflush(stderr)
        dup2(saved, STDERR_FILENO)
        close(saved)
        try? pipe.fileHandleForWriting.close()
        return String(decoding: pipe.fileHandleForReading.readDataToEndOfFile(), as: UTF8.self)
    }

    /// A frame in a window large enough that WebKit would dock the inspector
    /// in it, as it does in an app's window.
    private func mount(_ arm: Arm, configuration: [String: Any], src: String, document: Data? = nil) -> UnsafeMutableRawPointer {
        let handle = arm.create(41, nil, Self.event, Self.reply)!
        post(configuration, arm.configure, handle)
        let view = Unmanaged<AnyObject>.fromOpaque(arm.view(handle)!).takeUnretainedValue() as! NSView
        view.frame = CGRect(x: 0, y: 0, width: 1200, height: 800)
        let window = NSWindow(contentRect: view.frame, styleMask: [.titled], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        window.contentView!.addSubview(view)
        window.orderFrontRegardless()
        self.window = window
        document?.withUnsafeBytes { arm.setDocument(handle, $0.bindMemory(to: UInt8.self).baseAddress, UInt32($0.count), 1) }
        Array(src.utf8).withUnsafeBufferPointer { arm.setSrc(handle, $0.baseAddress, UInt32($0.count), 1) }
        return handle
    }

    private func web(_ arm: Arm, _ handle: UnsafeMutableRawPointer) -> WKWebView? {
        Unmanaged<AnyObject>.fromOpaque(arm.view(handle)!).takeUnretainedValue() as? WKWebView
    }

    func testOpenDevToolsShowsTheGuestsInspectorDetachedAndOnce() throws {
        let arm = try arm()
        let handle = mount(arm, configuration: ["partition": "inspector-test"], src: "about:blank")
        defer { arm.destroy(handle) }
        let web = try XCTUnwrap(web(arm, handle))
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
        post(["op": "openDevTools"], arm.command, handle)
        spin { visible() && inspectorWindow().map { $0 !== self.window } == true }
        XCTAssertTrue(web.isInspectable)
        XCTAssertTrue(visible())
        let first = try XCTUnwrap(inspectorWindow(), "the inspector has no window")
        XCTAssertFalse(first === window, "the inspector is docked in the frame's window, not detached")

        // Open already: the same window again, none added.
        let windows = settledWindowCount()
        post(["op": "openDevTools"], arm.command, handle)
        XCTAssertEqual(settledWindowCount(), windows)
        XCTAssertTrue(visible())
        XCTAssertTrue(inspectorWindow() === first)
    }

    /// A WebKit whose `_inspector` answers nothing, as one without it would:
    /// the refusal names Safari › Develop, and no inspector opens.
    func testAWebKitWithoutTheInspectorIsRefusedInTheLog() throws {
        let arm = try arm()
        let handle = mount(arm, configuration: ["partition": "inspector-refused"], src: "about:blank")
        defer { arm.destroy(handle) }
        let web = try XCTUnwrap(web(arm, handle))
        spin { !web.isLoading && web.url != nil }
        let method = try XCTUnwrap(class_getInstanceMethod(WKWebView.self, NSSelectorFromString("_inspector")), "this WebKit has no -[WKWebView _inspector]")
        let windows = settledWindowCount()

        let original = method_setImplementation(method, imp_implementationWithBlock({ (_: AnyObject) -> AnyObject? in nil } as @convention(block) (AnyObject) -> AnyObject?))
        let said = standardError { post(["op": "openDevTools"], arm.command, handle) }
        method_setImplementation(method, original)

        XCTAssertTrue(said.contains("openDevTools: this WebKit has no -[WKWebView _inspector]; inspect the frame from Safari › Develop"), said)
        let inspector = try XCTUnwrap(web.perform(NSSelectorFromString("_inspector"))?.takeUnretainedValue() as? NSObject)
        let isVisible = NSSelectorFromString("isVisible")
        XCTAssertFalse(unsafeBitCast(inspector.method(for: isVisible), to: Flag.self)(inspector, isVisible))
        XCTAssertEqual(settledWindowCount(), windows)
    }

    /// A local PDF is PDFKit's view, not a web page (D10): nothing to inspect.
    func testAPDFFrameIsRefusedInTheLog() throws {
        let arm = try arm()
        let handle = mount(arm, configuration: ["pdf": true], src: "app:/tmp/doc.pdf", document: Self.document())
        defer { arm.destroy(handle) }
        spin { Unmanaged<AnyObject>.fromOpaque(arm.view(handle)!).takeUnretainedValue() is PDFView }
        let windows = settledWindowCount()

        let said = standardError { post(["op": "openDevTools"], arm.command, handle) }
        XCTAssertTrue(said.contains("openDevTools: the frame shows no web page"), said)
        XCTAssertEqual(settledWindowCount(), windows)
    }

    private static func document() -> Data {
        let data = NSMutableData()
        var box = CGRect(x: 0, y: 0, width: 612, height: 792)
        let context = CGContext(consumer: CGDataConsumer(data: data as CFMutableData)!, mediaBox: &box, nil)!
        context.beginPDFPage(nil)
        context.endPDFPage()
        context.closePDF()
        return data as Data
    }
}
#endif
