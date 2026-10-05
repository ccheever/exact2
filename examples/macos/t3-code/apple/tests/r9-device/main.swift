import AppKit
import SceneKit
import XCTest
import simd
import Network

// Lane r9-device: the iPhone Duo's hinge protocol, panel feeds and 3D viewer, and an Android
// foldable's fold reads and procedural body, against the loopback hub in hub.swift with synthetic
// frames (no simulator, emulator or device tools; the device hub is not enabled here).
// `T3_DEVICE_TEST_DIR` receives the renders; `T3_DEVICE_GLB_DIR` (a T3 server's built client assets)
// adds the served iPhone Duo model.
final class R9DeviceTests: XCTestCase {
    private func spin(until condition: () -> Bool, timeout: TimeInterval = 10) {
        let end = Date().addingTimeInterval(timeout)
        while !condition() && Date() < end { RunLoop.main.run(until: Date().addingTimeInterval(0.02)) }
    }
    private func window(_ view: NSView, width: CGFloat = 420, height: CGFloat = 620, dark: Bool = false) -> NSWindow {
        let window = NSWindow(contentRect: NSRect(x: 80, y: 80, width: width, height: height), styleMask: [.borderless], backing: .buffered, defer: false)
        window.appearance = NSAppearance(named: dark ? .darkAqua : .aqua)
        window.contentView!.wantsLayer = true
        window.contentView!.layer?.backgroundColor = (dark ? NSColor(srgbRed: 0x0a / 255, green: 0x0a / 255, blue: 0x0a / 255, alpha: 1) : NSColor(srgbRed: 0xfc / 255, green: 0xfc / 255, blue: 0xfc / 255, alpha: 1)).cgColor
        view.frame = window.contentView!.bounds
        view.autoresizingMask = [.width, .height]
        window.contentView!.addSubview(view)
        window.orderFrontRegardless()
        return window
    }
    private func save(_ window: NSWindow, _ name: String) {
        guard let dir = ProcessInfo.processInfo.environment["T3_DEVICE_TEST_DIR"] else { return }
        RunLoop.main.run(until: Date().addingTimeInterval(0.4))
        guard let image = CGWindowListCreateImageForTest(window.windowNumber), let space = CGColorSpace(name: CGColorSpace.sRGB),
              let context = CGContext(data: nil, width: image.width, height: image.height, bitsPerComponent: 8, bytesPerRow: 0, space: space, bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue) else { return }
        context.draw(image, in: CGRect(x: 0, y: 0, width: image.width, height: image.height))
        try? NSBitmapImageRep(cgImage: context.makeImage()!).representation(using: .png, properties: [:])?.write(to: URL(fileURLWithPath: dir).appendingPathComponent(name))
    }
    private func json(_ data: Data) -> [String: Any] { (try? JSONSerialization.jsonObject(with: data) as? [String: Any]) ?? [:] }
    private func same(_ a: simd_quatd, _ b: [Double], _ message: String) {
        let q = SIMD4(a.imag.x, a.imag.y, a.imag.z, a.real), r = SIMD4(b[0], b[1], b[2], b[3])
        XCTAssertLessThan(min(simd_length(q - r), simd_length(q + r)), 1e-4, "\(message): \(q) vs \(r)")
    }

    // MARK: Protocol

    func testScreenConfigAndCommands() {
        guard case .success(let none) = R9DuoConfig.parse(["width": 1, "height": 1, "orientation": "portrait"]) else { return XCTFail("plain config") }
        XCTAssertNil(none, "a phone's config carries no hinge")
        guard case .success(let duo?) = R9DuoConfig.parse(["screenId": 3, "supportsHingeAngle": true, "hingeAngle": 90.5, "hingePose": NSNull()]) else { return XCTFail("duo config") }
        XCTAssertEqual(duo, R9DuoConfig(screenId: 3, supportsHingeAngle: true, supportsPhysicalOrientation: false, hingeAngle: 90.5, hingePose: nil))
        guard case .success(let pose?) = R9DuoConfig.parse(["screenId": 1, "hingePose": "laptop", "tableMode": false]) else { return XCTFail("pose") }
        XCTAssertEqual(pose.hingePose, "laptop"); XCTAssertEqual(pose.tableMode, false)
        for bad: [String: Any] in [["hingeAngle": 181], ["hingeAngle": -1], ["hingePose": "flat"], ["supportsHingeAngle": 1], ["screenId": "3"]] {
            if case .success = R9DuoConfig.parse(bad) { XCTFail("rejects \(bad)") }
        }
        XCTAssertEqual(R9DuoCommand.parse("pose:book"), .pose("book"))
        XCTAssertEqual(R9DuoCommand.parse("angle:120"), .angle(120))
        XCTAssertEqual(R9DuoCommand.parse("orientation:landscape_left"), .orientation("landscape_left"))
        XCTAssertNil(R9DuoCommand.parse("pose:flat")); XCTAssertNil(R9DuoCommand.parse("pose"))
        XCTAssertEqual(R9DuoCommand.pose("tent").json as NSDictionary, ["control": "pose", "value": "tent"])
        XCTAssertEqual(R9Duo.rawPoint(panel: 1, x: 0.2, y: 0.7), CGPoint(x: 0.2, y: 0.7))
        XCTAssertEqual(R9Duo.rawPoint(panel: 3, x: 0.2, y: 0.7), CGPoint(x: 0.7, y: 0.8))
        let inner = R7DeviceClient.Screen(width: 1125, height: 1600, orientation: "portrait", duo: duo)
        XCTAssertTrue(R9Duo.frameMatches(width: 1125, height: 1600, screen: inner))
        XCTAssertFalse(R9Duo.frameMatches(width: 784, height: 1140, screen: inner))
        XCTAssertEqual(R9Duo.displayKey(inner), "3:1125:1600:portrait")
        XCTAssertEqual(R9Duo.displayKey(nil), "")
    }

    /// duoControl.test.ts, case for case.
    func testControlQueue() {
        var sent: [(Int, R9DuoCommand)] = [], states: [R9DuoControlState] = [], accept = false
        var queue = R9DuoControl(send: { id, command in sent.append((id, command)); return accept }, onChange: { states.append($0) })
        queue.enqueue(.angle(40))
        XCTAssertEqual(states.last, R9DuoControlState(pending: false, requested: nil, error: "Device is disconnected."))
        accept = true
        queue = R9DuoControl(send: { id, command in sent.append((id, command)); return accept }, onChange: { states.append($0) })
        sent = []
        queue.enqueue(.angle(40)); queue.enqueue(.angle(50)); queue.enqueue(.angle(60))
        XCTAssertEqual(sent.count, 1, "hinge edits wait behind the acknowledgement")
        XCTAssertEqual(states.last, R9DuoControlState(pending: true, requested: .angle(60), error: nil))
        queue.receive(requestId: 1, ok: true, error: nil)
        XCTAssertEqual(sent.last?.0, 2); XCTAssertEqual(sent.last?.1, .angle(60))
        queue.enqueue(.angle(100)); queue.enqueue(.pose("tent"))
        queue.receive(requestId: 2, ok: true, error: nil)
        XCTAssertEqual(sent.last?.1, .pose("tent"), "a preset replaces queued motion")
        queue.receive(requestId: 3, ok: true, error: nil)
        XCTAssertEqual(states.last, R9DuoControlState(pending: false, requested: nil, error: nil))
        // Failure, timeout and late replies.
        queue = R9DuoControl(timeout: 0.1, send: { id, command in sent.append((id, command)); return true }, onChange: { states.append($0) })
        sent = []
        queue.enqueue(.angle(90)); queue.enqueue(.angle(100))
        spin(until: { states.last?.error != nil }, timeout: 2)
        XCTAssertTrue(states.last?.error?.contains("timed out") == true)
        queue.enqueue(.pose("open"))
        queue.receive(requestId: 1, ok: true, error: nil)
        XCTAssertEqual(states.last?.pending, true, "a late reply cannot acknowledge later work")
        queue.enqueue(.angle(130))
        queue.receive(requestId: 2, ok: false, error: "native refused")
        XCTAssertEqual(states.last, R9DuoControlState(pending: false, requested: nil, error: "native refused"))
        queue.enqueue(.pose("book")); queue.enqueue(.pose("closed")); queue.clear()
        queue.receive(requestId: 3, ok: true, error: nil)
        XCTAssertEqual(sent.count, 3)
        queue.enqueue(.angle(.infinity)); queue.enqueue(.angle(181))
        XCTAssertEqual(sent.count, 3, "an impossible angle is never sent")
        queue.clear()
    }

    func testPinch() {
        var confirmed = 90.0, changes: [Double?] = []
        let pinch = R9DuoPinch(angle: { confirmed }, contains: { x, y in x > 0.2 && y > 0.2 }, change: { changes.append($0) })
        XCTAssertFalse(pinch.begin(0.1, 0.5)); pinch.move(1); XCTAssertTrue(changes.isEmpty)
        XCTAssertTrue(pinch.begin(0.5, 0.5))
        pinch.move(0.25); XCTAssertEqual(changes.last, 120)
        confirmed = 100
        pinch.move(0.25); XCTAssertEqual(changes.last, 150, "accumulates independently of native readback")
        pinch.move(2); XCTAssertEqual(changes.last, 180)
        pinch.move(-3); XCTAssertEqual(changes.last, 0)
        pinch.move(.nan); pinch.end()
        XCTAssertEqual(changes.last, .some(nil))
        let count = changes.count
        pinch.move(1); pinch.end()
        XCTAssertEqual(changes.count, count); XCTAssertFalse(pinch.active)
    }

    /// duoViewSnaps / nearestDeviceView against the reference's own numbers (three.js 0.180, duoSnap.ts).
    func testViewSnapsMatchTheReference() {
        let s = sin(Double.pi / 4), c = cos(Double.pi / 4)
        let cover = R9DuoSnaps.snaps([R9DuoRestFrame(face: "cover", normal: SIMD3(0, 0, -1), up: SIMD3(0, 1, 0), center: SIMD3(-4, 0, -0.5))], panel: 1)
        XCTAssertEqual(cover.map(\.orientation), ["portrait", "landscape_left", "portrait_upside_down", "landscape_right"])
        for (snap, q) in zip(cover, [[0, -1, 0, 0], [-0.707107, -0.707107, 0, 0], [-1, 0, 0, 0], [-0.707107, 0.707107, 0, 0]]) { same(snap.rotation, q, "cover \(snap.orientation)"); XCTAssertEqual(snap.yawLimit, 0.349066, accuracy: 1e-5) }
        let book = R9DuoSnaps.snaps([R9DuoRestFrame(face: "inside", normal: SIMD3(0, 0, 1), up: SIMD3(0, 1, 0), center: SIMD3(0, 0, 1)),
                                     R9DuoRestFrame(face: "left", normal: SIMD3(s, 0, c), up: SIMD3(0, 1, 0), center: SIMD3(-3, 0, 1)),
                                     R9DuoRestFrame(face: "right", normal: SIMD3(-s, 0, c), up: SIMD3(0, 1, 0), center: SIMD3(3, 0, 1))], panel: 3)
        XCTAssertEqual(book.map { "\($0.face):\($0.orientation)" }, ["inside:portrait", "inside:landscape_left", "inside:portrait_upside_down", "inside:landscape_right", "left:portrait_upside_down", "right:portrait"])
        let expected: [([Double], Double)] = [([0, 0, 0.707107, 0.707107], 1.047198), ([0, 0, 0, 1], 0.733038), ([0, 0, -0.707107, 0.707107], 1.047198), ([0, 0, -1, 0], 0.733038),
                                              ([-0.183013, -0.183013, -0.683013, 0.683013], 1.047198), ([-0.183013, 0.183013, 0.683013, 0.683013], 1.047198)]
        for (snap, (q, yaw)) in zip(book, expected) { same(snap.rotation, q, "book \(snap.face) \(snap.orientation)"); XCTAssertEqual(snap.yawLimit, yaw, accuracy: 1e-5, "\(snap.face) \(snap.orientation)") }
        let probe = simd_quatd(angle: 0.7, axis: simd_normalize(SIMD3(0.3, 1, 0.1)))
        let near = R9DuoSnaps.nearest(probe, book)
        XCTAssertEqual(near?.orientation, "landscape_left"); same(near!.rotation, [0, 0.328702, 0, 0.944434], "nearest book")
        let nearCover = R9DuoSnaps.nearest(probe, cover)
        same(nearCover!.rotation, [0, -0.984808, 0, -0.173648], "nearest cover")
    }

    func testPanelPaint() throws {
        // A portrait inner framebuffer drawn a quarter turn clockwise into the 1600 × 1125 surface:
        // its top (red) lands on the right, its bottom (blue) on the left.
        let context = CGContext(data: nil, width: 90, height: 128, bitsPerComponent: 8, bytesPerRow: 0, space: CGColorSpace(name: CGColorSpace.sRGB)!, bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue)!
        context.setFillColor(NSColor.systemBlue.cgColor); context.fill(CGRect(x: 0, y: 0, width: 90, height: 64))
        context.setFillColor(NSColor.systemRed.cgColor); context.fill(CGRect(x: 0, y: 64, width: 90, height: 64))
        let inner = try XCTUnwrap(R9Duo.paint(context.makeImage()!, panel: 3))
        XCTAssertEqual([inner.width, inner.height], [1600, 1125])
        XCTAssertGreaterThan(pixel(inner, x: 0.8, y: 0.5).r, 180); XCTAssertGreaterThan(pixel(inner, x: 0.2, y: 0.5).b, 180)
        let cover = try XCTUnwrap(R9Duo.paint(context.makeImage()!, panel: 1))
        XCTAssertEqual([cover.width, cover.height], [784, 1140])
        XCTAssertGreaterThan(pixel(cover, x: 0.5, y: 0.2).r, 180, "the cover keeps its framebuffer upright")
        XCTAssertTrue(R9Duo.isBlank(R9DeviceTests.solid(width: 32, height: 32, color: .black)), "a native shutdown frame")
        XCTAssertFalse(R9Duo.isBlank(R9Duo.blank(CGSize(width: 32, height: 32))!), "the surface fill is not a shutdown frame"); XCTAssertFalse(R9Duo.isBlank(cover))
    }

    /// The Duo's panels are not macroblock-sized (784 × 1140, 1125 × 1600): their frames must come
    /// back at the framebuffer's own size, or duoFrameMatches rejects them.
    func testPanelSizedFramesDecodeAtTheirSize() {
        for (width, height) in [(784, 1140), (1125, 1600)] {
            let encoded = Encoded.make(width: width, height: height, colors: [.systemPurple, .systemPurple])
            var sizes: [String] = []
            let decoder = R7H264Decoder(onFrame: { sizes.append("\($0.width)x\($0.height)") }, onError: { XCTFail("decode") })
            XCTAssertTrue(decoder.configure(avcC: encoded.avcC))
            for sample in encoded.samples { decoder.decode(sample: sample.data, pts: 0) }
            spin(until: { sizes.count == encoded.samples.count }, timeout: 5)
            // 4:2:0 frames are even-sized: an odd framebuffer width comes back a pixel narrower, within duoFrameMatches' tolerance.
            XCTAssertEqual(Set(sizes), ["\(width / 2 * 2)x\(height)"])
            let screen = R7DeviceClient.Screen(width: Double(width), height: Double(height), orientation: "portrait")
            XCTAssertTrue(R9Duo.frameMatches(width: Double(width / 2 * 2), height: Double(height), screen: screen))
        }
    }

    // MARK: Duo over the hub

    private func duoHub(_ inner: Encoded, _ cover: Encoded, panels: Bool = true) throws -> Hub {
        let hub = try Hub()
        let jpeg = NSBitmapImageRep(cgImage: R9DeviceTests.solid(width: 90, height: 128, color: .systemOrange)).representation(using: .jpeg, properties: [:])!
        // A live feed: the description, then one sample every 33 ms, looping from the keyframe.
        func stream(_ connection: NWConnection, _ encoded: Encoded) {
            hub.open(connection, type: "application/octet-stream")
            hub.write(connection, R7H264.Demuxer.envelope(.description, encoded.avcC))
            let timer = DispatchSource.makeTimerSource(queue: hub.queue)
            var index = 0
            timer.schedule(deadline: .now(), repeating: .milliseconds(33))
            timer.setEventHandler {
                guard connection.state == .ready else { return timer.cancel() }
                let sample = encoded.samples[index % encoded.samples.count]; index += 1
                hub.write(connection, R7H264.Demuxer.envelope(sample.key ? .keyframe : .delta, sample.data))
            }
            timer.resume()
            hub.timers.append(timer)
        }
        hub.http.append(("/panel/1/stream.avcc", { _, connection in
            guard panels else { return hub.respond(connection, status: "404 Not Found", body: Data()) }
            stream(connection, cover)
        }))
        hub.http.append(("/panel/3/stream.avcc", { _, connection in
            guard panels else { return hub.respond(connection, status: "404 Not Found", body: Data()) }
            stream(connection, inner)
        }))
        hub.http.append(("/stream.mjpeg", { _, connection in
            hub.open(connection, type: "multipart/x-mixed-replace; boundary=frame")
            hub.write(connection, Data("--frame\r\nContent-Type: image/jpeg\r\nContent-Length: \(jpeg.count)\r\n\r\n".utf8) + jpeg + Data("\r\n".utf8))
        }))
        hub.http.append(("/stream.avcc", { _, connection in stream(connection, inner) }))
        return hub
    }
    static func solid(width: Int, height: Int, color: NSColor) -> CGImage {
        let context = CGContext(data: nil, width: width, height: height, bitsPerComponent: 8, bytesPerRow: 0, space: CGColorSpace(name: CGColorSpace.sRGB)!, bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue)!
        context.setFillColor(color.cgColor); context.fill(CGRect(x: 0, y: 0, width: width, height: height))
        return context.makeImage()!
    }
    private func config(_ screenId: Int, _ angle: Double, _ pose: String?, orientation: String = "portrait") -> Data {
        R6DeviceWire.tagged(0x82, ["width": screenId == 1 ? 784 : 1125, "height": screenId == 1 ? 1140 : 1600, "orientation": orientation, "screenId": screenId,
                                   "supportsHingeAngle": true, "supportsPhysicalOrientation": false, "hingeAngle": angle, "hingePose": pose ?? NSNull()])
    }

    func testDuoCommandsAndPanelFeeds() throws {
        let inner = Encoded.make(width: 1125, height: 1600, colors: [.systemGreen, .systemGreen]), cover = Encoded.make(width: 784, height: 1140, colors: [.systemPurple])
        let hub = try duoHub(inner, cover)
        let view = R6DeviceScreenView(key: "local\u{0}DUO", platform: "ios", deviceId: "DUO", hostId: "local", access: { done in done(hub.origin, "ticket-d") }) {}
        view.deviceName = "iPhone Duo"
        view.presentation = "flat"
        let window = window(view)
        view.start()
        spin(until: { view.status == "streaming" && view.inputConnected })
        XCTAssertEqual(view.phoneUnavailableReason, "iPhone Duo 3D requires Device Hub 0.11.0 or newer", "until the hub reports the hinge")
        hub.push("/helper/ws", config(3, 180, "open"))
        spin(until: { view.screen != nil && view.client.screen?.duo != nil })
        XCTAssertNil(view.phoneUnavailableReason)
        let duo = view.reportValue["duo"] as? [String: Any]
        XCTAssertEqual(duo?["supported"] as? Bool, true); XCTAssertEqual(duo?["hingeAngle"] as? Double, 180); XCTAssertEqual(duo?["hingePose"] as? String, "open")
        // A stand: one 0x10 transaction, acknowledged by 0x90.
        view.folding("duo", "pose:book")
        spin(until: { hub.received("/helper/ws").contains { $0.first == 0x10 } })
        let command = try XCTUnwrap(hub.received("/helper/ws").first { $0.first == 0x10 })
        XCTAssertEqual(json(command.dropFirst()) as NSDictionary, ["requestId": 1, "command": ["control": "pose", "value": "book"]])
        XCTAssertEqual((view.reportValue["duo"] as? [String: Any])?["pending"] as? Bool, true)
        hub.push("/helper/ws", R6DeviceWire.tagged(0x90, ["requestId": 1, "ok": true]))
        hub.push("/helper/ws", config(3, 90, "book"))
        spin(until: { (view.reportValue["duo"] as? [String: Any])?["hingePose"] as? String == "book" })
        XCTAssertEqual((view.reportValue["duo"] as? [String: Any])?["pending"] as? Bool, false)
        // Rotate goes through the queue as an orientation, acknowledged by the next config.
        view.press("rotate")
        spin(until: { hub.received("/helper/ws").contains { $0 == R6DeviceWire.tagged(0x07, ["orientation": "landscape_left"]) } })
        XCTAssertTrue(hub.received("/helper/ws").contains(R6DeviceWire.tagged(0x07, ["orientation": "landscape_left"])))
        hub.push("/helper/ws", config(3, 90, nil, orientation: "landscape_left"))
        spin(until: { (view.reportValue["duo"] as? [String: Any])?["pending"] as? Bool == false && view.screen?.orientation == "landscape_left" })
        XCTAssertEqual((view.reportValue["duo"] as? [String: Any])?["error"] as? String, "")
        // The 3D view reads the fixed panels instead of the device stream.
        view.presentation = "phone"
        spin(until: { hub.requests.contains { $0.contains("/panel/3/stream.avcc") } && hub.requests.contains { $0.contains("/panel/1/stream.avcc") } })
        XCTAssertNil(view.phone, "the Duo never takes the single-screen phone")
        // This hub serves no device model: as the reference's model slot, the viewer reports 3D
        // unavailable and the device's own stream returns (the served model is testDuoViewerRendersServedModel).
        spin(until: { view.phoneUnavailableReason != nil })
        XCTAssertEqual(view.phoneUnavailableReason, "3D is unavailable on this browser")
        XCTAssertNil(view.duoView)
        spin(until: { view.image.map { pixel($0, x: 0.5, y: 0.3).g > 150 } == true })
        XCTAssertGreaterThan(view.image.map { pixel($0, x: 0.5, y: 0.3).g } ?? 0, 150, "the flat picture keeps streaming")
        view.stop()
        window.orderOut(nil)
    }

    func testMissingPanelFeedsFallBackToFlat() throws {
        let inner = Encoded.make(width: 1125, height: 1600, colors: [.systemGreen]), cover = Encoded.make(width: 784, height: 1140, colors: [.systemPurple])
        let hub = try duoHub(inner, cover, panels: false)
        let view = R6DeviceScreenView(key: "local\u{0}DUO", platform: "ios", deviceId: "DUO", hostId: "local", access: { done in done(hub.origin, "ticket-d") }) {}
        view.deviceName = "iPhone Duo"; view.presentation = "phone"
        let window = window(view)
        view.start()
        spin(until: { view.status == "streaming" && view.inputConnected })
        hub.push("/helper/ws", config(3, 180, "open"))
        spin(until: { view.phoneUnavailableReason == "3D is unavailable on this browser" })
        XCTAssertEqual(view.phoneUnavailableReason, "3D is unavailable on this browser", "a hub without fixed panels leaves the flat stream")
        XCTAssertNil(view.duoView)
        view.stop()
        window.orderOut(nil)
    }

    // MARK: Android fold

    func testFoldState() {
        XCTAssertEqual(R9FoldState.parse(Data(#"{"ok":true,"fold":{"supported":true,"posture":"opened","hingeAngle":180}}"#.utf8)), R9FoldState(supported: true, posture: "opened", hingeAngle: 180))
        XCTAssertEqual(R9FoldState.parse(Data(#"{"ok":true,"fold":{"supported":false,"posture":null,"hingeAngle":null}}"#.utf8)), R9FoldState(supported: false, posture: nil, hingeAngle: nil))
        XCTAssertNil(R9FoldState.parse(Data(#"{"fold":{"supported":true,"posture":"opened","hingeAngle":180}}"#.utf8)), "ok must be true")
        XCTAssertNil(R9FoldState.parse(Data(#"{"ok":true,"fold":{"supported":true,"posture":"wide","hingeAngle":180}}"#.utf8)))
        XCTAssertNil(R9FoldState.parse(Data(#"{"ok":true,"fold":{"supported":"yes","posture":null,"hingeAngle":null}}"#.utf8)))
        XCTAssertEqual(R9FoldState(supported: true, posture: "closed", hingeAngle: nil).drawnAngle, 0)
        XCTAssertEqual(R9FoldState(supported: true, posture: "half_opened", hingeAngle: 95).drawnAngle, 95)
        XCTAssertNil(R9FoldState(supported: false, posture: nil, hingeAngle: nil).drawnAngle)
    }

    func testFoldReadsAndWrites() throws {
        let hub = try Hub()
        var posture = "opened", fail = false, bodies: [String] = []
        hub.http.append(("/vendor/serve-emu/api/fold", { path, connection in
            if fail { return hub.respond(connection, status: "500 Internal Server Error", body: Data(#"{"ok":false,"error":"Emulator refused the posture."}"#.utf8)) }
            hub.respond(connection, body: Data(#"{"ok":true,"fold":{"supported":true,"posture":"\#(posture)","hingeAngle":\#(posture == "closed" ? 0 : 180)}}"#.utf8))
        }))
        let reader = R9DeviceFold(deviceId: "emulator-5556", hostId: "local", access: { done in done(hub.origin, "ticket-f") }) {}
        reader.update(visible: true, enabled: true, width: 2076, height: 2152)
        spin(until: { reader.fold != nil })
        XCTAssertEqual(reader.fold, R9FoldState(supported: true, posture: "opened", hingeAngle: 180))
        XCTAssertEqual(reader.angle, 180)
        XCTAssertTrue(hub.requests.contains { $0.hasPrefix("GET /api/device-hub/vendor/serve-emu/api/fold?device=emulator-5556&wsTicket=ticket-f&hostId=local ") }, "\(hub.requests)")
        posture = "closed"
        reader.set("closed", enabled: true)
        XCTAssertEqual(reader.angle, 0, "the hinge turns at once"); XCTAssertTrue(reader.pending)
        spin(until: { !reader.pending })
        XCTAssertEqual(reader.fold?.posture, "closed"); XCTAssertEqual(reader.angle, 0)
        XCTAssertTrue(hub.requests.contains { $0.hasPrefix("POST /api/device-hub/vendor/serve-emu/api/fold?device=emulator-5556") })
        fail = true
        reader.set("opened", enabled: true)
        XCTAssertEqual(reader.angle, 180)
        spin(until: { !reader.pending })
        XCTAssertEqual(reader.error, "Emulator refused the posture."); XCTAssertEqual(reader.angle, 0, "a failed command returns the hinge")
        XCTAssertEqual(reader.report["error"] as? String, "Emulator refused the posture.")
        reader.set("opened", enabled: false)
        XCTAssertFalse(reader.pending, "disabled while the stream is not live")
        _ = bodies
        reader.stop()
    }

    /// The procedural foldable in the 3D phone, open, half open and shut, with touches on its screens.
    func testFoldSceneRendersAndMapsTouches() {
        for dark in [false, true] {
            let phone = R7DevicePhoneView(platform: "android", name: "Pixel 9 Pro Fold", touch: { _, _, _ in }, unavailable: { XCTFail("3D unavailable") })
            let window = window(phone, dark: dark)
            phone.setScreen(.init(width: 2076, height: 2152, orientation: "portrait"))
            phone.setFrame(R9DeviceTests.split(width: 692, height: 717))
            phone.setFoldAngle(180)
            XCTAssertNotNil(phone.fold)
            RunLoop.main.run(until: Date().addingTimeInterval(0.6))
            let left = phone.screenPoint(CGPoint(x: 0.4, y: 0.5), captured: false), right = phone.screenPoint(CGPoint(x: 0.6, y: 0.5), captured: false)
            XCTAssertNotNil(left); XCTAssertNotNil(right)
            XCTAssertLessThan(left?.x ?? 1, 0.5); XCTAssertGreaterThan(right?.x ?? 0, 0.5)
            XCTAssertEqual(left?.y ?? 0, 0.5, accuracy: 0.03)
            save(window, "r9-fold-open-\(dark ? "dark" : "light").png")
            phone.setFoldAngle(0)
            RunLoop.main.run(until: Date().addingTimeInterval(1.2))
            XCTAssertEqual(phone.fold?.angle ?? -1, 0, accuracy: 0.01, "the hinge turns over 850 ms")
            phone.setScreen(.init(width: 1080, height: 2400, orientation: "portrait"))
            phone.setFrame(R9DeviceTests.split(width: 360, height: 800))
            RunLoop.main.run(until: Date().addingTimeInterval(0.6))
            save(window, "r9-fold-closed-\(dark ? "dark" : "light").png")
            phone.setFoldAngle(nil)
            XCTAssertNil(phone.fold, "a device that stops folding returns to the plain body")
            phone.dispose()
            window.orderOut(nil)
        }
    }

    /// Left half green, right half blue, with a white bar near the top.
    static func split(width: Int, height: Int) -> CGImage {
        let context = CGContext(data: nil, width: width, height: height, bitsPerComponent: 8, bytesPerRow: 0, space: CGColorSpace(name: CGColorSpace.sRGB)!, bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue)!
        context.setFillColor(NSColor(srgbRed: 52 / 255, green: 199 / 255, blue: 89 / 255, alpha: 1).cgColor); context.fill(CGRect(x: 0, y: 0, width: width / 2, height: height))
        context.setFillColor(NSColor(srgbRed: 0, green: 122 / 255, blue: 1, alpha: 1).cgColor); context.fill(CGRect(x: width / 2, y: 0, width: width - width / 2, height: height))
        context.setFillColor(NSColor.white.cgColor); context.fill(CGRect(x: Double(width) / 4, y: Double(height) * 0.75, width: Double(width) / 2, height: Double(height) / 10))
        return context.makeImage()!
    }

    // MARK: The served iPhone Duo model

    private func assetHub(_ hub: Hub) throws {
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

    /// The whole path: the hub's config, the panel feeds, the served model, a stand that shuts the Duo
    /// and the cover's own feed taking over.
    func testDuoPanelsReachTheViewer() throws {
        let inner = Encoded.make(width: 1125, height: 1600, colors: Array(repeating: .systemGreen, count: 20)), cover = Encoded.make(width: 784, height: 1140, colors: Array(repeating: .systemPurple, count: 20))
        let hub = try duoHub(inner, cover)
        try assetHub(hub)
        let view = R6DeviceScreenView(key: "local\u{0}DUO", platform: "ios", deviceId: "DUO", hostId: "local", access: { done in done(hub.origin, "ticket-d") }) {}
        view.deviceName = "iPhone Duo"; view.presentation = "phone"
        let window = window(view)
        view.start()
        spin(until: { view.status == "streaming" && view.inputConnected })
        hub.push("/helper/ws", config(3, 180, "open"))
        spin(until: { view.duoView?.isLoaded == true }, timeout: 30)
        XCTAssertEqual(view.duoView?.isLoaded, true, "\(String(describing: view.phoneUnavailableReason)) \(view.status) \(view.detail) \(hub.requests.filter { $0.contains("assets") || $0.hasPrefix("GET / ") })")
        spin(until: { view.duoView?.readyDisplay == "3:1125:1600:portrait" })
        XCTAssertEqual(view.duoView?.readyDisplay, "3:1125:1600:portrait", "the inner panel's feed paints the inner display")
        save(window, "r9-duo-flow-open.png")
        hub.push("/helper/ws", config(1, 0, "closed"))
        spin(until: { view.duoView?.readyDisplay == "1:784:1140:portrait" }, timeout: 15)
        XCTAssertEqual(view.duoView?.readyDisplay, "1:784:1140:portrait", "the cover's feed paints the cover once it is the active display")
        RunLoop.main.run(until: Date().addingTimeInterval(1))
        save(window, "r9-duo-flow-closed.png")
        view.stop()
        window.orderOut(nil)
    }

    func testDuoViewerRendersServedModel() throws {
        guard let dir = ProcessInfo.processInfo.environment["T3_DEVICE_GLB_DIR"] else { throw XCTSkip("T3_DEVICE_GLB_DIR is not set") }
        let hub = try Hub()
        let files = try FileManager.default.contentsOfDirectory(atPath: dir)
        let chat = try XCTUnwrap(files.first { $0.hasPrefix("_chat-") && $0.hasSuffix(".js") })
        hub.http.append(("/assets/", { path, connection in
            let name = String(path.split(separator: "/").last ?? "")
            guard let data = FileManager.default.contents(atPath: dir + "/" + name) else { return hub.respond(connection, status: "404 Not Found", body: Data()) }
            hub.respond(connection, type: name.hasSuffix(".js") ? "text/javascript" : "model/gltf-binary", body: data)
        }))
        hub.http.insert(("=/", { _, connection in hub.respond(connection, type: "text/html", body: Data("<script type=module src=\"/assets/\(chat)\"></script>".utf8)) }), at: 0)
        let innerFrame = R9DeviceTests.split(width: 1125, height: 1600), coverFrame = R9DeviceTests.solid(width: 784, height: 1140, color: NSColor(srgbRed: 1, green: 149 / 255, blue: 0, alpha: 1))
        // Each rest pose on a fresh viewer (as refduo.mjs renders the reference's), then the hinge moving on one.
        let poses: [(String, Int, Double, Double, Double)] = [("open", 3, 180, 1125, 1600), ("book", 3, 90, 1125, 1600), ("closed", 1, 0, 784, 1140)]
        for dark in [false, true] {
            for (pose, panel, angle, width, height) in poses {
                var orientations: [String] = []
                let view = R9DuoView(touch: { _, _, _ in }, unavailable: { XCTFail("Duo 3D unavailable") }, panelRequested: { _ in },
                                     orientationRequested: { orientations.append($0) }, hinge: { _ in })
                let window = window(view, dark: dark)
                view.setScreen(.init(width: width, height: height, orientation: "portrait", duo: R9DuoConfig(screenId: panel, supportsHingeAngle: true, supportsPhysicalOrientation: false, hingeAngle: angle, hingePose: pose)))
                view.origin = hub.origin
                spin(until: { view.isLoaded }, timeout: 30)
                XCTAssertTrue(view.isLoaded, "the served model has the hinge rig")
                view.frameUpdated(panel, panel == 3 ? innerFrame : coverFrame)
                RunLoop.main.run(until: Date().addingTimeInterval(1.5))
                XCTAssertEqual(view.hingeAngle, angle, accuracy: 0.5, pose)
                let middle = view.screenPoint(0.5, 0.5, captured: false)
                XCTAssertNotNil(middle, "\(pose): the display faces the camera")
                if pose == "open", let middle { XCTAssertEqual(middle.x, 0.5, accuracy: 0.05); XCTAssertEqual(middle.y, 0.5, accuracy: 0.05) }
                save(window, "r9-duo-\(pose)-\(dark ? "dark" : "light").png")
                if pose == "open" {
                    // A preview from a pinch or a queued angle command moves the hinge before the hub confirms it.
                    view.setControlPreview(120)
                    XCTAssertEqual(view.hingeAngle, 120)
                    XCTAssertNil(view.screenPoint(0.5, 0.5, captured: false), "no contact while the hinge is held")
                    view.setControlPreview(nil)
                    RunLoop.main.run(until: Date().addingTimeInterval(1.2))
                    XCTAssertEqual(view.hingeAngle, 180, accuracy: 0.5, "back to the confirmed angle")
                }
                _ = orientations
                view.dispose()
                window.orderOut(nil)
            }
        }
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
let suite = XCTestSuite(forTestCaseClass: R9DeviceTests.self)
suite.run()
let run = suite.testRun!
print("Executed \(run.executionCount) tests, with \(run.totalFailureCount) failures")
exit(run.totalFailureCount == 0 ? 0 : 1)
