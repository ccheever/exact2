#if os(macOS)
import AppKit
import XCTest
@testable import ExactKit

private var periods: [Double] = []
private var events: [UInt32] = []
private let replyBuffer = UnsafeMutablePointer<UInt8>.allocate(capacity: 512)
private var replyLength: UInt32 = 0
private var recoveryCalls = 0
private var mockLost = true
private var failRecoveryOnce = false
private func recoverReply() -> UInt32 {
    recoveryCalls += 1
    let failed = failRecoveryOnce && recoveryCalls == 1
    mockLost = failed
    let bytes = Array("{\"status\":\"\(failed ? "failed" : "recovered")\"}".utf8)
    bytes.withUnsafeBufferPointer { replyBuffer.update(from: $0.baseAddress!, count: $0.count) }
    return UInt32(bytes.count)
}
private func restoreState(_ restored: Bool) {
    let bytes = Array("{\"world\":{\"restored\":\(restored)}}".utf8)
    bytes.withUnsafeBufferPointer { replyBuffer.update(from: $0.baseAddress!, count: $0.count) }
    replyLength = UInt32(bytes.count)
}

private final class VisibleWindow: NSWindow {
    override var occlusionState: NSWindow.OcclusionState { .visible }
}

final class CanvasClockAudioTests: XCTestCase {
    func testFailedRecoveryLeavesCanvasEligibleForRetry() {
        let s = session(module())
        defer { s.destroy() }
        s.canvases.recoveredDevice(false, error: "transient adapter failure")
        XCTAssertTrue(s.canvases.entries[100]!.presentable)
        XCTAssertTrue(s.canvases.entries[100]!.wants)
    }

    func testRecoveryCoalescesFiltersDeviceAndRetriesOneLossGeneration() {
        for failOnce in [false, true] {
            recoveryCalls = 0; mockLost = true; failRecoveryOnce = failOnce
            let m = module()
            let s = session(m); defer { s.destroy() }; m.canvases.add(s.canvases)
            let entry = s.canvases.entries[100]!
            m.recover = { recoverReply() }; m.deviceLost = { mockLost }; m.deviceID = { 0 }
            XCTAssertNotNil(m.deliveryClock(entry, now: 500)["now"])
            m.removedDevice(42, generation: 0)
            XCTAssertEqual(recoveryCalls, 0)
            m.recoverDevice(); m.removedDevice(0, generation: 0); m.recoverDevice()
            XCTAssertEqual(recoveryCalls, 0, "recovery is always queued")
            let done = expectation(description: "replacement")
            DispatchQueue.main.asyncAfter(deadline: .now() + 0.3) { done.fulfill() }
            wait(for: [done], timeout: 2)
            XCTAssertEqual(recoveryCalls, failOnce ? 2 : 1)
            XCTAssertEqual(m.lossGeneration, 1)
            XCTAssertNil(m.deliveryClock(entry, now: 900)["now"], "redelivery must not seek")
            m.removedDevice(0, generation: 0); m.recoverDevice()
            XCTAssertEqual(recoveryCalls, failOnce ? 2 : 1)
        }
    }

    func testSecondRestoreReplacesContactsIncludingEmptySave() {
        let m=module(), s=session(module()); defer { s.destroy() }
        let e=s.canvases.entries[100]!
        e.controls[7]=SurfaceControl(node:101,name:"jump",offset:.zero,position:.zero)
        e.restorePending=true; restoreState(true); s.canvases.finishRestore(m,e)
        XCTAssertTrue(e.controls.isEmpty)
    }

    private func module() -> GpuModule {
        let m = GpuModule(create: { _, _, _, _, _ in 1 }, bind: { _, _, _ in 0 },
            render: { _, _, _, _, _ in 0 }, dirty: { _ in 0 }, destroy: { _ in },
            texture: { _, _, _, _, _ in 0 }, textureMetal: nil, sync: nil,
            childrenMode: { _ in 0 }, readback: { _, _, _, _, _, _, _ in 0 },
            child: { _, _, _, _, _, _, _, _, _, _ in 0 },
            childrenCount: { _, _ in 0 }, placement: { _, _, _, _ in 0 },
            shader: nil, validateShader: nil, clearShaders: nil, errorLen: { 0 },
            errorPtr: { nil }, wantsInput: nil, input: nil, messages: nil,
            published: nil, agent: { _, _, _ in replyLength }, outPtr: { UnsafePointer(replyBuffer) })
        m.period = { periods.append($0) }
        m.lifecycle = { _, code in events.append(code) }
        return m
    }
    private func session(_ module: GpuModule) -> ExactSession {
        _ = NSApplication.shared
        let s = ExactApp.shared.makeSession(label: "clock-audio")
        s.canvases.module = module
        s.canvases.loadRequested = true
        let view = NodeView(id: 100, kind: "canvas", presenter: s.presenter)
        let entry = Canvases.Entry(view: view, name: "fixture", values: [])
        entry.id = 1
        s.canvases.entries[100] = entry
        return s
    }
    private func window(_ s: ExactSession) -> NSWindow {
        let w = VisibleWindow(contentRect: NSRect(x: 10, y: 10, width: 100, height: 100),
                         styleMask: [.borderless], backing: .buffered, defer: false)
        w.contentView = ExactView(session: s)
        return w
    }
    func testQuantizerBoundariesHysteresisAndDroppedInterval() {
        for hz in [60.0, 80, 90, 120, 144] {
            var p = DisplayPeriod()
            var sent: [Double] = []
            for _ in 0..<3 { p.publish(1000 / hz, maximum: hz) { sent.append($0) } }
            XCTAssertEqual(sent, [0, 0, 1000 / hz])
            p.publish(2000 / hz, maximum: hz) { sent.append($0) }
            XCTAssertEqual(sent.last!, 1000 / hz)
        }
        var p = DisplayPeriod()
        for _ in 0..<3 { p.publish(1000 / 120, maximum: 120) { _ in } }
        let midpoint = (1000.0 / 120 + 12.5) / 2
        for _ in 0..<5 { p.publish(midpoint + 0.01, maximum: 120) { _ in } }
        XCTAssertEqual(p.value, 1000 / 120)
        for _ in 0..<3 { p.publish(midpoint + 0.1, maximum: 120) { _ in } }
        XCTAssertEqual(p.value, 12.5)
        for _ in 0..<5 { p.publish(midpoint - 0.01, maximum: 120) { _ in } }
        XCTAssertEqual(p.value, 12.5)
        for _ in 0..<3 { p.publish(midpoint - 0.1, maximum: 120) { _ in } }
        XCTAssertEqual(p.value, 1000 / 120)
    }
    func testAlternatingSessionsPublishBeforeEveryRenderIncludingTheFirst() {
        let m = module(), a = session(module()), b = session(module())
        a.canvases.module = m; b.canvases.module = m
        defer { a.destroy(); b.destroy() }
        periods = []
        for frame in 0..<6 {
            for (s, hz) in [(a, 60.0), (b, 120.0)] {
                let count = periods.count
                s.canvases.period(1000 / hz)
                XCTAssertEqual(periods.count, count + 1, "publication precedes this session's render")
                XCTAssertEqual(periods.last!, frame < 2 ? 0 : 1000 / hz)
            }
        }
    }
    func testNoResumePolicyAppliesToNewLifecycleAndGestureRecovers() {
        let s = session(module()), w = window(s)
        defer { w.orderOut(nil); s.destroy() }
        let first = CanvasLifecycle(s.canvases, activate: { true })
        first.interruption(began: true, shouldResume: false)
        first.interruption(began: false, shouldResume: false)
        defer { first.interruption(began: false, shouldResume: true) }
        var attempts = 0
        let second = CanvasLifecycle(s.canvases, activate: { attempts += 1; return true })
        events = []
        second.deliver(1)
        XCTAssertEqual(events.last, 2)
        second.refresh()
        second.requestAudio(userInitiated: false)
        for _ in 0..<600 { second.frame() }
        XCTAssertEqual(attempts, 0)
        second.gesture()
        XCTAssertEqual(attempts, 1)
        XCTAssertEqual(events.last, 3)
    }
    func testNoResumeRecoversOnlyOnTheNextVisibleTransition() {
        let s = session(module()), w = window(s)
        defer { w.orderOut(nil); s.destroy() }
        var attempts = 0
        let lifecycle = CanvasLifecycle(s.canvases, activate: { attempts += 1; return true })
        s.canvases.lifecycle = lifecycle
        lifecycle.requestAudio()
        lifecycle.interruption(began: true, shouldResume: false)
        lifecycle.interruption(began: false, shouldResume: false)
        defer { lifecycle.interruption(began: false, shouldResume: true) }
        lifecycle.refresh()
        XCTAssertEqual(attempts, 1)
        let view = w.contentView
        w.contentView = nil
        w.contentView = view
        XCTAssertEqual(attempts, 2)
        XCTAssertEqual(events.last, 3)
    }
    func testFailedActivationRetriesAt300LiveFrames() {
        let s = session(module()), w = window(s)
        defer { w.orderOut(nil); s.destroy() }
        var attempts = 0
        let lifecycle = CanvasLifecycle(s.canvases, activate: { attempts += 1; return attempts > 1 })
        events = []
        lifecycle.gesture()
        XCTAssertEqual(attempts, 0, "silent surfaces do not activate an audio session")
        lifecycle.requestAudio()
        XCTAssertEqual(attempts, 1)
        XCTAssertEqual(events.last, 2)
        for _ in 0..<299 { lifecycle.frame() }
        XCTAssertEqual(attempts, 1)
        lifecycle.frame()
        XCTAssertEqual(attempts, 2)
        XCTAssertEqual(events.last, 3)
    }
    func testDetachRefreshesExistingLifecycle() {
        let s = session(module()), w = window(s)
        defer { w.orderOut(nil); s.destroy() }
        s.canvases.lifecycle.refresh()
        XCTAssertFalse(s.canvases.lifecycle.hidden)
        events = []
        w.contentView = nil
        XCTAssertTrue(s.canvases.lifecycle.hidden)
        XCTAssertEqual(events.last, 0)
    }
    func testQuantizerNeverExceedsDisplayMaximumAndRepublishesStableClass() {
        var p = DisplayPeriod(), sent: [Double] = []
        for _ in 0..<3 { p.publish(1000 / 60, maximum: 60) { sent.append($0) } }
        for _ in 0..<6 { p.publish(10, maximum: 60) { sent.append($0) } }
        XCTAssertEqual(sent.count, 9)
        XCTAssertEqual(p.value, 1000 / 60)
        for _ in 0..<10 { p.publish(1000 / 60, maximum: 60) { sent.append($0) } }
        XCTAssertEqual(sent.count, 19)
        XCTAssertEqual(sent.last!, 1000 / 60)
    }
    func testRecoveryBroadcastsToTwoLiveSessionsAndLatchesFirstSilentGesture() {
        let a = session(module()), b = session(module()), wa = window(a), wb = window(b)
        defer { wa.orderOut(nil); wb.orderOut(nil); a.destroy(); b.destroy() }
        var ac = 0, bc = 0
        let first = CanvasLifecycle(a.canvases, activate: { ac += 1; return true })
        let second = CanvasLifecycle(b.canvases, activate: { bc += 1; return true })
        first.interruption(began: true, shouldResume: false)
        second.interruption(began: true, shouldResume: false)
        first.interruption(began: false, shouldResume: false)
        second.interruption(began: false, shouldResume: false)
        defer { first.interruption(began: false, shouldResume: true) }
        second.requestAudio(userInitiated: false)
        first.gesture()
        XCTAssertEqual(ac, 0, "permission alone never activates a silent surface")
        XCTAssertEqual(bc, 1, "recovery reaches the other live owner")
        first.requestAudio(userInitiated: false)
        XCTAssertEqual(ac, 1, "the first gesture was retained before wantsAudio")
    }
    func testBindUsesTheSessionCommitClock() {
        let m = module(), s = session(m)
        defer { s.destroy() }
        m.bindAt = { _, _, _, at in periods.append(at); return 0 }
        s.clock = 1234.5; periods = []
        XCTAssertEqual(s.canvases.bindSurface(m, s.canvases.entries[100]!), 0)
        XCTAssertEqual(periods, [1234.5])
    }
    func testDeferredRestoreRetainsCarrierUntilCommitAndReportsLateRefusalOnce() {
        for refused in [true, false] {
            let m = module(), s = session(m)
            defer { s.destroy() }
            let c = s.canvases, e = c.entries[100]!
            m.restore = { _, _, _, _ in true }
            m.carry = { _ in 0 }
            c.worldInput.bytes = Data([1, 2, 3])
            restoreState(false)
            c.restoreWorld(m, e)
            XCTAssertEqual(c.worldInput.bytes, Data([1, 2, 3]))
            if refused {
                c.finishRestore(m, e, refusal: "restore refused: invalid save")
                c.finishRestore(m, e, refusal: "restore refused: invalid save")
                XCTAssertEqual(c.restoreJournal.count, 1)
                XCTAssertEqual(e.restoreError!, "surface fixture: restore refused: invalid save")
                XCTAssertNotNil(c.worldInput.bytes)
            } else {
                restoreState(true); c.finishRestore(m, e)
                XCTAssertNil(c.worldInput.bytes)
                XCTAssertNil(e.restoreError)
            }
        }
    }

}
#endif
