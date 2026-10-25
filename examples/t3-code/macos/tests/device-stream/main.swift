import AppKit
import Network
import XCTest

// The device screen (T3DeviceStream.swift) against loopback stand-ins for the
// server's /api/device-hub proxy: an HTTP peer that serves serve-sim's multipart MJPEG feed and a
// WebSocket peer that plays the helper input socket. A real hub needs the server's device tools
// installed, which this test does not do; these peers speak the same wire format the reference's
// stream.ts does. `T3_DEVICE_TEST_DIR` receives the rendered screen.
final class Peer {
    let listener: NWListener
    private(set) var port: UInt16 = 0
    private let queue = DispatchQueue(label: "device-stream-peer")
    private var lines: [String] = []
    private var packets: [Data] = []
    private var connections: [NWConnection] = []
    var onHTTP: ((String, NWConnection) -> Void)?

    init(websocket: Bool) throws {
        let parameters: NWParameters
        if websocket {
            parameters = NWParameters.tcp
            let options = NWProtocolWebSocket.Options()
            options.autoReplyPing = true
            parameters.defaultProtocolStack.applicationProtocols.insert(options, at: 0)
        } else { parameters = .tcp }
        listener = try NWListener(using: parameters, on: .any)
        let ready = DispatchSemaphore(value: 0)
        listener.stateUpdateHandler = { if case .ready = $0 { ready.signal() } }
        listener.newConnectionHandler = { [weak self] connection in
            guard let self else { return }
            self.connections.append(connection)
            connection.start(queue: self.queue)
            if websocket {
                connection.stateUpdateHandler = { [weak self] state in
                    if case .ready = state { self?.push(Data([0x83])) }
                }
                self.readMessages(connection)
            } else { self.readRequest(connection) }
        }
        listener.start(queue: queue)
        _ = ready.wait(timeout: .now() + 5)
        port = listener.port?.rawValue ?? 0
    }
    var requests: [String] { queue.sync { lines } }
    var received: [Data] { queue.sync { packets } }

    private func readRequest(_ connection: NWConnection) {
        connection.receive(minimumIncompleteLength: 1, maximumLength: 65_536) { [weak self] data, _, _, _ in
            guard let self, let data, let head = String(data: data, encoding: .utf8)?.split(separator: "\r\n").first else { return }
            let line = String(head)
            self.lines.append(line)
            self.onHTTP?(line, connection)
        }
    }
    private func readMessages(_ connection: NWConnection) {
        connection.receiveMessage { [weak self] data, context, _, error in
            guard let self, error == nil else { return }
            if let data, !data.isEmpty { self.packets.append(data) }
            if context?.isFinal == false || data != nil { self.readMessages(connection) }
        }
    }
    func push(_ data: Data) {
        queue.async {
            let metadata = NWProtocolWebSocket.Metadata(opcode: .binary)
            let context = NWConnection.ContentContext(identifier: "config", metadata: [metadata])
            for connection in self.connections { connection.send(content: data, contentContext: context, isComplete: true, completion: .idempotent) }
        }
    }
    deinit { listener.cancel(); connections.forEach { $0.cancel() } }
}

private let hatchResolve: ExactHatches.ResolveFn = { _, _, _, _, _ in 0 }
private let hatchAct: ExactHatches.ActFn = { _, _, _ in 0 }
private let hatchLog: ExactHatches.LogFn = { _, _, _ in }
private let hatchDelegate: ExactHatches.DelegateFn = { _, _, _ in }
/// A host's hatch table that answers nothing, for elements made by hand.
let quietHatches: ExactHatches = {
    let table = UnsafeMutableRawPointer.allocate(byteCount: 40, alignment: 8)
    table.initializeMemory(as: UInt8.self, repeating: 0, count: 40)
    table.storeBytes(of: UInt32(40), as: UInt32.self)
    table.storeBytes(of: unsafeBitCast(hatchResolve, to: UnsafeRawPointer.self), toByteOffset: 8, as: UnsafeRawPointer.self)
    table.storeBytes(of: unsafeBitCast(hatchAct, to: UnsafeRawPointer.self), toByteOffset: 16, as: UnsafeRawPointer.self)
    table.storeBytes(of: unsafeBitCast(hatchLog, to: UnsafeRawPointer.self), toByteOffset: 24, as: UnsafeRawPointer.self)
    table.storeBytes(of: unsafeBitCast(hatchDelegate, to: UnsafeRawPointer.self), toByteOffset: 32, as: UnsafeRawPointer.self)
    return ExactHatches(host: nil, table: UnsafeRawPointer(table))!
}()

final class DeviceStreamTests: XCTestCase {
    private func spin(until condition: () -> Bool, timeout: TimeInterval = 10) {
        let end = Date().addingTimeInterval(timeout)
        while !condition() && Date() < end { RunLoop.main.run(until: Date().addingTimeInterval(0.05)) }
    }
    private func jpeg(width: Int, height: Int, color: NSColor) -> Data {
        let rep = NSBitmapImageRep(bitmapDataPlanes: nil, pixelsWide: width, pixelsHigh: height, bitsPerSample: 8, samplesPerPixel: 4, hasAlpha: true, isPlanar: false, colorSpaceName: .deviceRGB, bytesPerRow: 0, bitsPerPixel: 0)!
        NSGraphicsContext.saveGraphicsState()
        NSGraphicsContext.current = NSGraphicsContext(bitmapImageRep: rep)
        color.setFill(); NSRect(x: 0, y: 0, width: width, height: height).fill()
        NSColor.white.setFill(); NSRect(x: width / 4, y: height / 3, width: width / 2, height: height / 6).fill()
        NSGraphicsContext.restoreGraphicsState()
        return rep.representation(using: .jpeg, properties: [.compressionFactor: 0.9])!
    }
    private func window(_ view: NSView) -> NSWindow {
        let window = NSWindow(contentRect: NSRect(x: 100, y: 100, width: 300, height: 600), styleMask: [.titled], backing: .buffered, defer: false)
        view.frame = window.contentView!.bounds
        window.contentView!.addSubview(view)
        window.orderFrontRegardless()
        return window
    }

    func testWireFormat() {
        XCTAssertEqual(T3DeviceWire.tagged(0x04, ["button": "home"]), Data([0x04]) + Data(#"{"button":"home"}"#.utf8))
        XCTAssertEqual(T3DeviceWire.nextOrientation(nil), "landscape_left")
        XCTAssertEqual(T3DeviceWire.nextOrientation("landscape_right"), "portrait")
        // rawPoint: only a portrait framebuffer shown rotated is remapped.
        XCTAssertTrue(T3DeviceWire.rawPoint(x: 0.25, y: 0.75, screen: (390, 844, "landscape_left")) == (0.75, 0.75))
        XCTAssertTrue(T3DeviceWire.rawPoint(x: 0.25, y: 0.75, screen: (390, 844, "portrait_upside_down")) == (0.75, 0.25))
        XCTAssertTrue(T3DeviceWire.rawPoint(x: 0.25, y: 0.75, screen: (844, 390, "landscape_left")) == (0.25, 0.75))
        XCTAssertEqual(T3DeviceWire.fit(CGSize(width: 100, height: 200), in: CGRect(x: 0, y: 0, width: 300, height: 300)), CGRect(x: 75, y: 0, width: 150, height: 300))
        let url = T3DeviceWire.url(origin: URL(string: "https://host.example:8443")!, path: "/vendor/serve-sim/helper/ws", query: [URLQueryItem(name: "device", value: "A-B")], ticket: "t", hostId: "local", websocket: true)
        XCTAssertEqual(url?.absoluteString, "wss://host.example:8443/api/device-hub/vendor/serve-sim/helper/ws?device=A-B&wsTicket=t&hostId=local")
        // Split frames: two whole JPEGs and the start of a third survive arbitrary chunking.
        let a = jpeg(width: 4, height: 8, color: .red), b = jpeg(width: 4, height: 8, color: .blue)
        var stream = Data("--frame\r\nContent-Type: image/jpeg\r\n\r\n".utf8) + a + Data("\r\n--frame\r\n\r\n".utf8) + b + Data("\r\n--frame\r\n\r\n".utf8) + a.prefix(10)
        var found: [Data] = [], buffer = Data()
        while !stream.isEmpty { buffer.append(stream.prefix(7)); stream = stream.dropFirst(7); let (images, rest) = T3DeviceWire.frames(buffer); found += images; buffer = rest }
        XCTAssertEqual(found, [a, b])
        XCTAssertEqual(buffer, a.prefix(10))
    }

    // iOS video is serve-sim's AVCC body now, with this MJPEG feed as its fallback,
    // and Android streams over H.264; both are covered against a full loopback hub (HTTP and
    // WebSocket on one origin) in macos/tests/device-video (testIosUndecodableDescriptionFallsBackToMjpeg,
    // testAndroidStreamDecodesAndForwardsInput).
    func testMissingAccessSaysWhy() {
        let offline = T3DeviceScreenView(key: "k", platform: "ios", deviceId: "SIM-1", hostId: "local", access: { done in done(nil, nil) }) {}
        offline.start()
        spin(until: { offline.status == "error" }, timeout: 3)
        XCTAssertEqual(offline.detail, "Reconnect to the environment and try again.")
    }

    /// The data module's hub access: each use asks (`want`, a `t3.status`); a stream node's words answer every ask up to the
    /// count they name for this instance, an empty ticket answers with nothing, and words without an answer for this
    /// instance (the event log's node, an older module's ask) answer none.
    func testHubAccessAsksTheDataModuleForEachUse() {
        let hub = T3DeviceHubAccess()
        var asks = 0, answers: [String] = []
        hub.changed = { asks += 1 }
        for _ in 0..<2 { hub.access { _, ticket in answers.append(ticket ?? "none") } }
        spin(until: { hub.want == 2 }, timeout: 1)
        XCTAssertEqual(asks, 2)
        XCTAssertEqual(hub.status["module"] as? String, hub.id)
        hub.received(origin: nil, ticket: nil, ask: nil) // the event log's node
        hub.received(origin: "http://127.0.0.1:4001", ticket: "old", ask: "another-module:9")
        XCTAssertEqual(answers, [])
        hub.received(origin: "http://127.0.0.1:4001", ticket: "ticket-1", ask: "\(hub.id):1")
        XCTAssertEqual(answers, ["ticket-1"]) // the second ask came after the mint
        hub.received(origin: "http://127.0.0.1:4001", ticket: "ticket-1", ask: "\(hub.id):1") // the other stream node
        XCTAssertEqual(answers, ["ticket-1"])
        var origin: URL?
        hub.access { url, ticket in origin = url; answers.append(ticket ?? "none") }
        spin(until: { hub.want == 3 }, timeout: 1)
        hub.received(origin: "http://127.0.0.1:4001", ticket: "ticket-2", ask: "\(hub.id):3")
        XCTAssertEqual(answers, ["ticket-1", "ticket-2", "ticket-2"])
        XCTAssertEqual(origin?.absoluteString, "http://127.0.0.1:4001")
        hub.access { _, ticket in answers.append(ticket ?? "none") }
        spin(until: { hub.want == 4 }, timeout: 1)
        hub.received(origin: "", ticket: nil, ask: "\(hub.id):4") // the mint failed
        XCTAssertEqual(answers.last, "none")
        // The Tools drawer's reads reuse a good ticket until the hub refuses it.
        hub.access { _, _ in }
        spin(until: { hub.want == 5 }, timeout: 1)
        hub.received(origin: "http://127.0.0.1:4001", ticket: "ticket-5", ask: "\(hub.id):5")
        var reused: String?
        hub.reusing { _, ticket in reused = ticket }
        spin(until: { reused != nil }, timeout: 1)
        XCTAssertEqual([reused, String(hub.want)], ["ticket-5", "5"])
        hub.expire()
        hub.reusing { _, _ in }
        spin(until: { hub.want == 6 }, timeout: 1)
        XCTAssertEqual(hub.want, 6)
        let streams = T3DeviceStreams(changed: { _ in })
        XCTAssertEqual((streams.status["deviceHub"] as? [String: Any])?["want"] as? Int, 0)
    }

    /// A new ticket on the stream's node (any word but the overlay's) leaves the stream and the accessibility reads as they
    /// are: neither asks for another ticket, so a ticket never leads to another.
    func testANewTicketOnTheNodeRestartsNothing() {
        let hub = T3DeviceHubAccess()
        var asks = 0, reads = 0 // the stream's ask is left waiting, so the stream stays where it starts
        let streams = T3DeviceStreams(access: { _ in asks += 1 }, changed: { _ in }, hub: hub, toolAccess: { done in reads += 1; done(nil, nil) })
        let host = NSView(frame: NSRect(x: 0, y: 0, width: 300, height: 600))
        let element = ExactElement(hatch: .t3Media, id: "screen", node: 1, hatches: quietHatches)
        element.view = host
        let words = { (ticket: String, ask: Int) in ExactData(["media-kind": "device-ios", "device-id": "SIM-1", "device-host": "local", "device-view": "flat", "device-ax": "1",
                                                              "device-origin": "http://127.0.0.1:9", "device-ticket": ticket, "device-ask": "\(hub.id):\(ask)"]) }
        element.data = words("", 0)
        streams.install(element)
        spin(until: { false }, timeout: 0.5)
        let started = reads
        XCTAssertEqual(asks, 1)
        XCTAssertGreaterThan(started, 0)
        element.data = words("ticket-1", 1)
        streams.install(element)
        element.data = words("", 1) // the data module takes the answered ticket off the node
        streams.install(element)
        spin(until: { false }, timeout: 0.5)
        XCTAssertEqual([asks, hub.answered, reads], [1, 1, started])
        streams.destroy()
    }

    func testInputSocketCarriesHomeAndRotate() throws {
        let helper = try Peer(websocket: true)
        let view = T3DeviceScreenView(key: "local\u{0}SIM-1", platform: "ios", deviceId: "SIM-1", hostId: "local",
                                      access: { done in done(URL(string: "http://127.0.0.1:\(helper.port)")!, "ticket-2") }) {}
        let window = window(view)
        view.start()
        spin(until: { view.inputConnected })
        XCTAssertTrue(view.inputConnected)
        helper.push(T3DeviceWire.tagged(0x82, ["width": 390, "height": 844, "orientation": "portrait"]))
        spin(until: { view.screen != nil })
        XCTAssertEqual(view.screen?.orientation, "portrait")
        view.press("home")
        view.press("rotate")
        spin(until: { helper.received.count >= 2 })
        let packets = helper.received
        XCTAssertTrue(packets.contains(T3DeviceWire.tagged(0x04, ["button": "home"])))
        XCTAssertTrue(packets.contains(T3DeviceWire.tagged(0x07, ["orientation": "landscape_left"])))
        view.stop()
        window.orderOut(nil)
    }
}

/// The window server's picture of one of this process's windows (looked up at run time, as the agent does).
func CGWindowListCreateImageForTest(_ number: Int) -> CGImage? {
    typealias Create = @convention(c) (CGRect, UInt32, UInt32, UInt32) -> Unmanaged<CGImage>?
    guard let symbol = dlsym(UnsafeMutableRawPointer(bitPattern: -2), "CGWindowListCreateImage") else { return nil }
    return unsafeBitCast(symbol, to: Create.self)(.null, 1 << 3, UInt32(number), 1 << 0 | 1 << 3)?.takeRetainedValue()
}

_ = NSApplication.shared
NSApp.setActivationPolicy(.accessory)
let suite = XCTestSuite(forTestCaseClass: DeviceStreamTests.self)
suite.run()
let run = suite.testRun!
print("Executed \(run.executionCount) tests, with \(run.totalFailureCount) failures")
exit(run.totalFailureCount == 0 ? 0 : 1)
