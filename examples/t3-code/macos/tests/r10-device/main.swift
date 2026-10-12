import AppKit
import SceneKit
import XCTest
import simd
import Network

// Lane r10-device: the iPhone Duo's panel feeds under a loaded main thread (a feed that falls
// behind must come back, not go dark), and the physical hand-off (one elected feed; an orbit to
// the other display asks for facedown / faceup), against the loopback hub in hub.swift.
// `T3_DEVICE_GLB_DIR` (a T3 server's built client assets) adds the served model the orbit needs.
final class R10DeviceTests: XCTestCase {
    private func spin(until condition: () -> Bool, timeout: TimeInterval = 10) {
        let end = Date().addingTimeInterval(timeout)
        while !condition() && Date() < end { RunLoop.main.run(until: Date().addingTimeInterval(0.02)) }
    }
    private func json(_ data: Data) -> [String: Any] { (try? JSONSerialization.jsonObject(with: data) as? [String: Any]) ?? [:] }
    private func config(_ screenId: Int, _ angle: Double, _ pose: String?, physical: Bool = false, orientation: String = "portrait") -> Data {
        R6DeviceWire.tagged(0x82, ["width": screenId == 1 ? 784 : 1125, "height": screenId == 1 ? 1140 : 1600, "orientation": orientation, "screenId": screenId,
                                   "supportsHingeAngle": true, "supportsPhysicalOrientation": physical, "hingeAngle": angle, "hingePose": pose ?? NSNull()])
    }

    /// A hub whose feeds arrive in bursts, as a busy main thread receives them: `burst` samples per
    /// write every `every` ms (more than the decoder's soft queue of 8 at once).
    private func burstHub(_ inner: Encoded, _ cover: Encoded, burst: Int, every: Int, elected: (() -> Encoded)? = nil) throws -> Hub {
        let hub = try Hub()
        func stream(_ connection: NWConnection, _ encoded: @escaping () -> Encoded) {
            hub.open(connection, type: "application/octet-stream")
            var current = encoded()
            hub.write(connection, R7H264.Demuxer.envelope(.description, current.avcC))
            let timer = DispatchSource.makeTimerSource(queue: hub.queue)
            var index = 0
            timer.schedule(deadline: .now(), repeating: .milliseconds(every))
            timer.setEventHandler {
                guard connection.state == .ready else { return timer.cancel() }
                let next = encoded()
                if next.avcC != current.avcC { current = next; index = 0; hub.write(connection, R7H264.Demuxer.envelope(.description, current.avcC)) }
                var chunk = Data()
                for _ in 0..<burst {
                    let sample = current.samples[index % current.samples.count]; index += 1
                    chunk += R7H264.Demuxer.envelope(sample.key ? .keyframe : .delta, sample.data)
                }
                hub.write(connection, chunk)
            }
            timer.resume()
            hub.timers.append(timer)
        }
        hub.http.append(("/panel/1/stream.avcc", { _, connection in stream(connection) { cover } }))
        hub.http.append(("/panel/3/stream.avcc", { _, connection in stream(connection) { inner } }))
        hub.http.append(("/stream.mjpeg", { _, connection in hub.open(connection, type: "multipart/x-mixed-replace; boundary=frame") }))
        hub.http.append(("/stream.avcc", { _, connection in stream(connection, elected ?? { inner }) }))
        return hub
    }

    private func client(_ hub: Hub) -> R7DeviceClient {
        R7DeviceClient(platform: "ios", deviceId: "DUO", hostId: "local", access: { done in done(hub.origin, "ticket-d") })
    }

    /// The black cover after Closed (round 9, 3 of 6 drives): the inactive cover feed, decoded while
    /// the inner display is active, fell behind once (a stalled main thread receives a burst over the
    /// soft queue), took the MJPEG fallback a fixed panel cannot have, and never painted again. It
    /// must reopen its video and keep going.
    func testABurstOnTheInactiveCoverKeepsItsFeed() throws {
        let inner = Encoded.make(width: 1125, height: 1600, colors: Array(repeating: .systemGreen, count: 12))
        let cover = Encoded.make(width: 784, height: 1140, colors: Array(repeating: .systemPurple, count: 12))
        let hub = try burstHub(inner, cover, burst: 1, every: 33)
        let client = client(hub)
        var painted: [Int: Int] = [:], unavailable: [String] = []
        client.onPanelFrame = { panel, _ in painted[panel, default: 0] += 1 }
        client.onDuoUnavailable = { unavailable.append($0) }
        client.start()
        spin(until: { client.inputOpen })
        hub.push("/helper/ws", config(3, 180, "open"))
        spin(until: { client.screen?.duo != nil })
        client.setDuoPanels(true)
        spin(until: { painted[3, default: 0] > 0 })
        XCTAssertGreaterThan(painted[3, default: 0], 0, "the inner feed paints the open Duo")
        // One stall: about 15 samples reach each feed at once while the cover is inactive.
        Thread.sleep(forTimeInterval: 0.5)
        RunLoop.main.run(until: Date().addingTimeInterval(1.5))
        let recovered = client.panelClients.map(\.recovered)
        XCTAssertTrue(recovered.contains("queue"), "the bursts overflow the soft queue (\(recovered)): this test must exercise the recovery")
        XCTAssertFalse(client.panelClients.contains { $0.mjpeg }, "a fixed panel never takes the MJPEG fallback it cannot show")
        hub.push("/helper/ws", config(1, 0, "closed"))
        spin(until: { painted[1, default: 0] > 0 }, timeout: 5)
        XCTAssertGreaterThan(painted[1, default: 0], 0, "the cover paints after Closed")
        XCTAssertEqual(unavailable, [], "no feed gave up")
        // And keeps painting.
        let before = painted[1, default: 0]
        RunLoop.main.run(until: Date().addingTimeInterval(1.2))
        XCTAssertGreaterThan(painted[1, default: 0], before, "the cover keeps streaming")
        client.stop()
    }

    /// The same, run many times under a busy main thread (as the parity drives load it): the cover
    /// paints after Closed every time.
    func testCoverPaintsAfterClosedUnderLoad() throws {
        let inner = Encoded.make(width: 1125, height: 1600, colors: Array(repeating: .systemGreen, count: 12))
        let cover = Encoded.make(width: 784, height: 1140, colors: Array(repeating: .systemPurple, count: 12))
        let hub = try burstHub(inner, cover, burst: 1, every: 33)
        var successes = 0
        for _ in 0..<10 {
            let client = client(hub)
            var painted: [Int: Int] = [:]
            client.onPanelFrame = { panel, _ in painted[panel, default: 0] += 1 }
            client.start()
            spin(until: { client.inputOpen })
            hub.push("/helper/ws", config(3, 180, "open"))
            spin(until: { client.screen?.duo?.screenId == 3 })
            client.setDuoPanels(true)
            spin(until: { painted[3, default: 0] > 0 })
            // Block the main thread as a scene build or a layout pass does: samples pile up.
            for _ in 0..<3 { Thread.sleep(forTimeInterval: 0.45); RunLoop.main.run(until: Date().addingTimeInterval(0.05)) }
            hub.push("/helper/ws", config(1, 0, "closed"))
            spin(until: { painted[1, default: 0] > 2 }, timeout: 5)
            if painted[1, default: 0] > 2 { successes += 1 }
            client.stop()
            hub.push("/helper/ws", config(3, 180, "open"))
        }
        XCTAssertEqual(successes, 10, "the cover painted after Closed in \(successes) of 10 runs")
    }

    // MARK: Physical hand-off (supportsPhysicalOrientation)

    /// stream.ts startDuoVideo: a hub that hands the surface over physically gets one elected feed
    /// (the device's own `stream.avcc`, video only), never the fixed panels; a new election
    /// (screenId changes) reopens only that video; `physical` commands are sent only to such a hub.
    func testPhysicalHubUsesOneElectedFeed() throws {
        let inner = Encoded.make(width: 1125, height: 1600, colors: Array(repeating: .systemGreen, count: 6))
        let cover = Encoded.make(width: 784, height: 1140, colors: Array(repeating: .systemPurple, count: 6))
        var screenId = 3
        let lock = NSLock()
        let hub = try burstHub(inner, cover, burst: 1, every: 33, elected: { lock.lock(); defer { lock.unlock() }; return screenId == 1 ? cover : inner })
        let client = client(hub)
        var painted: [(Int, Int, Int)] = []
        client.onPanelFrame = { panel, image in painted.append((panel, image.width, image.height)) }
        client.start()
        spin(until: { client.inputOpen })
        // Without physical orientation, a physical command is refused locally.
        hub.push("/helper/ws", config(3, 90, "book"))
        spin(until: { client.screen?.duo?.hingePose == "book" })
        client.controlDuo(.physical("facedown"))
        RunLoop.main.run(until: Date().addingTimeInterval(0.3))
        XCTAssertFalse(hub.received("/helper/ws").contains { $0.first == 0x10 }, "no physical command reaches a hub without physical orientation")
        XCTAssertEqual(client.duoControl.state.error, "Device is disconnected.", "stream.ts send refuses it; createDuoControl reports it")
        hub.push("/helper/ws", config(3, 90, "book", physical: true))
        spin(until: { client.screen?.duo?.supportsPhysicalOrientation == true })
        let before = hub.requests.count
        client.setDuoPanels(true)
        spin(until: { painted.contains { $0.0 == 3 } })
        XCTAssertEqual(client.panelClients.map { $0.panelId }, [0], "one elected feed")
        let opened = hub.requests[before...].filter { $0.contains("stream.avcc") }
        XCTAssertEqual(opened.count, 1, "\(opened)")
        XCTAssertFalse(opened.contains { $0.contains("/panel/") }, "no fixed panel feed")
        XCTAssertTrue(painted.allSatisfy { $0 == (3, 1124, 1600) }, "the inner surface owns the elected feed")
        // Facedown: a 0x10 physical transaction; the hub elects the cover and the client reopens video.
        client.controlDuo(.physical("facedown"))
        spin(until: { hub.received("/helper/ws").contains { $0.first == 0x10 } })
        let command = try XCTUnwrap(hub.received("/helper/ws").last { $0.first == 0x10 })
        let sent = json(command.dropFirst())
        XCTAssertEqual(sent["command"] as? NSDictionary, ["control": "physical", "value": "facedown"])
        let requestId = try XCTUnwrap(sent["requestId"] as? Int)
        let reopened = hub.requests.count
        lock.lock(); screenId = 1; lock.unlock()
        hub.push("/helper/ws", R6DeviceWire.tagged(0x90, ["requestId": requestId, "ok": true]))
        hub.push("/helper/ws", config(1, 90, "book", physical: true))
        spin(until: { painted.contains { $0.0 == 1 } })
        XCTAssertTrue(painted.contains { $0 == (1, 784, 1140) }, "the cover paints from the newly elected feed")
        XCTAssertEqual(hub.requests[reopened...].filter { $0.contains("stream.avcc") }.count, 1, "the election reopened only the video")
        XCTAssertEqual(client.panelClients.map { $0.panelId }, [0])
        XCTAssertFalse(client.duoControl.state.pending)
        client.stop()
    }

    private func modelHub(_ hub: Hub) throws {
        guard let dir = ProcessInfo.processInfo.environment["T3_DEVICE_GLB_DIR"] else { throw XCTSkip("T3_DEVICE_GLB_DIR is not set") }
        let files = try FileManager.default.contentsOfDirectory(atPath: dir)
        let chat = try XCTUnwrap(files.first { $0.hasPrefix("_chat-") && $0.hasSuffix(".js") })
        hub.http.insert(("/assets/", { path, connection in
            let name = String(path.split(separator: "/").last ?? "")
            guard let data = FileManager.default.contents(atPath: dir + "/" + name) else { return hub.respond(connection, status: "404 Not Found", body: Data()) }
            hub.respond(connection, type: name.hasSuffix(".js") ? "text/javascript" : "model/gltf-binary", body: data)
        }), at: 0)
        hub.http.insert(("=/", { _, connection in hub.respond(connection, type: "text/html", body: Data("<script type=module src=\"/assets/\(chat)\"></script>".utf8)) }), at: 0)
    }

    private func window(_ view: NSView) -> NSWindow {
        let window = NSWindow(contentRect: NSRect(x: 80, y: 80, width: 420, height: 620), styleMask: [.borderless], backing: .buffered, defer: false)
        view.frame = window.contentView!.bounds
        view.autoresizingMask = [.width, .height]
        window.contentView!.addSubview(view)
        window.orderFrontRegardless()
        return window
    }

    /// duoViewer.ts orbit `choose`: with physical orientation, turning the device to face the other
    /// display asks the hub for that display (`facedown` for the cover, `faceup` for the inner pair)
    /// and holds touches until the election; a refusal or 5 s without one returns to the active display.
    func testOrbitRequestsTheOtherDisplay() throws {
        let hub = try Hub()
        try modelHub(hub)
        var requested: [Int] = []
        let view = R9DuoView(touch: { _, _, _ in }, unavailable: { XCTFail("Duo 3D unavailable") }, panelRequested: { requested.append($0) },
                             orientationRequested: { _ in }, hinge: { _ in })
        let window = window(view)
        let book = R7DeviceClient.Screen(width: 1125, height: 1600, orientation: "portrait", duo: R9DuoConfig(screenId: 3, supportsHingeAngle: true, supportsPhysicalOrientation: true, hingeAngle: 90, hingePose: "book"))
        view.setScreen(book)
        view.origin = hub.origin
        spin(until: { view.isLoaded }, timeout: 30)
        XCTAssertTrue(view.isLoaded)
        RunLoop.main.run(until: Date().addingTimeInterval(1.5))
        // Drag outside the device by half a turn: the nearest rest view is now the cover's.
        func drag(_ dx: CGFloat) {
            let start = NSPoint(x: 8, y: 8)
            view.mouseDown(with: mouseEvent(.leftMouseDown, at: start, in: view))
            for step in 1...12 { view.mouseDragged(with: mouseEvent(.leftMouseDragged, at: NSPoint(x: start.x + dx * CGFloat(step) / 12, y: start.y), in: view)) }
            view.mouseUp(with: mouseEvent(.leftMouseUp, at: NSPoint(x: start.x + dx, y: start.y), in: view))
        }
        drag(.pi / 0.006)
        XCTAssertEqual(requested, [1], "facing the cover asks for it")
        XCTAssertNil(view.screenPoint(0.5, 0.5, captured: false), "no contact while the hand-off is pending")
        // The hub elects the cover: the hand-off is confirmed and touches return once it settles.
        view.setScreen(.init(width: 784, height: 1140, orientation: "portrait", duo: R9DuoConfig(screenId: 1, supportsHingeAngle: true, supportsPhysicalOrientation: true, hingeAngle: 90, hingePose: "book")))
        // And back: facing the inner pair asks for it (faceup).
        RunLoop.main.run(until: Date().addingTimeInterval(1.5))
        drag(.pi / 0.006)
        XCTAssertEqual(requested, [1, 3], "facing the inner displays asks for them")
        // A refused command (the control queue's error) returns to the active display at once.
        view.rejectOrientation()
        RunLoop.main.run(until: Date().addingTimeInterval(1.5))
        drag(.pi / 0.006)
        XCTAssertEqual(requested, [1, 3, 3], "after a refusal the next orbit asks again")
        // Without physical orientation the same turn never asks.
        view.setScreen(.init(width: 1125, height: 1600, orientation: "portrait", duo: R9DuoConfig(screenId: 3, supportsHingeAngle: true, supportsPhysicalOrientation: false, hingeAngle: 90, hingePose: "book")))
        RunLoop.main.run(until: Date().addingTimeInterval(1.5))
        drag(.pi / 0.006)
        XCTAssertEqual(requested, [1, 3, 3], "a hub without physical orientation is never asked")
        view.dispose()
        window.orderOut(nil)
    }
}

_ = NSApplication.shared
NSApp.setActivationPolicy(.accessory)
let suite = XCTestSuite(forTestCaseClass: R10DeviceTests.self)
suite.run()
let run = suite.testRun!
print("Executed \(run.executionCount) tests, with \(run.totalFailureCount) failures")
exit(run.totalFailureCount == 0 ? 0 : 1)
