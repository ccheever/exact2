#if os(macOS)
import AppKit
import XCTest
@testable import ExactKit

private var periods: [Double] = []
private var events: [UInt32] = []

private final class VisibleWindow: NSWindow {
    override var occlusionState: NSWindow.OcclusionState { .visible }
}

final class CanvasClockAudioTests: XCTestCase {
    private func module() -> GpuModule {
        let m = GpuModule(create: { _, _, _, _, _ in 1 }, bind: { _, _, _ in 0 },
            render: { _, _, _, _, _ in 0 }, dirty: { _ in 0 }, destroy: { _ in },
            texture: { _, _, _, _, _ in 0 }, textureMetal: nil, sync: nil,
            wantsChildren: { _ in 0 }, readback: { _, _, _, _, _, _, _ in 0 },
            wantsChildrenEach: { _ in 0 }, child: { _, _, _, _, _, _, _, _, _, _ in 0 },
            childrenCount: { _, _ in 0 }, placement: { _, _, _, _ in 0 },
            shader: nil, validateShader: nil, clearShaders: nil, errorLen: { 0 },
            errorPtr: { nil }, wantsInput: nil, input: nil, messages: nil,
            published: nil, agent: nil, outPtr: nil)
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
}
#endif
