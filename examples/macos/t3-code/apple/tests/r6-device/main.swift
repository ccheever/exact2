import AppKit
import Network
import XCTest

// Lane r6-media: the device screen (R6DeviceStream.swift) against loopback stand-ins for the
// server's /api/device-hub proxy: an HTTP peer that serves serve-sim's multipart MJPEG feed and a
// WebSocket peer that plays the helper input socket. A real hub needs the server's device tools
// installed, which this lane does not do; these peers speak the same wire format the reference's
// stream.ts does. `T3_DEVICE_TEST_DIR` receives the rendered screen.
final class Peer {
    let listener: NWListener
    private(set) var port: UInt16 = 0
    private let queue = DispatchQueue(label: "r6-device-peer")
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
            if websocket { self.readMessages(connection) } else { self.readRequest(connection) }
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
        XCTAssertEqual(R6DeviceWire.tagged(0x04, ["button": "home"]), Data([0x04]) + Data(#"{"button":"home"}"#.utf8))
        XCTAssertEqual(R6DeviceWire.nextOrientation(nil), "landscape_left")
        XCTAssertEqual(R6DeviceWire.nextOrientation("landscape_right"), "portrait")
        // rawPoint: only a portrait framebuffer shown rotated is remapped.
        XCTAssertTrue(R6DeviceWire.rawPoint(x: 0.25, y: 0.75, screen: (390, 844, "landscape_left")) == (0.75, 0.75))
        XCTAssertTrue(R6DeviceWire.rawPoint(x: 0.25, y: 0.75, screen: (390, 844, "portrait_upside_down")) == (0.75, 0.25))
        XCTAssertTrue(R6DeviceWire.rawPoint(x: 0.25, y: 0.75, screen: (844, 390, "landscape_left")) == (0.25, 0.75))
        XCTAssertEqual(R6DeviceWire.fit(CGSize(width: 100, height: 200), in: CGRect(x: 0, y: 0, width: 300, height: 300)), CGRect(x: 75, y: 0, width: 150, height: 300))
        let url = R6DeviceWire.url(origin: URL(string: "https://host.example:8443")!, path: "/vendor/serve-sim/helper/ws", query: [URLQueryItem(name: "device", value: "A-B")], ticket: "t", hostId: "local", websocket: true)
        XCTAssertEqual(url?.absoluteString, "wss://host.example:8443/api/device-hub/vendor/serve-sim/helper/ws?device=A-B&wsTicket=t&hostId=local")
        // Split frames: two whole JPEGs and the start of a third survive arbitrary chunking.
        let a = jpeg(width: 4, height: 8, color: .red), b = jpeg(width: 4, height: 8, color: .blue)
        var stream = Data("--frame\r\nContent-Type: image/jpeg\r\n\r\n".utf8) + a + Data("\r\n--frame\r\n\r\n".utf8) + b + Data("\r\n--frame\r\n\r\n".utf8) + a.prefix(10)
        var found: [Data] = [], buffer = Data()
        while !stream.isEmpty { buffer.append(stream.prefix(7)); stream = stream.dropFirst(7); let (images, rest) = R6DeviceWire.frames(buffer); found += images; buffer = rest }
        XCTAssertEqual(found, [a, b])
        XCTAssertEqual(buffer, a.prefix(10))
    }

    // Lane r7-device: iOS video is serve-sim's AVCC body now, with this MJPEG feed as its fallback,
    // and Android streams over H.264; both are covered against a full loopback hub (HTTP and
    // WebSocket on one origin) in apple/tests/r7-device (testIosUndecodableDescriptionFallsBackToMjpeg,
    // testAndroidStreamDecodesAndForwardsInput).
    func testMissingAccessSaysWhy() {
        let offline = R6DeviceScreenView(key: "k", platform: "ios", deviceId: "SIM-1", hostId: "local", access: { done in done(nil, nil) }) {}
        offline.start()
        spin(until: { offline.status == "error" }, timeout: 3)
        XCTAssertEqual(offline.detail, "Reconnect to the environment and try again.")
    }

    func testInputSocketCarriesHomeAndRotate() throws {
        let helper = try Peer(websocket: true)
        let view = R6DeviceScreenView(key: "local\u{0}SIM-1", platform: "ios", deviceId: "SIM-1", hostId: "local",
                                      access: { done in done(URL(string: "http://127.0.0.1:\(helper.port)")!, "ticket-2") }) {}
        let window = window(view)
        view.start()
        spin(until: { view.inputConnected })
        XCTAssertTrue(view.inputConnected)
        helper.push(R6DeviceWire.tagged(0x82, ["width": 390, "height": 844, "orientation": "portrait"]))
        spin(until: { view.screen != nil })
        XCTAssertEqual(view.screen?.orientation, "portrait")
        view.press("home")
        view.press("rotate")
        spin(until: { helper.received.count >= 3 })
        let packets = helper.received
        XCTAssertEqual(packets.first, R6DeviceWire.tagged(0x0d, ["enabled": false]), "the helper's hardware keyboard is released on open")
        XCTAssertTrue(packets.contains(R6DeviceWire.tagged(0x04, ["button": "home"])))
        XCTAssertTrue(packets.contains(R6DeviceWire.tagged(0x07, ["orientation": "landscape_left"])))
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
