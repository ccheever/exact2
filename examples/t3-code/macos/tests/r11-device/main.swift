import AppKit
import SceneKit
import XCTest
import Network

// Lane r11-device: the 3D phone on its first open under a loaded main thread. A live iOS feed
// (serve-sim's AVCC body at 60 fps over the loopback hub in hub.swift) keeps H.264, and so the 3D
// phone, through main-thread stalls that hand the decoder a burst of samples at once; a decoder
// that really falls behind still takes the reference's MJPEG fallback.
final class R11DeviceTests: XCTestCase {
    private func spin(until condition: () -> Bool, timeout: TimeInterval = 10) {
        let end = Date().addingTimeInterval(timeout)
        while !condition() && Date() < end { RunLoop.main.run(until: Date().addingTimeInterval(0.02)) }
    }
    private func window(_ view: NSView) -> NSWindow {
        let window = NSWindow(contentRect: NSRect(x: 80, y: 80, width: 420, height: 720), styleMask: [.borderless], backing: .buffered, defer: false)
        view.frame = window.contentView!.bounds
        view.autoresizingMask = [.width, .height]
        window.contentView!.addSubview(view)
        window.orderFrontRegardless()
        return window
    }

    /// serve-sim's `stream.avcc`: the description, then one sample every `every` ms, looped.
    private func pacedHub(_ encoded: Encoded, every: Int) throws -> Hub {
        let hub = try Hub()
        hub.http.append(("/stream.mjpeg", { _, connection in hub.open(connection, type: "multipart/x-mixed-replace; boundary=frame") }))
        hub.http.append(("/stream.avcc", { _, connection in
            hub.open(connection, type: "application/octet-stream")
            hub.write(connection, R7H264.Demuxer.envelope(.description, encoded.avcC))
            let timer = DispatchSource.makeTimerSource(queue: hub.queue)
            var index = 0
            timer.schedule(deadline: .now(), repeating: .milliseconds(every))
            timer.setEventHandler {
                guard connection.state == .ready else { return timer.cancel() }
                let sample = encoded.samples[index % encoded.samples.count]; index += 1
                hub.write(connection, R7H264.Demuxer.envelope(sample.key ? .keyframe : .delta, sample.data))
            }
            timer.resume()
            hub.timers.append(timer)
        }))
        return hub
    }

    /// Round 10's matrix (ios-3d-light-1280) and round 8's outlier: the phone fell back to flat on
    /// its first open. Building the phone's scene, and any busy pass after it, stalls the main
    /// thread; the samples that arrived meanwhile reach the decoder together, over the soft queue,
    /// and the feed took MJPEG ("3D requires the H.264 stream"). It must stay on H.264 and in 3D.
    func testMainThreadStallsKeepThe3DPhone() throws {
        let colors: [NSColor] = (0..<60).map { $0 % 2 == 0 ? .systemBlue : .systemTeal }
        let encoded = Encoded.make(width: 1206, height: 2622, colors: colors)
        XCTAssertEqual(encoded.samples.count, 60)
        let hub = try pacedHub(encoded, every: 16)
        let view = R6DeviceScreenView(key: "local\u{0}SIM-3D", platform: "ios", deviceId: "SIM-3D", hostId: "local", access: { done in done(hub.origin, "ticket-3d") }) {}
        view.deviceName = "iPhone 18 Pro"
        view.presentation = "phone"
        let window = window(view)
        view.start()
        spin(until: { view.reportValue["phone"] as? Bool == true }, timeout: 15)
        XCTAssertEqual(view.reportValue["phone"] as? Bool, true, "the 3D phone opens: \(view.reportValue)")
        for stall in [0.3, 0.6, 1.0, 2.0, 0.6, 0.6] {
            Thread.sleep(forTimeInterval: stall)
            RunLoop.main.run(until: Date().addingTimeInterval(0.6))
            XCTAssertEqual(view.reportValue["mjpeg"] as? Bool, false, "after a \(stall) s stall: \(view.reportValue)")
        }
        XCTAssertEqual(view.client.recovered, "", "no recovery: a stall is not a decoder that fell behind")
        XCTAssertEqual(view.reportValue["phone"] as? Bool, true, "still 3D")
        XCTAssertEqual(view.reportValue["phoneUnavailable"] as? String, "")
        XCTAssertEqual(view.status, "streaming")
        // Still live: the picture keeps changing.
        let before = view.image
        spin(until: { view.image !== before }, timeout: 2)
        XCTAssertFalse(view.image === before, "frames keep arriving")
        view.stop()
        window.orderOut(nil)
    }

    /// The same first open, ten times over, each with a stall right as the phone appears.
    func testFirstOpenUnderStallsTenTimes() throws {
        let encoded = Encoded.make(width: 1206, height: 2622, colors: (0..<30).map { $0 % 2 == 0 ? .systemIndigo : .systemMint })
        let hub = try pacedHub(encoded, every: 16)
        var phones = 0
        for run in 0..<10 {
            let view = R6DeviceScreenView(key: "local\u{0}SIM-\(run)", platform: "ios", deviceId: "SIM-\(run)", hostId: "local", access: { done in done(hub.origin, "t\(run)") }) {}
            view.deviceName = "iPhone 18 Pro"
            view.presentation = "phone"
            let window = window(view)
            view.start()
            spin(until: { view.reportValue["phone"] as? Bool == true }, timeout: 15)
            Thread.sleep(forTimeInterval: 0.8)
            RunLoop.main.run(until: Date().addingTimeInterval(1.5))
            if view.reportValue["phone"] as? Bool == true, view.reportValue["mjpeg"] as? Bool == false { phones += 1 }
            view.stop()
            window.orderOut(nil)
        }
        XCTAssertEqual(phones, 10, "the 3D phone stayed in \(phones) of 10 first opens")
    }

    /// The reference's rule still holds for a decoder that cannot keep up: a backlog over the soft
    /// queue that does not shrink for a second is behind; a burst that drains is not.
    func testBacklogWindow() {
        var backlog = R11DecodeBacklog()
        XCTAssertFalse(backlog.behind(pending: 8, now: 0), "at the soft queue")
        // A burst: 40 samples at once, draining.
        XCTAssertFalse(backlog.behind(pending: 40, now: 1))
        XCTAssertFalse(backlog.behind(pending: 30, now: 1.5))
        XCTAssertFalse(backlog.behind(pending: 12, now: 2.01), "shrank over the window: still draining")
        XCTAssertFalse(backlog.behind(pending: 3, now: 2.2), "drained")
        XCTAssertFalse(backlog.behind(pending: 9, now: 2.3))
        XCTAssertFalse(backlog.behind(pending: 2, now: 3.4), "dipped under the soft queue: the window closes")
        // A decoder slower than the stream: the backlog grows.
        XCTAssertFalse(backlog.behind(pending: 9, now: 10))
        XCTAssertFalse(backlog.behind(pending: 14, now: 10.5))
        XCTAssertTrue(backlog.behind(pending: 20, now: 11), "over the soft queue for a second and not shrinking")
        // A steady backlog over the soft queue is behind too.
        backlog.reset()
        XCTAssertFalse(backlog.behind(pending: 12, now: 20))
        XCTAssertTrue(backlog.behind(pending: 12, now: 21.2))
        // Shrinking slowly but never under the soft queue: each window measured from the last.
        backlog.reset()
        XCTAssertFalse(backlog.behind(pending: 60, now: 30))
        XCTAssertFalse(backlog.behind(pending: 50, now: 31))
        XCTAssertTrue(backlog.behind(pending: 55, now: 32.1), "grew again over the next window")
    }
}

_ = NSApplication.shared
NSApp.setActivationPolicy(.accessory)
let suite = XCTestSuite(forTestCaseClass: R11DeviceTests.self)
suite.run()
let run = suite.testRun!
print("Executed \(run.executionCount) tests, with \(run.totalFailureCount) failures")
exit(run.totalFailureCount == 0 ? 0 : 1)
