#if os(macOS)
import AppKit
import XCTest
@testable import ExactKit

// A plan restart carries each uniquely named canvas's world to the new plan's
// canvas of that name (LLP 1046.009 G1), restored as a carry, never as the
// agent's EXACT_WORLD.
private let out = UnsafeMutablePointer<UInt8>.allocate(capacity: 64)
private var saved: [UInt8] = [7, 8, 9]
private var restores: [(id: UInt32, bytes: [UInt8], mode: UInt32)] = []

private func fill(_ bytes: [UInt8]) -> UInt32 {
    bytes.withUnsafeBufferPointer { out.update(from: $0.baseAddress!, count: $0.count) }
    return UInt32(bytes.count)
}

final class PlanCarryTests: XCTestCase {
    func testRestartCarriesAUniqueNameAndRestoresItAsACarry() {
        restores = []
        let m = module(), s = session(m)
        defer { s.destroy() }
        add(s, m, view: 100, name: "world")
        s.canvases.carryForRestart()
        XCTAssertEqual(s.canvases.planCarries["world"], Data(saved))
        s.canvases.reset()
        s.canvases.surface(view: NodeView(id: 200, kind: "canvas", presenter: s.presenter), name: "world", values: [])
        let e = s.canvases.entries[200]!
        XCTAssertEqual(e.carry, Data(saved), "the new canvas of the name takes the world")
        XCTAssertTrue(s.canvases.planCarries.isEmpty)
        e.id = 2; e.module = m
        s.canvases.worldInput.bytes = Data([1])
        s.canvases.restoreWorld(m, e)
        XCTAssertEqual(restores.map(\.mode), [1], "restored as a carry (Restore::Carry)")
        XCTAssertEqual(restores.first?.bytes, saved)
        XCTAssertEqual(s.canvases.worldInput.bytes, Data([1]), "the agent's world input is not consumed by a carry")
        XCTAssertNil(e.carry)
        s.canvases.finishRestart()
        XCTAssertTrue(s.canvases.planCarries.isEmpty)
    }

    func testDuplicateNamesStartFresh() {
        let m = module(), s = session(m)
        defer { s.destroy() }
        add(s, m, view: 100, name: "world")
        add(s, m, view: 101, name: "world")
        s.canvases.carryForRestart()
        XCTAssertTrue(s.canvases.planCarries.isEmpty, "names alone cannot match duplicate worlds")
    }

    private func add(_ s: ExactSession, _ m: GpuModule, view: UInt32, name: String) {
        let e = Canvases.Entry(view: NodeView(id: view, kind: "canvas", presenter: s.presenter), name: name, values: [])
        e.id = view
        e.module = m
        s.canvases.entries[view] = e
    }

    private func module() -> GpuModule {
        let m = GpuModule(create: { _, _, _, _, _ in 0 }, bind: { _, _, _ in 0 },
            render: { _, _, _, _, _ in 0 }, dirty: { _ in 0 }, destroy: { _ in },
            texture: { _, _, _, _, _ in 0 }, textureMetal: nil, sync: nil,
            childrenMode: { _ in 0 }, readback: { _, _, _, _, _, _, _ in 0 },
            child: { _, _, _, _, _, _, _, _, _, _, _, _ in 0 },
            childrenCount: { _, _ in 0 }, placement: { _, _, _, _ in 0 },
            shader: nil, validateShader: nil, clearShaders: nil, errorLen: { 0 },
            errorPtr: { nil }, wantsInput: nil, input: nil, messages: nil,
            published: nil, agent: { _, _, _ in fill(Array("{\"world\":{\"restored\":true}}".utf8)) },
            outPtr: { UnsafePointer(out) })
        m.carry = { _ in fill(saved) }
        m.restore = { id, data, len, mode in
            restores.append((id, Array(UnsafeBufferPointer(start: data, count: len)), mode))
            return true
        }
        return m
    }

    private func session(_ module: GpuModule) -> ExactSession {
        _ = NSApplication.shared
        let s = ExactApp.shared.makeSession(label: "plan-carry")
        s.canvases.modules[""] = module
        s.canvases.attempted = [""]
        return s
    }
}
#endif
