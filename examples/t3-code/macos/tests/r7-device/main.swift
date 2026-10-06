import AppKit
import SceneKit
import XCTest

// Lane r7-device: Android's H.264 socket, iOS's AVCC body (and its MJPEG fallback), keyboard
// forwarding, the 3D phone and the Tools drawer's hub feeds, against the loopback hub in hub.swift
// with frames VideoToolbox encodes here (no emulator, simulator or device tools are involved).
// `T3_DEVICE_TEST_DIR` receives the rendered screens; `T3_DEVICE_GLB_DIR` (a directory holding a
// T3 server's `assets/*.glb` and `_chat-*.js`) adds the model-backed 3D renders.
final class R7DeviceTests: XCTestCase {
    private func spin(until condition: () -> Bool, timeout: TimeInterval = 10) {
        let end = Date().addingTimeInterval(timeout)
        while !condition() && Date() < end { RunLoop.main.run(until: Date().addingTimeInterval(0.02)) }
    }
    private func window(_ view: NSView, width: CGFloat = 360, height: CGFloat = 640, dark: Bool = false) -> NSWindow {
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
        // The window server hands back display colours; pair them with the browser's sRGB.
        guard let image = CGWindowListCreateImageForTest(window.windowNumber), let space = CGColorSpace(name: CGColorSpace.sRGB),
              let context = CGContext(data: nil, width: image.width, height: image.height, bitsPerComponent: 8, bytesPerRow: 0, space: space, bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue) else { return }
        context.draw(image, in: CGRect(x: 0, y: 0, width: image.width, height: image.height))
        try? NSBitmapImageRep(cgImage: context.makeImage()!).representation(using: .png, properties: [:])?.write(to: URL(fileURLWithPath: dir).appendingPathComponent(name))
    }
    private func json(_ data: Data) -> [String: Any] { (try? JSONSerialization.jsonObject(with: data) as? [String: Any]) ?? [:] }

    func testH264Wire() {
        // SEMU: magic, version 1, key flag, 64-bit pts, then Annex B.
        let packet = R7H264.semu(Data([0, 0, 0, 1, 0x65, 1, 2]), key: true, pts: 0x1_0000_0002)
        XCTAssertEqual(packet.count, 23)
        XCTAssertEqual(R7H264.parseSemu(packet), R7H264.Semu(data: Data([0, 0, 0, 1, 0x65, 1, 2]), isKey: true, timestamp: 0x1_0000_0002))
        XCTAssertEqual(R7H264.parseSemu(Data([0, 0, 1, 0x41])), R7H264.Semu(data: Data([0, 0, 1, 0x41]), isKey: nil, timestamp: nil), "unframed payloads pass through")
        // scanAccessUnit: 3- and 4-byte start codes, SPS, PPS and IDR.
        let unit = Data([0, 0, 0, 1, 0x67, 0x42, 0xc0, 0x1e, 0, 0, 1, 0x68, 0xce, 0, 0, 0, 1, 0x65, 0x88, 0x84])
        let scanned = R7H264.scan(unit)
        XCTAssertTrue(scanned.isKey)
        XCTAssertEqual(scanned.sps, Data([0x67, 0x42, 0xc0, 0x1e]))
        XCTAssertEqual(scanned.pps, Data([0x68, 0xce]))
        XCTAssertFalse(R7H264.scan(Data([0, 0, 0, 1, 0x41, 0x9a])).isKey)
        XCTAssertEqual(R7H264.avccSample(annexB: unit), Data([0, 0, 0, 3, 0x65, 0x88, 0x84]), "parameter sets move to the format description")
        XCTAssertEqual(R7H264.codecString(Data([0x67, 0x42, 0xc0, 0x1e])), "avc1.42c01e")
        let record = R7H264.avcC(sps: Data([0x67, 0x64, 0x00, 0x28, 1]), pps: Data([0x68, 0xee]))
        XCTAssertEqual(R7H264.codecString(record), "avc1.640028")
        let sets = R7H264.parameterSets(avcC: record)
        XCTAssertEqual(sets?.sps, [Data([0x67, 0x64, 0x00, 0x28, 1])]); XCTAssertEqual(sets?.pps, [Data([0x68, 0xee])]); XCTAssertEqual(sets?.nalLength, 4)
        XCTAssertNil(R7H264.parameterSets(avcC: Data([1, 2, 3])))
        // AvccDemuxer: envelopes split across arbitrary reads; unknown tags are skipped.
        let body = R7H264.Demuxer.envelope(.description, record) + Data([0, 0, 0, 2, 9, 9]) + R7H264.Demuxer.envelope(.keyframe, Data([1, 2, 3])) + R7H264.Demuxer.envelope(.delta, Data([4]))
        let demuxer = R7H264.Demuxer()
        var kinds: [R7H264.Demuxer.Kind] = [], payloads: [Data] = []
        var rest = body
        while !rest.isEmpty { for chunk in demuxer.push(rest.prefix(3)) { kinds.append(chunk.kind); payloads.append(chunk.payload) }; rest = rest.dropFirst(3) }
        XCTAssertEqual(kinds, [.description, .keyframe, .delta])
        XCTAssertEqual(payloads, [record, Data([1, 2, 3]), Data([4])])
    }

    func testKeyForwarding() {
        // HID usages (iOS) follow the physical key.
        XCTAssertEqual(R7DeviceKeys.usage(code: "KeyA"), 0x04); XCTAssertEqual(R7DeviceKeys.usage(code: "KeyZ"), 0x1d)
        XCTAssertEqual(R7DeviceKeys.usage(code: "Digit1"), 0x1e); XCTAssertEqual(R7DeviceKeys.usage(code: "Digit9"), 0x26); XCTAssertEqual(R7DeviceKeys.usage(code: "Digit0"), 0x27)
        XCTAssertEqual(R7DeviceKeys.usage(code: "Enter"), 0x28); XCTAssertEqual(R7DeviceKeys.usage(code: "ArrowUp"), 0x52); XCTAssertEqual(R7DeviceKeys.usage(code: "MetaRight"), 0xe7)
        XCTAssertNil(R7DeviceKeys.usage(code: "F5")); XCTAssertNil(R7DeviceKeys.usage(code: "Home"))
        XCTAssertEqual(R7DeviceKeys.packet(platform: "ios", phase: "down", keyCode: 0, characters: "a", meta: false, control: false), R6DeviceWire.tagged(0x06, ["type": "down", "usage": 4]))
        XCTAssertEqual(R7DeviceKeys.packet(platform: "ios", phase: "up", keyCode: 56, characters: nil, meta: false, control: false), R6DeviceWire.tagged(0x06, ["type": "up", "usage": 0xe1]))
        XCTAssertNil(R7DeviceKeys.packet(platform: "ios", phase: "down", keyCode: 96, characters: "", meta: false, control: false), "F5 has no usage")
        // Android: Back for Escape, key codes for named keys, text for the produced character.
        func android(_ keyCode: UInt16, _ characters: String?, meta: Bool = false, phase: String = "down") -> [String: Any] {
            json(R7DeviceKeys.packet(platform: "android", phase: phase, keyCode: keyCode, characters: characters, meta: meta, control: false) ?? Data())
        }
        XCTAssertEqual(android(53, "\u{1b}") as NSDictionary, ["type": "back"])
        XCTAssertEqual(android(126, "\u{F700}") as NSDictionary, ["type": "key", "keycode": 19])
        XCTAssertEqual(android(36, "\r") as NSDictionary, ["type": "key", "keycode": 66])
        XCTAssertEqual(android(51, "\u{7f}") as NSDictionary, ["type": "key", "keycode": 67])
        XCTAssertEqual(android(115, nil) as NSDictionary, ["type": "key", "keycode": 122])
        XCTAssertEqual(android(0, "A") as NSDictionary, ["type": "text", "text": "A"])
        XCTAssertEqual(android(0, "å") as NSDictionary, ["type": "text", "text": "å"])
        XCTAssertTrue(android(0, "a", meta: true).isEmpty, "⌘ chords are not typed")
        XCTAssertTrue(android(0, "a", phase: "up").isEmpty, "Android takes key downs only")
        XCTAssertEqual(R7DeviceKeys.modifierPhase(keyCode: 56, flags: [.shift]), "down")
        XCTAssertEqual(R7DeviceKeys.modifierPhase(keyCode: 56, flags: []), "up")
        XCTAssertNil(R7DeviceKeys.modifierPhase(keyCode: 0, flags: []))
    }

    func testAndroidStreamDecodesAndForwardsInput() throws {
        let hub = try Hub()
        let encoded = Encoded.make(width: 360, height: 800, colors: [.systemGreen, .systemGreen, .systemGreen])
        XCTAssertEqual(encoded.samples.count, 3); XCTAssertTrue(encoded.samples[0].key)
        var reports = 0
        let view = R6DeviceScreenView(key: "local\u{0}emulator-5554", platform: "android", deviceId: "emulator-5554", hostId: "local",
                                      access: { done in done(hub.origin, "ticket-a") }) { reports += 1 }
        view.presentation = "flat"
        let window = window(view)
        hub.onSocket = { path, _ in
            guard path.contains("/vendor/serve-emu/ws?") else { return }
            hub.push("/ws?", R7H264.semu(encoded.annexB(0, parameterSets: true), key: true, pts: 0))
        }
        view.start()
        spin(until: { view.inputConnected })
        XCTAssertTrue(view.inputConnected)
        let line = try XCTUnwrap(hub.requests.first { $0.contains("serve-emu") })
        XCTAssertTrue(line.hasPrefix("GET /api/device-hub/vendor/serve-emu/ws?device=emulator-5554&frame-meta=1&wsTicket=ticket-a&hostId=local "), line)
        // The first keyframe configures the decoder; the client asks for a fresh one.
        spin(until: { hub.texts("/ws?").contains { $0.contains("reset-video") } })
        XCTAssertEqual(json(Data(try XCTUnwrap(hub.texts("/ws?").first { $0.contains("reset-video") }).utf8)) as NSDictionary, ["type": "reset-video", "ack": false])
        XCTAssertEqual(view.status, "connecting")
        hub.push("/ws?", R7H264.semu(encoded.annexB(0, parameterSets: true), key: true, pts: 16_667))
        hub.push("/ws?", R7H264.semu(encoded.annexB(1, parameterSets: false), key: false, pts: 33_334))
        spin(until: { view.status == "streaming" })
        XCTAssertEqual(view.status, "streaming")
        XCTAssertEqual(view.reportValue["width"] as? Double, 360); XCTAssertEqual(view.reportValue["height"] as? Double, 800)
        XCTAssertEqual(view.reportValue["orientation"] as? String, "portrait")
        let image = try XCTUnwrap(view.image)
        // The bar is drawn a third of the way down (CoreGraphics y-up, 2/3 to 2/3 + 1/8).
        let center = pixel(image, x: 0.5, y: 0.6)
        XCTAssertTrue(center.g > 150 && center.r < 120 && center.b < 140, "decoded green, got \(center)")
        let bar = pixel(image, x: 0.5, y: 1.0 / 3 - 1.0 / 16)
        XCTAssertTrue(bar.r > 200 && bar.g > 200 && bar.b > 200, "decoded white bar, got \(bar)")
        XCTAssertGreaterThan(reports, 0)
        save(window, "r7-android-flat.png")
        // Touches are JSON in the displayed frame; keys, Back, Home and Recents follow.
        let frame = view.frameRect
        view.mouseDown(with: mouseEvent(.leftMouseDown, at: NSPoint(x: frame.midX, y: frame.minY + frame.height / 4), in: view))
        view.mouseUp(with: mouseEvent(.leftMouseUp, at: NSPoint(x: frame.midX, y: frame.minY + frame.height / 4), in: view))
        view.keyDown(with: keyEvent(.keyDown, keyCode: 0, characters: "a", window: window))
        view.keyUp(with: keyEvent(.keyUp, keyCode: 0, characters: "a", window: window))
        view.keyDown(with: keyEvent(.keyDown, keyCode: 53, characters: "\u{1b}", window: window))
        view.keyDown(with: keyEvent(.keyDown, keyCode: 126, characters: "\u{F700}", window: window))
        view.keyDown(with: keyEvent(.keyDown, keyCode: 0, characters: "a", flags: .command, window: window))
        view.press("back"); view.press("recents"); view.press("home")
        spin(until: { hub.texts("/ws?").count >= 9 })
        let sent = hub.texts("/ws?").map { json(Data($0.utf8)) as NSDictionary }.filter { $0["type"] as? String != "reset-video" }
        XCTAssertEqual(sent.count, 8, "\(sent)")
        XCTAssertEqual(sent[0]["type"] as? String, "touch"); XCTAssertEqual(sent[0]["action"] as? String, "down")
        XCTAssertEqual(sent[0]["x"] as? Double ?? 0, 0.5, accuracy: 0.01); XCTAssertEqual(sent[0]["y"] as? Double ?? 0, 0.25, accuracy: 0.01)
        XCTAssertEqual(sent[1]["action"] as? String, "up")
        XCTAssertEqual(Array(sent[2...]), [["type": "text", "text": "a"], ["type": "back"], ["type": "key", "keycode": 19], ["type": "back"], ["type": "recents"], ["type": "home"]] as [NSDictionary])
        // A new video session (a rotation) drops the decoder and asks for a keyframe again.
        let resets = hub.texts("/ws?").filter { $0.contains("reset-video") }.count
        hub.push("/ws?", Data(#"{"type":"video-session","width":800,"height":360}"#.utf8), text: true)
        spin(until: { hub.texts("/ws?").filter { $0.contains("reset-video") }.count > resets })
        XCTAssertEqual(view.status, "connecting")
        XCTAssertEqual(view.reportValue["retaining"] as? Bool, true, "the last frame stays while the encoder restarts")
        view.stop()
        window.orderOut(nil)
    }

    func testAndroidSocketDropReconnects() throws {
        let hub = try Hub()
        var tickets = 0
        let view = R6DeviceScreenView(key: "k", platform: "android", deviceId: "emulator-5554", hostId: "local",
                                      access: { done in tickets += 1; done(hub.origin, "ticket-\(tickets)") }) {}
        let window = window(view)
        view.start()
        spin(until: { view.inputConnected })
        hub.close("/ws?", code: 1001)
        spin(until: { !view.inputConnected })
        XCTAssertEqual(view.inputDetail, "closed 1001")
        XCTAssertEqual(view.status, "connecting")
        spin(until: { view.inputConnected }, timeout: 5)
        XCTAssertTrue(view.inputConnected, "retried after a second")
        hub.refuseSockets = true
        hub.close("/ws?", code: 1001)
        spin(until: { tickets >= 3 }, timeout: 6)
        XCTAssertGreaterThanOrEqual(tickets, 3, "a refused upgrade fetches a fresh ticket")
        view.stop()
        window.orderOut(nil)
    }

    /// serve-sim: the helper is primed through the MJPEG endpoint, then its input socket opens;
    /// video is the AVCC body.
    private func iosHub(_ encoded: Encoded, description: Data? = nil) throws -> Hub {
        let hub = try Hub()
        let jpeg = NSBitmapImageRep(cgImage: Self.solid(width: 120, height: 260, color: .systemOrange)).representation(using: .jpeg, properties: [:])!
        hub.http.append(("/stream.mjpeg", { _, connection in
            hub.open(connection, type: "multipart/x-mixed-replace; boundary=frame")
            hub.write(connection, Data("--frame\r\nContent-Type: image/jpeg\r\nContent-Length: \(jpeg.count)\r\n\r\n".utf8) + jpeg + Data("\r\n".utf8))
        }))
        hub.http.append(("/stream.avcc", { _, connection in
            hub.open(connection, type: "application/octet-stream")
            var body = R7H264.Demuxer.envelope(.description, description ?? encoded.avcC)
            for sample in encoded.samples { body += R7H264.Demuxer.envelope(sample.key ? .keyframe : .delta, sample.data) }
            hub.write(connection, body)
        }))
        return hub
    }
    static func solid(width: Int, height: Int, color: NSColor) -> CGImage {
        let context = CGContext(data: nil, width: width, height: height, bitsPerComponent: 8, bytesPerRow: 0, space: CGColorSpace(name: CGColorSpace.sRGB)!, bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue)!
        context.setFillColor(color.cgColor); context.fill(CGRect(x: 0, y: 0, width: width, height: height))
        return context.makeImage()!
    }

    func testIosAvccStreamAndHidKeys() throws {
        let encoded = Encoded.make(width: 390, height: 844, colors: [.systemBlue, .systemBlue])
        let hub = try iosHub(encoded)
        let view = R6DeviceScreenView(key: "local\u{0}SIM-1", platform: "ios", deviceId: "SIM-1", hostId: "local", access: { done in done(hub.origin, "ticket-i") }) {}
        view.presentation = "flat"
        let window = window(view)
        view.start()
        spin(until: { view.status == "streaming" && view.inputConnected })
        XCTAssertEqual(view.status, "streaming"); XCTAssertTrue(view.inputConnected)
        XCTAssertEqual(view.reportValue["mjpeg"] as? Bool, false)
        let requests = hub.requests
        XCTAssertTrue(requests.contains { $0.hasPrefix("GET /api/device-hub/vendor/serve-sim/helper/SIM-1/stream.avcc?wsTicket=ticket-i&hostId=local ") }, "\(requests)")
        XCTAssertTrue(requests.contains { $0.contains("/helper/SIM-1/stream.mjpeg") }, "the helper is primed")
        XCTAssertTrue(requests.contains { $0.hasPrefix("GET /api/device-hub/vendor/serve-sim/helper/ws?device=SIM-1&wsTicket=ticket-i") })
        let center = pixel(try XCTUnwrap(view.image), x: 0.5, y: 0.6)
        XCTAssertTrue(center.b > 180 && center.r < 100, "decoded blue, got \(center)")
        hub.push("/helper/ws", R6DeviceWire.tagged(0x82, ["width": 390, "height": 844, "orientation": "landscape_left"]))
        spin(until: { view.screen?.orientation == "landscape_left" })
        XCTAssertEqual(view.rotation, 90, "a portrait framebuffer reporting landscape is turned")
        XCTAssertEqual(view.aspect, 844.0 / 390, accuracy: 0.001)
        XCTAssertEqual(view.reportValue["width"] as? Double, 844)
        save(window, "r7-ios-flat-landscape.png")
        // Keys: HID usages down and up; ⌘ chords stay with the app except ⌘R.
        window.makeFirstResponder(view)
        view.keyDown(with: keyEvent(.keyDown, keyCode: 4, characters: "h", window: window))
        view.keyUp(with: keyEvent(.keyUp, keyCode: 4, characters: "h", window: window))
        view.flagsChanged(with: NSEvent.keyEvent(with: .flagsChanged, location: .zero, modifierFlags: [.shift], timestamp: 0, windowNumber: window.windowNumber, context: nil, characters: "", charactersIgnoringModifiers: "", isARepeat: false, keyCode: 56)!)
        view.keyDown(with: keyEvent(.keyDown, keyCode: 0, characters: "a", flags: .command, window: window))
        XCTAssertTrue(view.performKeyEquivalent(with: keyEvent(.keyDown, keyCode: 15, characters: "r", flags: .command, window: window)))
        view.press("home")
        // A touch on the turned picture is remapped into the raw framebuffer.
        let frame = view.frameRect
        view.mouseDown(with: mouseEvent(.leftMouseDown, at: NSPoint(x: frame.minX + frame.width * 0.25, y: frame.minY + frame.height * 0.75), in: view))
        view.mouseUp(with: mouseEvent(.leftMouseUp, at: NSPoint(x: frame.minX + frame.width * 0.25, y: frame.minY + frame.height * 0.75), in: view))
        spin(until: { hub.received("/helper/ws").count >= 8 })
        let packets = hub.received("/helper/ws")
        XCTAssertEqual(packets.first, R6DeviceWire.tagged(0x0d, ["enabled": false]))
        XCTAssertEqual(Array(packets[1...4]), [R6DeviceWire.tagged(0x06, ["type": "down", "usage": 0x0b]), R6DeviceWire.tagged(0x06, ["type": "up", "usage": 0x0b]),
                                               R6DeviceWire.tagged(0x06, ["type": "down", "usage": 0xe1]), R6DeviceWire.tagged(0x06, ["type": "down", "usage": 0x15])])
        XCTAssertEqual(packets[5], R6DeviceWire.tagged(0x04, ["button": "home"]))
        let touch = json(packets[6].dropFirst())
        XCTAssertEqual(packets[6].first, 0x03)
        XCTAssertEqual(touch["type"] as? String, "begin")
        XCTAssertEqual(touch["x"] as? Double ?? 0, 0.75, accuracy: 0.01); XCTAssertEqual(touch["y"] as? Double ?? 0, 0.75, accuracy: 0.01)
        view.stop()
        window.orderOut(nil)
    }

    func testIosUndecodableDescriptionFallsBackToMjpeg() throws {
        let encoded = Encoded.make(width: 390, height: 844, colors: [.systemBlue])
        let hub = try iosHub(encoded, description: Data([1, 0x64, 0, 0x28, 0xff, 0xe1, 0, 2, 0x67, 0x00, 1, 0, 1, 0x68]))
        let view = R6DeviceScreenView(key: "k", platform: "ios", deviceId: "SIM-2", hostId: "local", access: { done in done(hub.origin, "t") }) {}
        view.presentation = "phone"
        let window = window(view)
        view.start()
        spin(until: { view.status == "streaming" })
        XCTAssertEqual(view.status, "streaming")
        XCTAssertEqual(view.reportValue["mjpeg"] as? Bool, true)
        XCTAssertEqual(view.reportValue["phone"] as? Bool, false)
        XCTAssertEqual(view.reportValue["phoneUnavailable"] as? String, "3D requires the H.264 stream")
        let center = pixel(try XCTUnwrap(view.image), x: 0.5, y: 0.5)
        XCTAssertTrue(center.r > 200 && center.b < 100, "the MJPEG frame, got \(center)")
        view.stop()
        window.orderOut(nil)
    }

    func testPhoneViewDrawsTheStreamAndTakesTouches() throws {
        let encoded = Encoded.make(width: 390, height: 844, colors: [.systemTeal, .systemTeal])
        let hub = try iosHub(encoded)
        let view = R6DeviceScreenView(key: "k", platform: "ios", deviceId: "SIM-3", hostId: "local", access: { done in done(hub.origin, "t") }) {}
        view.deviceName = "iPhone 17"
        view.presentation = "phone"
        let window = window(view, width: 420, height: 620)
        view.start()
        spin(until: { view.status == "streaming" && view.inputConnected && view.phone != nil })
        XCTAssertEqual(view.reportValue["phone"] as? Bool, true)
        XCTAssertEqual(view.reportValue["phoneUnavailable"] as? String, "")
        let phone = try XCTUnwrap(view.phone)
        RunLoop.main.run(until: Date().addingTimeInterval(0.5))
        save(window, "r7-ios-phone-procedural.png")
        // The middle of the view is the middle of the screen; the corner is off the device.
        let middle = try XCTUnwrap(phone.screenPoint(CGPoint(x: 0.5, y: 0.5), captured: false))
        XCTAssertEqual(middle.x, 0.5, accuracy: 0.02); XCTAssertEqual(middle.y, 0.5, accuracy: 0.02)
        let upper = try XCTUnwrap(phone.screenPoint(CGPoint(x: 0.5, y: 0.3), captured: false))
        XCTAssertLessThan(upper.y, 0.5)
        XCTAssertNil(phone.screenPoint(CGPoint(x: 0.02, y: 0.02), captured: false))
        XCTAssertEqual(phone.screenPoint(CGPoint(x: 0.02, y: 0.02), captured: true)?.x, 0, "a captured drag clamps to the screen")
        // Drag on the screen: touches; drag off it: the device turns, then springs back to rest.
        phone.mouseDown(with: mouseEvent(.leftMouseDown, at: NSPoint(x: phone.bounds.midX, y: phone.bounds.midY), in: phone))
        phone.mouseDragged(with: mouseEvent(.leftMouseDragged, at: NSPoint(x: phone.bounds.midX + 10, y: phone.bounds.midY), in: phone))
        phone.mouseUp(with: mouseEvent(.leftMouseUp, at: NSPoint(x: phone.bounds.midX + 10, y: phone.bounds.midY), in: phone))
        // onPointerUp moves to the release point, then ends.
        spin(until: { hub.received("/helper/ws").filter { $0.first == 0x03 }.count >= 4 })
        let touches = hub.received("/helper/ws").filter { $0.first == 0x03 }.map { json($0.dropFirst()) }
        XCTAssertEqual(touches.map { $0["type"] as? String }, ["begin", "move", "move", "end"])
        XCTAssertEqual(touches[0]["x"] as? Double ?? 0, 0.5, accuracy: 0.02)
        phone.mouseDown(with: mouseEvent(.leftMouseDown, at: NSPoint(x: 8, y: 8), in: phone))
        for step in 1...6 { phone.mouseDragged(with: mouseEvent(.leftMouseDragged, at: NSPoint(x: 8 + step * 15, y: 8), in: phone)); RunLoop.main.run(until: Date().addingTimeInterval(0.016)) }
        RunLoop.main.run(until: Date().addingTimeInterval(0.2))
        save(window, "r7-ios-phone-turned.png")
        phone.mouseUp(with: mouseEvent(.leftMouseUp, at: NSPoint(x: 98, y: 8), in: phone))
        RunLoop.main.run(until: Date().addingTimeInterval(0.2))
        XCTAssertEqual(hub.received("/helper/ws").filter { $0.first == 0x03 }.count, 4, "orbiting sends no touches")
        view.stop()
        window.orderOut(nil)
    }

    func testMotionSnapsToTheNearestViewAndResets() {
        let motion = R7DeviceMotion()
        var now = 1000.0
        motion.dragActive(true, now)
        for _ in 0..<10 { now += 16; motion.orbit(30, 0, now); motion.advance(now) }
        XCTAssertGreaterThan(R7Quat.angle(motion.rotation, R7Quat.identity), 0.3, "the drag turned the device")
        now += 200 // a pause before lifting: no flick
        motion.dragActive(false, now)
        while motion.needsFrame { now += 16; motion.advance(now) }
        // 1800 pt of yaw (≈10.8 rad) snaps into the ±60° yaw family about the rest pose.
        let yaw = 2 * atan2(motion.rotation.imag.y, motion.rotation.real)
        XCTAssertLessThanOrEqual(abs(atan2(sin(yaw), cos(yaw))), .pi / 3 + 1e-6)
        XCTAssertEqual(abs(motion.rotation.imag.x), 0, accuracy: 1e-6)
        motion.reset(R7Quat.identity, now)
        while motion.needsFrame { now += 16; motion.advance(now) }
        XCTAssertEqual(R7Quat.angle(motion.rotation, R7Quat.identity), 0, accuracy: 1e-3)
        // Framing fits the bounds at the reference's 0.565 factor.
        let framing = R7DeviceFraming()
        framing.setBounds(min: SIMD3(-0.55, -1.14, -0.16), max: SIMD3(0.55, 1.14, 0.043), vertical: 16 * .pi / 180, aspect: 0.7, now: 0, immediate: true)
        let tanY: Double = tan(16 * Double.pi / 180)
        let byWidth: Double = 1.1 / (tanY * 0.7), byHeight: Double = 2.28 / tanY
        let expected: Double = max(1, max(byWidth, byHeight) * 0.565 + 0.043)
        XCTAssertEqual(framing.distance, expected, accuracy: 1e-6)
    }

    func testGeometryAndShapes() {
        XCTAssertEqual(R7DeviceGeometry.fit(aspect: 0.5, width: 300, height: 400), CGSize(width: 200, height: 400))
        XCTAssertEqual(R7DeviceGeometry.fit(aspect: 2, width: 300, height: 400), CGSize(width: 300, height: 150))
        XCTAssertEqual(R7DeviceGeometry.fit(aspect: 0.5, width: 300, height: 400, rightInset: 156), CGSize(width: 144, height: 288))
        let landscape = R7DeviceGeometry.layout(screen: .init(width: 390, height: 844, orientation: "landscape_left"), rawWidth: 390, rawHeight: 844)
        XCTAssertEqual(landscape.rotation, -.pi / 2); XCTAssertFalse(landscape.rawLandscape); XCTAssertEqual(landscape.aspect, 390.0 / 844, accuracy: 1e-9)
        let upright = R7DeviceGeometry.screenPoint(u: 0.2, v: 0.9, rotation: 0), turned = R7DeviceGeometry.screenPoint(u: 0.2, v: 0.9, rotation: -.pi / 2)
        XCTAssertEqual(upright.x, 0.2, accuracy: 1e-9); XCTAssertEqual(upright.y, 0.1, accuracy: 1e-9)
        XCTAssertEqual(turned.x, 0.9, accuracy: 1e-9); XCTAssertEqual(turned.y, 0.2, accuracy: 1e-9)
        XCTAssertEqual(R7DeviceModels.id(platform: "ios", name: "iPhone 18 Pro"), "iphone-18-pro")
        XCTAssertEqual(R7DeviceModels.id(platform: "ios", name: "iPhone 18 Pro Max"), "iphone-18-pro-max")
        XCTAssertEqual(R7DeviceModels.id(platform: "ios", name: "iPad Pro 13-inch (M5)"), "ipad-pro-13-m5")
        XCTAssertNil(R7DeviceModels.id(platform: "ios", name: "iPhone 17 Pro"), "never stretch a model to impersonate another device")
        XCTAssertNil(R7DeviceModels.id(platform: "android", name: "iPhone 18 Pro"))
        XCTAssertEqual(R7ShapeProfile.resolve(platform: "ios", name: "iPad Air", portraitAspect: 0.4).id, "ios-tablet")
        XCTAssertEqual(R7ShapeProfile.resolve(platform: "android", name: "Pixel 9", portraitAspect: 0.45).id, "android-phone")
        XCTAssertEqual(R7ShapeProfile.resolve(platform: "android", name: "Pixel Tablet", portraitAspect: 0.62).id, "android-tablet")
        XCTAssertEqual(R7ShapeProfile.resolve(platform: "android", name: "Medium Phone", portraitAspect: 0.7).id, "android-phone")
    }

    func testAccessibilityTreesAndOverlay() throws {
        let ios: [Any] = [["frame": ["x": 0, "y": 0, "width": 390, "height": 844], "AXLabel": "App", "children": [
            ["frame": ["x": 0, "y": 0, "width": 390, "height": 844], "AXLabel": "Window"],
            ["frame": ["x": 20, "y": 100, "width": 195, "height": 44], "AXLabel": "Sign in", "AXUniqueId": "signin", "type": "Button"],
            ["frame": ["x": 20, "y": 200, "width": 0, "height": 44], "AXLabel": "Empty"],
        ]]]
        let elements = try XCTUnwrap(R7DeviceAx.parse(platform: "ios", ios))
        XCTAssertEqual(elements, [R7AxElement(id: "signin", label: "Sign in", role: "Button", x: 20 / 390, y: 100.0 / 844, width: 0.5, height: 44.0 / 844)])
        let android: [String: Any] = ["nodes": [
            ["bounds": ["left": 0, "top": 0, "right": 1080, "bottom": 2400], "className": "android.widget.FrameLayout"],
            ["id": 7, "bounds": ["left": 0, "top": 0, "right": 1080, "bottom": 2300], "text": "Full", "className": "android.view.View"],
            ["id": 8, "bounds": ["left": 108, "top": 240, "right": 540, "bottom": 480], "text": "", "contentDescription": "Search", "className": "android.widget.ImageButton"],
            ["id": 9, "bounds": ["left": 0, "top": 600, "right": 100, "bottom": 700], "className": "android.view.View"],
            ["id": 10, "bounds": ["left": 0, "top": 800, "right": 540, "bottom": 900], "clickable": true, "className": "android.widget.Button"],
        ]]
        let flat = try XCTUnwrap(R7DeviceAx.parse(platform: "android", android))
        XCTAssertEqual(flat.map(\.id), ["8", "10"])
        XCTAssertEqual(flat[0].label, "Search"); XCTAssertEqual(flat[0].role, "ImageButton"); XCTAssertEqual(flat[0].x, 0.1, accuracy: 1e-9); XCTAssertEqual(flat[0].height, 0.1, accuracy: 1e-9)
        XCTAssertNil(R7DeviceAx.parse(platform: "android", ["error": "no device"]))
        XCTAssertNil(R7DeviceAx.parse(platform: "ios", ["error": "x"]))
        // Drawn over the flat picture while the overlay is on; 3D is unavailable meanwhile.
        let encoded = Encoded.make(width: 390, height: 844, colors: [.white, .white])
        let hub = try iosHub(encoded)
        hub.http.append(("/helper/SIM-4/ax", { _, connection in hub.respond(connection, body: try! JSONSerialization.data(withJSONObject: ios)) }))
        let streams = R6DeviceStreams(access: { done in done(hub.origin, "t") }, changed: { _ in })
        let view = R6DeviceScreenView(key: "k", platform: "ios", deviceId: "SIM-4", hostId: "local", access: { done in done(hub.origin, "t") }) {}
        view.presentation = "phone"
        view.axOverlay = true
        let feeds = R7DeviceToolFeeds(access: { done in done(hub.origin, "t") }, changed: { _ in })
        view.axSource = { done in feeds.readAx(platform: "ios", deviceId: "SIM-4", hostId: "local", done) }
        let window = window(view)
        view.start()
        spin(until: { view.status == "streaming" && view.overlay.count == 1 })
        XCTAssertEqual(view.overlay.count, 1)
        XCTAssertEqual(view.reportValue["phone"] as? Bool, false)
        XCTAssertEqual(view.reportValue["phoneUnavailable"] as? String, "Turn off accessibility frames to use 3D")
        XCTAssertTrue(hub.requests.contains { $0.hasPrefix("GET /api/device-hub/vendor/serve-sim/helper/SIM-4/ax?wsTicket=t&hostId=local ") })
        save(window, "r7-ios-ax-overlay.png")
        _ = streams
        view.stop()
        window.orderOut(nil)
    }

    func testForegroundAndEventLogFeeds() throws {
        let hub = try Hub()
        hub.http.append(("/appstate", { _, connection in
            hub.open(connection, type: "text/event-stream")
            hub.write(connection, Data(": keep-alive\n\ndata: {\"bundleId\":\"com.example.app\",\"pid\":42}\n\n".utf8))
        }))
        hub.http.append(("/api/event-log/events", { _, connection in
            hub.open(connection, type: "text/event-stream")
            hub.write(connection, Data("data: {\"events\":[{\"id\":1,\"timestamp\":\"2026-10-04T12:34:56.000Z\",\"kind\":\"tap\",\"summary\":\"Tap (120, 300)\"},{\"id\":2,\"timestamp\":\"2026-10-04T12:35:01.000Z\",\"msg\":\"Launch com.example.app\"}]}\n\n".utf8))
            hub.write(connection, Data("data: {\"event\":{\"id\":3,\"timestamp\":\"2026-10-04T12:35:09.000Z\",\"summary\":\"Swipe\"}}\r\n\r\n".utf8))
        }))
        var changes = 0
        let feeds = R7DeviceToolFeeds(access: { done in done(hub.origin, "ticket-f") }, changed: { _ in changes += 1 })
        feeds.watchForeground(panels: [("local\u{0}SIM-5", "ios", "SIM-5", "local"), ("local\u{0}emu", "android", "emu", "local")])
        let host = NSView()
        feeds.watchEvents(host: ObjectIdentifier(host), key: "local\u{0}SIM-5", deviceId: "SIM-5", hostId: "local")
        spin(until: { ((feeds.status["local\u{0}SIM-5"] as? [String: Any])?["events"] as? [[String: Any]])?.count == 3 && (feeds.status["local\u{0}SIM-5"] as? [String: Any])?["foreground"] != nil })
        let status = try XCTUnwrap(feeds.status["local\u{0}SIM-5"] as? [String: Any])
        XCTAssertEqual(status["foreground"] as? String, "com.example.app")
        XCTAssertEqual(status["eventsOpen"] as? Bool, true)
        let events = try XCTUnwrap(status["events"] as? [[String: Any]])
        XCTAssertEqual(events.map { $0["time"] as? String }, ["12:34:56", "12:35:01", "12:35:09"])
        XCTAssertEqual(events.map { $0["summary"] as? String }, ["Tap (120, 300)", "Launch com.example.app", "Swipe"])
        XCTAssertNil(feeds.status["local\u{0}emu"], "Android has no foreground or event-log feed")
        XCTAssertTrue(hub.requests.contains { $0.hasPrefix("GET /api/device-hub/vendor/serve-sim/appstate?device=SIM-5&wsTicket=ticket-f&hostId=local ") })
        XCTAssertTrue(hub.requests.contains { $0.hasPrefix("GET /api/device-hub/vendor/serve-sim/api/event-log/events?device=SIM-5&limit=100&wsTicket=ticket-f&hostId=local ") })
        feeds.unwatchEvents(host: ObjectIdentifier(host))
        XCTAssertNil((feeds.status["local\u{0}SIM-5"] as? [String: Any])?["events"], "closing the log drops its entries")
        XCTAssertGreaterThan(changes, 0)
        feeds.destroy()
    }

    /// One 3D phone over the synthetic screen, in both appearances, for pairing with ref3d.mjs.
    private func renderPhone(platform: String, name: String, width: Int, height: Int, origin: URL?, file: String, android: Bool = false) -> Bool {
        var imported = false
        for dark in [false, true] {
            let phone = R7DevicePhoneView(platform: platform, name: name, touch: { _, _, _ in }, unavailable: { XCTFail("3D unavailable") })
            let window = window(phone, width: 420, height: 620, dark: dark)
            phone.origin = origin
            phone.setScreen(.init(width: Double(width), height: Double(height), orientation: "portrait"))
            phone.setFrame(android ? Self.androidImage(width: width / 3, height: height / 3) : Self.screenImage(width: width / 3, height: height / 3))
            if origin != nil { spin(until: { phone.isImported }, timeout: 20); imported = phone.isImported } else { RunLoop.main.run(until: Date().addingTimeInterval(0.6)) }
            let middle = phone.screenPoint(CGPoint(x: 0.5, y: 0.5), captured: false)
            XCTAssertEqual(middle?.x ?? 0, 0.5, accuracy: 0.02, file)
            save(window, "r7-3d-\(file)-\(dark ? "dark" : "light").png")
            phone.dispose()
            window.orderOut(nil)
        }
        return imported
    }

    func testProceduralPhonesRender() {
        _ = renderPhone(platform: "ios", name: "iPhone 17", width: 1206, height: 2622, origin: nil, file: "procedural-ios")
        _ = renderPhone(platform: "android", name: "Pixel 9", width: 1080, height: 2400, origin: nil, file: "procedural-android", android: true)
    }

    /// The model-backed 3D views, when a T3 server's built client is at hand.
    func testServedModelsRender() throws {
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
        var found: [String: URL] = [:]
        R7DeviceModels.locate(origin: hub.origin) { found = $0 }
        spin(until: { !found.isEmpty })
        XCTAssertEqual(Set(found.keys), ["iphone-18-pro", "iphone-18-pro-max", "ipad-pro-13-m5", "iphone-duo", "ipad-pro-13-m5-magic-keyboard"])
        XCTAssertTrue(renderPhone(platform: "ios", name: "iPhone 18 Pro", width: 1206, height: 2622, origin: hub.origin, file: "iphone-18-pro"))
        XCTAssertTrue(renderPhone(platform: "ios", name: "iPad Pro 13-inch (M5)", width: 2064, height: 2752, origin: hub.origin, file: "ipad-pro"))
    }

    static func androidImage(width: Int, height: Int) -> CGImage {
        let context = CGContext(data: nil, width: width, height: height, bitsPerComponent: 8, bytesPerRow: 0, space: CGColorSpace(name: CGColorSpace.sRGB)!, bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue)!
        context.setFillColor(NSColor(srgbRed: 52 / 255, green: 199 / 255, blue: 89 / 255, alpha: 1).cgColor); context.fill(CGRect(x: 0, y: 0, width: width, height: height))
        context.setFillColor(NSColor.white.cgColor); context.fill(CGRect(x: Double(width) / 4, y: Double(height) * 2 / 3, width: Double(width) / 2, height: Double(height) / 8))
        return context.makeImage()!
    }

    /// A recognisable screen: a dark gradient, a light card and a coloured bar.
    static func screenImage(width: Int, height: Int) -> CGImage {
        let context = CGContext(data: nil, width: width, height: height, bitsPerComponent: 8, bytesPerRow: 0, space: CGColorSpace(name: CGColorSpace.sRGB)!, bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue)!
        let gradient = CGGradient(colorsSpace: CGColorSpace(name: CGColorSpace.sRGB)!, colors: [NSColor(srgbRed: 0.10, green: 0.16, blue: 0.40, alpha: 1).cgColor, NSColor(srgbRed: 0.55, green: 0.20, blue: 0.55, alpha: 1).cgColor] as CFArray, locations: [0, 1])!
        context.drawLinearGradient(gradient, start: CGPoint(x: 0, y: height), end: CGPoint(x: width, y: 0), options: [])
        context.setFillColor(NSColor(white: 0.97, alpha: 1).cgColor)
        context.addPath(CGPath(roundedRect: CGRect(x: Double(width) * 0.08, y: Double(height) * 0.45, width: Double(width) * 0.84, height: Double(height) * 0.3), cornerWidth: 24, cornerHeight: 24, transform: nil)); context.fillPath()
        context.setFillColor(NSColor(srgbRed: 1, green: 0.62, blue: 0.1, alpha: 1).cgColor)
        context.fill(CGRect(x: Double(width) * 0.08, y: Double(height) * 0.1, width: Double(width) * 0.84, height: Double(height) * 0.06))
        return context.makeImage()!
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
let suite = XCTestSuite(forTestCaseClass: R7DeviceTests.self)
suite.run()
let run = suite.testRun!
print("Executed \(run.executionCount) tests, with \(run.totalFailureCount) failures")
exit(run.totalFailureCount == 0 ? 0 : 1)
