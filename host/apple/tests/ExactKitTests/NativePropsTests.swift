#if os(macOS)
import AppKit
private typealias PropView = NSView
#else
import UIKit
private typealias PropView = UIView
#endif
import XCTest
@testable import ExactKit

final class NativePropsTests: XCTestCase {
    func testOversizedPropsNeverReachTheFactoryOrAnExistingInstance() throws {
        #if os(macOS)
        _ = NSApplication.shared
        #endif
        PropsModule.accepted = []
        PropsModule.table.withUnsafeBytes { NativeViews.install(table: $0.baseAddress!) }
        defer { NativeViews.uninstallTable() }
        let p = Presenter(), natives = NativeViews()
        natives.install(module: UnsafeMutableRawPointer(bitPattern: 1)!)
        func node(_ id: UInt32, _ props: String) -> NodeView {
            let view = NodeView(id: id, kind: "native", presenter: p)
            view.frame.size = CGSize(width: 100, height: 60)
            natives.create(owner: view)
            view.props = ["nativeViewModuleName": "props-probe", "nativeViewProps": props]
            natives.update(view)
            return view
        }
        func status(_ view: NodeView) -> [String: Any] {
            let tree = natives.decorate(["nodes": [["id": view.id, "type": "NativeView"]]])
            return ((tree["nodes"] as? [[String: Any]])?.first?["module"] as? [String: Any]) ?? [:]
        }
        // Count UTF8 bytes, including JSON syntax, rather than characters.
        let boundary = #"{"value":""# + String(repeating: "é", count: 32761) + #"aa"}"#
        XCTAssertEqual(boundary.utf8.count, 65536)
        let large = #"{"value":""# + String(repeating: "é", count: 32762) + #"a"}"#
        XCTAssertEqual(large.utf8.count, 65537)
        let refused = node(1, large)
        defer { natives.destroy(id: refused.id) }
        XCTAssertTrue(PropsModule.accepted.isEmpty)
        XCTAssertEqual(status(refused)["state"] as? String, "error")
        XCTAssertTrue((status(refused)["error"] as? String ?? "").contains("native-props-too-large"))
        let live = node(2, boundary)
        defer { natives.destroy(id: live.id) }
        XCTAssertEqual(PropsModule.accepted, [boundary])
        live.props["nativeViewProps"] = large
        natives.update(live)
        XCTAssertEqual(PropsModule.accepted, [boundary])
        XCTAssertTrue((status(live)["error"] as? String ?? "").contains("native-props-too-large"))
        live.props["nativeViewProps"] = boundary
        natives.update(live)
        XCTAssertEqual(status(live)["state"] as? String, "ready")
        XCTAssertEqual(PropsModule.accepted, [boundary])
        live.props["nativeViewProps"] = #"{"value":"recovered"}"#
        natives.update(live)
        XCTAssertEqual(PropsModule.accepted.last, #"{"value":"recovered"}"#)
        XCTAssertEqual(status(live)["state"] as? String, "ready")
        refused.props["nativeViewProps"] = "{}"
        natives.update(refused)
        XCTAssertEqual(PropsModule.accepted.last, "{}")
        XCTAssertEqual(status(refused)["state"] as? String, "ready")
    }

}

private enum PropsModule {
    final class Instance { let view = PropView() }
    nonisolated(unsafe) static var accepted: [String] = []
    static let roster = strdup(#"{"props-probe":{"snapshot":false}}"#)!
    static let table: [UInt8] = {
        var table = [UInt8](repeating: 0, count: 112)
        func put<T>(_ value: T, _ offset: Int) {
            withUnsafeBytes(of: value) { for (i, byte) in $0.enumerated() { table[offset + i] = byte } }
        }
        let create: @convention(c) (UnsafeMutableRawPointer?, UnsafePointer<UInt8>?, UInt32, UnsafePointer<UInt8>?, UInt32,
            UnsafeRawPointer?, UnsafeRawPointer?, UnsafeMutableRawPointer?, UInt32, UnsafeMutablePointer<UInt8>?, UInt32) -> UnsafeMutableRawPointer? = {
            _, _, _, json, count, _, _, _, _, _, _ in
            accepted.append(String(decoding: UnsafeBufferPointer(start: json, count: Int(count)), as: UTF8.self))
            return Unmanaged.passRetained(Instance()).toOpaque()
        }
        let view: @convention(c) (UnsafeMutableRawPointer?) -> UnsafeMutableRawPointer? = {
            Unmanaged.passUnretained(Unmanaged<Instance>.fromOpaque($0!).takeUnretainedValue().view).toOpaque()
        }
        let set: @convention(c) (UnsafeMutableRawPointer?, UnsafePointer<UInt8>?, UInt32, UnsafeMutablePointer<UInt8>?, UInt32) -> Int32 = {
            _, json, count, _, _ in
            accepted.append(String(decoding: UnsafeBufferPointer(start: json, count: Int(count)), as: UTF8.self)); return 0
        }
        let destroy: @convention(c) (UnsafeMutableRawPointer?) -> Void = { Unmanaged<Instance>.fromOpaque($0!).release() }
        let none: @convention(c) (UnsafeMutableRawPointer?) -> Void = { _ in }
        put(UInt32(3), 0); put(UInt32(112), 4); put(UnsafeRawPointer(roster), 8)
        put(unsafeBitCast(create, to: UnsafeRawPointer.self), 16)
        put(unsafeBitCast(view, to: UnsafeRawPointer.self), 24)
        put(unsafeBitCast(set, to: UnsafeRawPointer.self), 32)
        put(unsafeBitCast(destroy, to: UnsafeRawPointer.self), 48)
        for offset in [72, 80, 88, 96] { put(unsafeBitCast(none, to: UnsafeRawPointer.self), offset) }
        return table
    }()
}
