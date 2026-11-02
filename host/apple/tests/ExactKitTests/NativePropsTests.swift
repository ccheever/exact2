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
    func testLargeAggregatesReachCreationAndUpdatesOnEitherSideOfThePaintGate() {
        #if os(macOS)
        _ = NSApplication.shared
        #endif
        PropsModule.table.withUnsafeBytes { NativeViews.install(table: $0.baseAddress!) }
        defer { NativeViews.uninstallTable() }
        let p = Presenter()
        let props = #"{"pins":""# + String(repeating: "p", count: 69989) + #""}"#
        XCTAssertEqual(props.utf8.count, 70000)
        for gateOpen in [true, false] {
            PropsModule.accepted = []
            let natives = NativeViews()
            natives.install(module: UnsafeMutableRawPointer(bitPattern: 1)!, gateOpen: gateOpen)
            natives.firstFrameTags = ["props-probe"]
            let owner = NodeView(id: gateOpen ? 1 : 2, kind: "native", presenter: p)
            owner.frame.size = CGSize(width: 100, height: 60)
            natives.create(owner: owner)
            defer { natives.destroy(id: owner.id) }
            owner.props = ["nativeViewModuleName": "props-probe", "nativeViewProps": props]
            natives.update(owner)
            XCTAssertEqual(PropsModule.accepted.count, 1)
            XCTAssertTrue(PropsModule.accepted.last == props)
            owner.props["nativeViewProps"] = "{}"
            natives.update(owner)
            XCTAssertEqual(PropsModule.accepted.count, 2)
            XCTAssertEqual(PropsModule.accepted.last, "{}")
            owner.props["nativeViewProps"] = props
            natives.update(owner)
            XCTAssertEqual(PropsModule.accepted.count, 3)
            XCTAssertTrue(PropsModule.accepted.last == props)
        }
    }
}

private enum PropsModule {
    final class Instance { let view = PropView() }
    nonisolated(unsafe) static var accepted: [String] = []
    static let roster = strdup(#"{"props-probe":{"snapshot":false,"firstFrame":true}}"#)!
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
