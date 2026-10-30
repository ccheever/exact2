import Network
import PDFKit
import WebKit
import XCTest
#if os(macOS)
import AppKit
#else
import UIKit
#endif

#if os(macOS) || os(iOS)
/// A frame's pictures and a local PDF's view through the web arm's C ABI
/// (LLP 1020.002 stage 3): `captureScreenshot` of the whole viewport and of
/// a clip after a pinch and a pan (D7), and an `app:/` PDF in PDFKit's
/// `PDFView` (D10), against pages a loopback server in the test serves.
final class IframeCaptureTests: XCTestCase {
    private typealias EventFn = @convention(c) (UnsafeMutableRawPointer?, UInt32, UInt32, UnsafePointer<UInt8>?, UInt32) -> Void
    private typealias ReplyFn = @convention(c) (UnsafeMutableRawPointer?, UInt32, UInt32, UInt32, UnsafePointer<UInt8>?, UInt32) -> Void
    private typealias CreateFn = @convention(c) (UInt32, UnsafeMutableRawPointer?, EventFn?, ReplyFn?) -> UnsafeMutableRawPointer?
    private typealias ViewFn = @convention(c) (UnsafeMutableRawPointer?) -> UnsafeMutableRawPointer?
    private typealias SetFn = @convention(c) (UnsafeMutableRawPointer?, UnsafePointer<UInt8>?, UInt32, UInt32) -> Void
    private typealias PostFn = @convention(c) (UnsafeMutableRawPointer?, UnsafePointer<UInt8>?, UInt32) -> Void
    private typealias DestroyFn = @convention(c) (UnsafeMutableRawPointer?) -> Void

    /// What one frame reported: its records (kinds 4 and 5), loads (kind 0)
    /// and the platform views it handed over (kind 7).
    private final class Seen {
        var records: [(event: String, record: [String: Any])] = []
        var loads = 0
        var views: [[String: Any]] = []
    }

    private struct Arm {
        let create: CreateFn, view: ViewFn, setSrc: SetFn, setSandbox: SetFn, setDocument: SetFn, configure: PostFn, command: PostFn
        let destroy: DestroyFn, checkOwned: DestroyFn
    }

    private func arm() throws -> Arm {
        guard let path = ProcessInfo.processInfo.environment["EXACT_TEST_WEB_MODULE"] else {
            throw missing("module", "no WebKit arm: name a built one with EXACT_TEST_WEB_MODULE")
        }
        let library = try XCTUnwrap(dlopen(path, RTLD_NOW | RTLD_LOCAL), String(cString: dlerror()))
        func symbol<T>(_ name: String, _: T.Type) throws -> T { unsafeBitCast(try XCTUnwrap(dlsym(library, name), "missing \(name)"), to: T.self) }
        return Arm(create: try symbol("exact_web_create", CreateFn.self), view: try symbol("exact_web_platform_view", ViewFn.self),
                   setSrc: try symbol("exact_web_set_src", SetFn.self), setSandbox: try symbol("exact_web_set_sandbox", SetFn.self),
                   setDocument: try symbol("exact_web_set_document", SetFn.self), configure: try symbol("exact_web_configure", PostFn.self),
                   command: try symbol("exact_web_command", PostFn.self), destroy: try symbol("exact_web_destroy", DestroyFn.self),
                   checkOwned: try symbol("exact_web_check_owned", DestroyFn.self))
    }

    private static let event: EventFn = { context, _, kind, bytes, length in
        let seen = Unmanaged<Seen>.fromOpaque(context!).takeUnretainedValue()
        let data = Data(bytes: bytes ?? UnsafePointer(bitPattern: 1)!, count: Int(length))
        if kind == 0 { seen.loads += 1 }
        if kind == 7 { seen.views.append((try? JSONSerialization.jsonObject(with: data) as? [String: Any]) ?? [:]) }
        guard kind == 4 || kind == 5, let value = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
              let name = value["event"] as? String, let record = value["record"] as? [String: Any] else { return }
        seen.records.append((name, record))
    }
    private static let reply: ReplyFn = { _, _, _, _, _, _ in }

    private func send(_ text: String?, _ setter: SetFn, _ handle: UnsafeMutableRawPointer) {
        guard let text else { return setter(handle, nil, 0, 0) }
        Array(text.utf8).withUnsafeBufferPointer { setter(handle, $0.baseAddress, UInt32($0.count), 1) }
    }

    private func sendData(_ data: Data, _ setter: SetFn, _ handle: UnsafeMutableRawPointer) {
        data.withUnsafeBytes { setter(handle, $0.bindMemory(to: UInt8.self).baseAddress, UInt32(data.count), 1) }
    }

    private func json(_ value: [String: Any], _ fn: PostFn, _ handle: UnsafeMutableRawPointer) {
        let data = try! JSONSerialization.data(withJSONObject: value)
        data.withUnsafeBytes { fn(handle, $0.bindMemory(to: UInt8.self).baseAddress, UInt32(data.count)) }
    }

    private func spin(until done: () -> Bool, _ seconds: TimeInterval = 15) {
        let deadline = Date(timeIntervalSinceNow: seconds)
        while !done() && Date() < deadline { RunLoop.main.run(mode: .default, before: Date(timeIntervalSinceNow: 0.02)) }
    }

    private func pause(_ seconds: TimeInterval) { spin(until: { false }, seconds) }

    /// A measurement, in the log and as an activity in a simulator's result bundle.
    private func say(_ text: String) {
        print(text)
        XCTContext.runActivity(named: text) { _ in }
    }

    #if os(macOS)
    private var windows: [NSWindow] = []
    override func tearDown() { windows.forEach { $0.close() }; windows = [] }
    private var scale: CGFloat { windows.last?.backingScaleFactor ?? 2 }
    #else
    private var windows: [UIWindow] = []
    override func tearDown() { windows.forEach { $0.isHidden = true }; windows = [] }
    private var scale: CGFloat { windows.last?.screen.scale ?? 2 }
    #endif

    /// A window holding `view` at 400 × 300 points, so WebKit lays it out and paints.
    private func show(_ view: AnyObject) {
        #if os(macOS)
        let view = view as! NSView
        view.frame = CGRect(x: 0, y: 0, width: 400, height: 300)
        let window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 400, height: 300), styleMask: [.titled], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        window.contentView!.addSubview(view)
        window.orderFrontRegardless()
        windows.append(window)
        #else
        let view = view as! UIView
        view.frame = CGRect(x: 0, y: 0, width: 400, height: 300)
        let window = hostedWindowScene().map { UIWindow(windowScene: $0) } ?? UIWindow()
        window.frame = CGRect(x: 0, y: 0, width: 400, height: 300)
        window.addSubview(view)
        window.isHidden = false
        windows.append(window)
        #endif
    }

    private func mount(_ arm: Arm, _ seen: Seen, configuration: [String: Any], src: String?, document: Data? = nil, sandbox: String? = nil,
                       configuring: (UnsafeMutableRawPointer) -> Void = { _ in }) throws -> UnsafeMutableRawPointer {
        let handle = arm.create(9, Unmanaged.passUnretained(seen).toOpaque(), Self.event, Self.reply)!
        configuring(handle)
        json(configuration, arm.configure, handle)
        show(Unmanaged<AnyObject>.fromOpaque(arm.view(handle)!).takeUnretainedValue())
        if let document { sendData(document, arm.setDocument, handle) }
        send(sandbox, arm.setSandbox, handle)
        send(src, arm.setSrc, handle)
        return handle
    }

    /// The view the arm shows now, in the window in place of the last one, as
    /// the presenter swaps it at kind 7.
    private func current(_ arm: Arm, _ handle: UnsafeMutableRawPointer) -> AnyObject {
        let view = Unmanaged<AnyObject>.fromOpaque(arm.view(handle)!).takeUnretainedValue()
        #if os(macOS)
        let shown = view as! NSView
        if shown.window == nil { windows.last?.contentView?.subviews.forEach { $0.removeFromSuperview() }; shown.frame = CGRect(x: 0, y: 0, width: 400, height: 300); windows.last?.contentView?.addSubview(shown) }
        #else
        let shown = view as! UIView
        if shown.window == nil { windows.last?.subviews.forEach { $0.removeFromSuperview() }; shown.frame = CGRect(x: 0, y: 0, width: 400, height: 300); windows.last?.addSubview(shown) }
        #endif
        return view
    }

    nonisolated(unsafe) private static var server: CapturePages?
    private var baseURL: String { "http://127.0.0.1:\(Self.server!.port)" }
    override func setUpWithError() throws {
        #if os(macOS)
        _ = NSApplication.shared
        #endif
        if Self.server == nil { Self.server = try CapturePages() }
    }

    private func capture(_ arm: Arm, _ handle: UnsafeMutableRawPointer, _ seen: Seen, file: URL?, clip: [Double]? = nil) -> [String: Any]? {
        seen.records.removeAll { $0.event == "capture" }
        var op: [String: Any] = ["op": "captureScreenshot", "path": "app:/tmp/shot.png", "file": file?.path ?? ""]
        if let clip { op["clip"] = clip }
        json(op, arm.command, handle)
        spin { seen.records.contains { $0.event == "capture" } }
        return seen.records.first { $0.event == "capture" }?.record
    }

    private func temporary(_ name: String) -> URL {
        let folder = FileManager.default.temporaryDirectory.appendingPathComponent("iframe-capture-\(ProcessInfo.processInfo.processIdentifier)", isDirectory: true)
        return folder.appendingPathComponent(name)
    }

    /// The written PNG's pixels as RGBA, and its size.
    private func pixels(_ file: URL) throws -> (width: Int, height: Int, at: (Double, Double) -> [Int]) {
        let source = try XCTUnwrap(CGImageSourceCreateWithURL(file as CFURL, nil))
        let image = try XCTUnwrap(CGImageSourceCreateImageAtIndex(source, 0, nil))
        let (w, h) = (image.width, image.height)
        var bytes = [UInt8](repeating: 0, count: w * h * 4)
        let context = try XCTUnwrap(CGContext(data: &bytes, width: w, height: h, bitsPerComponent: 8, bytesPerRow: w * 4,
                                              space: CGColorSpace(name: CGColorSpace.sRGB)!, bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue))
        context.draw(image, in: CGRect(x: 0, y: 0, width: w, height: h))
        // Row 0 of the buffer is the image's top.
        return (w, h, { fx, fy in
            let x = min(w - 1, max(0, Int(fx * Double(w)))), y = min(h - 1, max(0, Int(fy * Double(h))))
            let o = (y * w + x) * 4
            return [Int(bytes[o]), Int(bytes[o + 1]), Int(bytes[o + 2])]
        })
    }

    private func isRed(_ p: [Int]) -> Bool { p[0] > 200 && p[1] < 60 && p[2] < 60 }

    /// The whole viewport is the frame's size in points times the screen's
    /// scale; what cannot be captured answers with a DOMException name.
    func testTheWholeViewportAndWhatIsRefused() throws {
        #if os(iOS)
        guard hostedWindowScene() != nil else { throw missing("scene", "WebKit paints a snapshot only in an app's window scene") }
        #endif
        let arm = try arm(), seen = Seen()
        let handle = try mount(arm, seen, configuration: ["partition": "memory-shot"], src: "\(baseURL)/target")
        defer { arm.destroy(handle) }
        spin { seen.records.contains { $0.event == "navigatesuccess" } }
        pause(0.3)
        let file = temporary("whole.png")
        let whole = try XCTUnwrap(capture(arm, handle, seen, file: file))
        XCTAssertEqual(whole["error"] as? String, "", "\(whole)")
        XCTAssertEqual(whole["path"] as? String, "app:/tmp/shot.png")
        XCTAssertEqual(whole["width"] as? Int, Int(400 * scale), "\(whole)")
        XCTAssertEqual(whole["height"] as? Int, Int(300 * scale), "\(whole)")
        let picture = try pixels(file)
        XCTAssertEqual(picture.width, Int(400 * scale))
        let top = picture.at(0.5, 0.05)
        XCTAssert(top[2] > 200 && top[0] < 60, "the page's blue band at the top: \(top)")

        let outside = try XCTUnwrap(capture(arm, handle, seen, file: file, clip: [2000, 2000, 10, 10]))
        XCTAssertEqual(outside["error"] as? String, "IndexSizeError", "\(outside)")
        let nowhere = try XCTUnwrap(capture(arm, handle, seen, file: nil))
        XCTAssertEqual(nowhere["error"] as? String, "SyntaxError", "\(nowhere)")
    }

    /// RFC D7's acceptance case: after a pinch to 1.5 and a pan to a nonzero
    /// offset, a clip from the element's `getBoundingClientRect()` captures
    /// that element and nothing around it.
    func testAClipAfterAPinchAndAPanCapturesItsElement() throws {
        #if os(iOS)
        guard hostedWindowScene() != nil else { throw missing("scene", "WebKit paints a snapshot only in an app's window scene") }
        #endif
        let arm = try arm(), seen = Seen()
        let handle = try mount(arm, seen, configuration: ["partition": "memory-pinch", "allowsMagnification": true], src: "\(baseURL)/target")
        defer { arm.destroy(handle) }
        spin { seen.records.contains { $0.event == "navigatesuccess" } }
        pause(0.3)
        let web = current(arm, handle) as! WKWebView
        #if os(macOS)
        web.setMagnification(1.5, centeredAt: .zero)
        // A pan, by the page's own scroll: WebKit on macOS moves the visual viewport with it.
        web.evaluateJavaScript("scrollBy(60, 40)") { _, _ in }
        #else
        web.scrollView.setZoomScale(1.5, animated: false)
        web.scrollView.setContentOffset(CGPoint(x: 90, y: 60), animated: false)
        #endif
        pause(0.8)
        var read: [Double]?
        web.evaluateJavaScript("(() => { const r = document.getElementById('target').getBoundingClientRect(), v = visualViewport; return [r.x, r.y, r.width, r.height, v.offsetLeft, v.offsetTop, v.pageLeft, v.pageTop, v.scale, scrollX, scrollY]; })()") { value, _ in
            read = (value as? [NSNumber])?.map(\.doubleValue) ?? []
        }
        spin { read != nil }
        let measured = try XCTUnwrap(read)
        XCTAssertEqual(measured.count, 11, "\(measured)")
        guard measured.count == 11 else { return }
        say("iframe capture: rect \(measured[0..<4]), visualViewport offset \(measured[4]),\(measured[5]) page \(measured[6]),\(measured[7]) scale \(measured[8]), scroll \(measured[9]),\(measured[10])")
        XCTAssertEqual(measured[8], 1.5, accuracy: 0.01, "the pinch reached the page")
        XCTAssert(measured[6] > 0 && measured[7] > 0, "the pan moved the visual viewport: \(measured)")
        let file = temporary("pinched.png")
        let shot = try XCTUnwrap(capture(arm, handle, seen, file: file, clip: Array(measured[0..<4])))
        XCTAssertEqual(shot["error"] as? String, "", "\(shot)")
        let picture = try pixels(file)
        XCTAssertEqual(Double(picture.width), 80 * 1.5 * Double(scale), accuracy: 2 * Double(scale), "\(shot)")
        XCTAssertEqual(Double(picture.height), 40 * 1.5 * Double(scale), accuracy: 2 * Double(scale), "\(shot)")
        for (x, y) in [(0.5, 0.5), (0.06, 0.08), (0.94, 0.08), (0.06, 0.92), (0.94, 0.92)] {
            XCTAssert(isRed(picture.at(x, y)), "(\(x), \(y)) is the element's red: \(picture.at(x, y))")
        }
    }

    /// A one-page PDF of `size`, filled with `colour`, and, when given, a
    /// link over its top half.
    private static func document(pages: Int, size: CGSize = CGSize(width: 612, height: 792), link: URL? = nil) -> Data {
        let data = NSMutableData()
        var box = CGRect(origin: .zero, size: size)
        let consumer = CGDataConsumer(data: data as CFMutableData)!
        let context = CGContext(consumer: consumer, mediaBox: &box, nil)!
        let colours: [(CGFloat, CGFloat, CGFloat)] = [(1, 0, 0), (0, 0.8, 0), (0, 0, 1)]
        for n in 0..<pages {
            context.beginPDFPage(nil)
            let c = colours[n % colours.count]
            context.setFillColor(red: c.0, green: c.1, blue: c.2, alpha: 1)
            context.fill(box)
            if let link { context.setURL(link as CFURL, for: CGRect(x: 0, y: size.height / 2, width: size.width, height: size.height / 2)) }
            context.endPDFPage()
        }
        context.closePDF()
        return data as Data
    }

    /// An `app:/` PDF shows in a PDF view at `#page=N`, fires `load`, draws
    /// its capture in-process, and gives way to a web view for HTML; a
    /// sandboxed one stays WebKit's.
    func testALocalPDFIsAPDFView() throws {
        #if os(iOS)
        guard hostedWindowScene() != nil else { throw missing("scene", "WebKit paints a snapshot only in an app's window scene") }
        #endif
        let arm = try arm(), seen = Seen()
        let pdf = Self.document(pages: 3, link: URL(string: "https://example.com/linked"))
        let handle = try mount(arm, seen, configuration: ["newwindow": true], src: "app:/tmp/doc.pdf#page=2", document: pdf)
        defer { arm.destroy(handle) }
        spin { !seen.views.isEmpty && seen.loads > 0 }
        XCTAssertEqual(seen.views.last?["pdf"] as? Int, 3, "\(seen.views)")
        XCTAssertEqual(seen.loads, 1)
        let view = try XCTUnwrap(current(arm, handle) as? PDFView, "a local PDF is PDFKit's view")
        pause(0.5)
        XCTAssertEqual(view.document?.pageCount, 3)
        XCTAssertEqual(view.currentPage.flatMap { view.document?.index(for: $0) }, 1, "#page=2 opened at the second page")
        XCTAssertEqual(view.displayMode, .singlePageContinuous)
        XCTAssertTrue(view.autoScales)

        // A capture draws the view as it shows now: the second page's green.
        let file = temporary("pdf.png")
        let shot = try XCTUnwrap(capture(arm, handle, seen, file: file, clip: [100, 100, 200, 100]))
        XCTAssertEqual(shot["error"] as? String, "", "\(shot)")
        XCTAssertEqual(shot["width"] as? Int, Int(200 * scale))
        let middle = try pixels(file).at(0.5, 0.5)
        XCTAssert(middle[1] > 150 && middle[0] < 80 && middle[2] < 80, "the second page's green: \(middle)")

        // `zoom` multiplies the automatic scale.
        let fit = view.scaleFactorForSizeToFit
        json(["newwindow": true, "zoom": 2], arm.configure, handle)
        pause(0.2)
        XCTAssertFalse(view.autoScales)
        XCTAssertEqual(view.scaleFactor, fit * 2, accuracy: 0.01)
        json(["newwindow": true], arm.configure, handle)
        pause(0.2)
        XCTAssertTrue(view.autoScales)

        // A person's own zoom stands through a later layout, as in a `PDFView`
        // of its own; `zoom` back to 1 gives `PDFView` its own scale bounds.
        let (low, high) = (view.minScaleFactor, view.maxScaleFactor)
        view.zoomIn(nil)
        view.zoomIn(nil)
        let zoomed = view.scaleFactor
        XCTAssertGreaterThan(zoomed, fit * 1.2, "the person zoomed in")
        view.frame = CGRect(x: 0, y: 0, width: 420, height: 320)
        #if os(macOS)
        view.layoutSubtreeIfNeeded()
        #else
        view.layoutIfNeeded()
        #endif
        pause(0.2)
        XCTAssertEqual(view.scaleFactor, zoomed, accuracy: 0.001, "a resize keeps the person's zoom")
        XCTAssertFalse(view.autoScales)
        json(["newwindow": true, "zoom": 3], arm.configure, handle)
        pause(0.2)
        json(["newwindow": true], arm.configure, handle)
        pause(0.2)
        XCTAssertTrue(view.autoScales)
        XCTAssertEqual(view.minScaleFactor, low, accuracy: 0.001)
        XCTAssertEqual(view.maxScaleFactor, high, accuracy: 0.001)

        // A link is the frame's: an http(s) one a `foreground-tab` for `newwindow=`.
        view.delegate?.pdfViewWillClick?(onLink: view, with: URL(string: "https://example.com/linked")!)
        XCTAssertEqual(seen.records.last?.event, "newwindow")
        XCTAssertEqual(seen.records.last?.record["disposition"] as? String, "foreground-tab")
        XCTAssertEqual(seen.records.last?.record["url"] as? String, "https://example.com/linked")

        // HTML again: a web view, `load` again.
        sendData(Data("<!doctype html><title>x</title><p>x".utf8), arm.setDocument, handle)
        send("app:/tmp/page.html", arm.setSrc, handle)
        spin { seen.views.count >= 2 && seen.loads >= 2 }
        XCTAssertEqual(seen.views.last?.isEmpty, true, "\(seen.views)")
        XCTAssertTrue(current(arm, handle) is WKWebView)
        XCTAssertEqual(seen.loads, 2)
    }

    func testASandboxedPDFStaysWebKits() throws {
        #if os(iOS)
        guard hostedWindowScene() != nil else { throw missing("scene", "WebKit paints a snapshot only in an app's window scene") }
        #endif
        let arm = try arm(), seen = Seen()
        let handle = try mount(arm, seen, configuration: [:], src: "app:/tmp/doc.pdf", document: Self.document(pages: 1), sandbox: "allow-scripts")
        defer { arm.destroy(handle) }
        spin { seen.loads > 0 }
        XCTAssertTrue(seen.views.isEmpty)
        XCTAssertTrue(current(arm, handle) is WKWebView)
    }

    /// A short document in a taller view: where PDFKit puts it (the clone
    /// keeps it at the top, RFC D10's acceptance case).
    func testWhereAShortDocumentSits() throws {
        #if os(iOS)
        guard hostedWindowScene() != nil else { throw missing("scene", "WebKit paints a snapshot only in an app's window scene") }
        #endif
        let arm = try arm(), seen = Seen()
        let handle = try mount(arm, seen, configuration: [:], src: "app:/tmp/short.pdf", document: Self.document(pages: 1, size: CGSize(width: 600, height: 200)))
        defer { arm.destroy(handle) }
        spin { seen.loads > 0 }
        let view = try XCTUnwrap(current(arm, handle) as? PDFView)
        pause(0.5)
        let page = try XCTUnwrap(view.document?.page(at: 0))
        let box = view.convert(page.bounds(for: .mediaBox), from: page)
        #if os(macOS)
        let top = view.isFlipped ? box.minY : view.bounds.height - box.maxY
        #else
        let top = box.minY
        #endif
        say("iframe pdf: a 600 x 200 page in a 400 x 300 view sits at \(box) (top \(top))")
        XCTAssertEqual(box.width, 400, accuracy: 30, "the page fits the width")
        // PDFView centres a document shorter than the view; the clone's
        // top-aligned body is a declared difference (RFC §11, stage 3).
        XCTAssertEqual(top, (300 - box.height) / 2, accuracy: 16, "\(box)")
    }

    #if os(macOS)
    /// What the arm owns that a hatch changed, said by name in a development build (RFC §9).
    func testAChangedDelegateIsSaidByName() throws {
        let arm = try arm(), seen = Seen()
        let handle = try mount(arm, seen, configuration: ["partition": "memory-owned"], src: "\(baseURL)/target")
        defer { arm.destroy(handle) }
        let web = try XCTUnwrap(current(arm, handle) as? WKWebView)
        let pipe = Pipe(), saved = dup(STDERR_FILENO)
        dup2(pipe.fileHandleForWriting.fileDescriptor, STDERR_FILENO)
        web.navigationDelegate = nil
        web.pageZoom = 2
        web.configuration.userContentController.addUserScript(WKUserScript(source: "1", injectionTime: .atDocumentEnd, forMainFrameOnly: true))
        arm.checkOwned(handle)
        arm.checkOwned(handle)
        dup2(saved, STDERR_FILENO)
        close(saved)
        try pipe.fileHandleForWriting.close()
        let said = String(decoding: pipe.fileHandleForReading.readDataToEndOfFile(), as: UTF8.self)
        XCTAssert(said.contains("the app's hatch changed navigationDelegate, which the web arm owns"), said)
        XCTAssert(said.contains("changed pageZoom"), said)
        XCTAssert(said.contains("changed the user scripts of the arm's content controller"), said)
        XCTAssertEqual(said.components(separatedBy: "navigationDelegate").count, 2, "said once: \(said)")
    }

    private typealias ConfiguringFn = @convention(c) (UnsafeMutableRawPointer?, UnsafeMutableRawPointer?) -> Void
    /// What the host's moment before construction was handed: the frame's
    /// context and its configuration's data store.
    nonisolated(unsafe) private static var configured: [(context: UnsafeMutableRawPointer?, store: WKWebsiteDataStore)] = []
    nonisolated(unsafe) private static var swapsStore = false
    private static let configuring: ConfiguringFn = { context, raw in
        let configuration = Unmanaged<WKWebViewConfiguration>.fromOpaque(raw!).takeUnretainedValue()
        configured.append((context, configuration.websiteDataStore))
        configuration.applicationNameForUserAgent = "Hatched/1"
        if swapsStore { configuration.websiteDataStore = .nonPersistent() }
    }

    /// Tier 3's moment before construction (RFC §9): the configuration, the
    /// arm's data store on it, before each web view the arm makes; what the
    /// hatch adds is the web view's, and a store it swaps is said by name.
    func testTheConfigurationComesBeforeEachWebView() throws {
        let arm = try arm(), seen = Seen()
        let library = try XCTUnwrap(dlopen(ProcessInfo.processInfo.environment["EXACT_TEST_WEB_MODULE"], RTLD_NOW | RTLD_LOCAL))
        let setConfiguring = unsafeBitCast(try XCTUnwrap(dlsym(library, "exact_web_set_configuring")), to: (@convention(c) (UnsafeMutableRawPointer?, ConfiguringFn?) -> Void).self)
        defer { Self.configured = []; Self.swapsStore = false }
        let context = Unmanaged.passUnretained(seen).toOpaque()

        let shared = try mount(arm, seen, configuration: [:], src: nil, configuring: { setConfiguring($0, Self.configuring) })
        defer { arm.destroy(shared) }
        XCTAssertEqual(Self.configured.count, 1)
        XCTAssertEqual(Self.configured.last?.context, context)
        XCTAssertTrue(Self.configured.last?.store === WKWebsiteDataStore.default(), "an unpartitioned frame's: the default store")
        XCTAssertEqual((current(arm, shared) as? WKWebView)?.configuration.applicationNameForUserAgent, "Hatched/1")

        let handle = try mount(arm, seen, configuration: ["partition": "memory-configured"], src: nil, configuring: { setConfiguring($0, Self.configuring) })
        defer { arm.destroy(handle) }
        let first = try XCTUnwrap(current(arm, handle) as? WKWebView)
        XCTAssertEqual(Self.configured.count, 2)
        let store = try XCTUnwrap(Self.configured.last?.store)
        XCTAssertFalse(store.isPersistent, "the partition's store, set before the hatch")
        XCTAssertTrue(first.configuration.websiteDataStore === store)
        XCTAssertEqual(first.configuration.applicationNameForUserAgent, "Hatched/1")

        // A new partition is a new web view, and the moment again; a store the
        // hatch swaps there is said by name in a development build.
        Self.swapsStore = true
        json(["partition": "memory-reconfigured"], arm.configure, handle)
        let second = try XCTUnwrap(current(arm, handle) as? WKWebView)
        XCTAssertFalse(second === first)
        XCTAssertEqual(Self.configured.count, 3)
        let pipe = Pipe(), saved = dup(STDERR_FILENO)
        dup2(pipe.fileHandleForWriting.fileDescriptor, STDERR_FILENO)
        arm.checkOwned(handle)
        dup2(saved, STDERR_FILENO)
        close(saved)
        try pipe.fileHandleForWriting.close()
        let said = String(decoding: pipe.fileHandleForReading.readDataToEndOfFile(), as: UTF8.self)
        XCTAssert(said.contains("the app's hatch changed websiteDataStore, which the web arm owns"), said)
    }

    private static let prefers: ConfiguringFn = { _, raw in
        let configuration = Unmanaged<WKWebViewConfiguration>.fromOpaque(raw!).takeUnretainedValue()
        configuration.defaultWebpagePreferences.allowsContentJavaScript = false
        configuration.preferences.minimumFontSize = 30
    }

    /// A hatched frame's preferences are its own (RFC §9): what its hatch
    /// writes, and a scheduling policy another frame says, change no other
    /// frame's, though unhatched frames share one copy.
    func testAHatchedFramesPreferencesAreItsOwn() throws {
        let arm = try arm(), seen = Seen()
        let library = try XCTUnwrap(dlopen(ProcessInfo.processInfo.environment["EXACT_TEST_WEB_MODULE"], RTLD_NOW | RTLD_LOCAL))
        let setConfiguring = unsafeBitCast(try XCTUnwrap(dlsym(library, "exact_web_set_configuring")), to: (@convention(c) (UnsafeMutableRawPointer?, ConfiguringFn?) -> Void).self)
        let hatched = try mount(arm, seen, configuration: ["hatched": true], src: nil, configuring: { setConfiguring($0, Self.prefers) })
        defer { arm.destroy(hatched) }
        let scheduled = try mount(arm, seen, configuration: ["inactiveSchedulingPolicy": "none"], src: nil)
        defer { arm.destroy(scheduled) }
        let plain = try mount(arm, seen, configuration: [:], src: nil)
        defer { arm.destroy(plain) }
        let other = try mount(arm, seen, configuration: [:], src: nil)
        defer { arm.destroy(other) }
        let view = { (handle: UnsafeMutableRawPointer) in Unmanaged<WKWebView>.fromOpaque(arm.view(handle)!).takeUnretainedValue() }
        XCTAssertFalse(view(hatched).configuration.defaultWebpagePreferences.allowsContentJavaScript)
        XCTAssertEqual(view(hatched).configuration.preferences.minimumFontSize, 30)
        XCTAssertTrue(view(plain).configuration.defaultWebpagePreferences.allowsContentJavaScript, "another frame keeps content JavaScript")
        XCTAssertEqual(view(plain).configuration.preferences.minimumFontSize, 0)
        XCTAssertFalse(view(hatched).configuration.preferences.javaScriptCanOpenWindowsAutomatically, "the arm's unsaid preferences on its own copy")
        XCTAssertTrue(view(hatched).configuration.preferences.isElementFullscreenEnabled)
        XCTAssertEqual(view(scheduled).configuration.preferences.inactiveSchedulingPolicy, .none)
        XCTAssertEqual(view(plain).configuration.preferences.inactiveSchedulingPolicy, .suspend, "WebKit's default stays every other frame's")
        XCTAssertTrue(view(plain).configuration.preferences === view(other).configuration.preferences, "unhatched frames share the template's")
    }
    #endif
}

/// `/target` on a loopback port: a 400 px wide, 1200 px tall page, a blue
/// band at its top, and a red element at (220, 160), 80 × 40 CSS px, inside
/// a blue frame, so a clip off by a few pixels shows blue.
final class CapturePages {
    private let listener: NWListener

    var port: UInt16 { listener.port!.rawValue }

    init() throws { listener = try loopbackListener(Self.serve) }

    static let target = """
    <!doctype html><meta name="viewport" content="width=device-width, initial-scale=1"><title>Target</title>
    <style>html,body{margin:0;background:#fff;width:400px;height:1200px}div{position:absolute}</style>
    <div style="left:0;top:0;width:400px;height:40px;background:rgb(0,0,255)"></div>
    <div style="left:200px;top:140px;width:120px;height:80px;background:rgb(0,0,255)"></div>
    <div id="target" style="left:220px;top:160px;width:80px;height:40px;background:rgb(255,0,0)"></div>
    """

    private static func serve(_ connection: NWConnection) {
        connection.start(queue: .main)
        connection.receive(minimumIncompleteLength: 1, maximumLength: 65536) { data, _, _, _ in
            guard let data, let line = String(decoding: data, as: UTF8.self).split(separator: "\r\n").first else { return connection.cancel() }
            let path = line.split(separator: " ").dropFirst().first.map(String.init) ?? "/"
            let body = path == "/target" ? target : ""
            let status = path == "/target" ? "200 OK" : "404 Not Found"
            let response = "HTTP/1.1 \(status)\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: \(body.utf8.count)\r\nConnection: close\r\n\r\n\(body)"
            connection.send(content: Data(response.utf8), completion: .contentProcessed { _ in connection.cancel() })
        }
    }
}
#endif
