// The touch runner (LLP 1080.000 D1): one XCTest UI test that stays alive
// across agent sessions and taps the device's screen, so a touch comes from
// the HID system and reaches the app through UIKit's own hit-testing and
// gesture recognizers. It connects out to the driver
// (`host/apple/touches.mjs`) with the token it was launched with, says the
// loopback port it listens on, and answers JSON lines until the driver hangs
// up; then it waits on that port for the next session, which opens with
// `{"token":T,"app":bundle id}`, and ends after ten idle minutes or `quit`.
//   {"op":"tap","point":[x,y],"screen":[x,y]?}
//                                a tap at a point in the app's scene, points
//                                → {"done":true,"injected":{"start":ms,"end":ms,"via":…}}
//   {"op":"drag","point":[x,y],"to":[x,y],"press":s,"velocity":v,"hold":s}
//                                one whole gesture (LLP 1080.000 §11): press,
//                                one straight drag at `v` points per second
//                                (the header says pixels; on the simulator a
//                                drag at v covers v points a second, measured
//                                at 3x), hold, lift; `to` equal to `point` is
//                                a still press for `press + hold`
//                                → {"done":true,"injected":{"start":ms,"end":ms,"via":…}}
//   {"op":"foreground"}          → {"state":"runningForeground"|…}
//   {"op":"orientation","to":O}  turns the device (portrait, portraitUpsideDown,
//                                landscapeLeft, landscapeRight) → {"device":O}
//   {"op":"quit"}                ends the test
// A tap's `screen`, the same point in the screen's fixed, portrait space,
// sends it through XCTest's own event synthesizer (`Synthesizer`); without
// it, or when that SPI is missing, refuses or does not complete, the public
// `XCUICoordinate` call makes it. A drag
// is always the public call's: a synthesized one reached the app with its
// moves batched, faster than asked, and flung further than a finger would.
// It never launches, activates or terminates the app: the driver launched it.
import XCTest
import UIKit
import Foundation

final class ExactTouches: XCTestCase {
    func testDrive() throws {
        let env = ProcessInfo.processInfo.environment
        guard let endpoint = env["EXACT_TOUCH_CONNECT"], let token = env["EXACT_TOUCH_TOKEN"],
              var app = env["EXACT_TOUCH_APP"] else { XCTFail("EXACT_TOUCH_CONNECT, EXACT_TOUCH_TOKEN and EXACT_TOUCH_APP are required"); return }
        let parts = endpoint.split(separator: ":")
        guard parts.count == 2, let port = UInt16(parts[1]) else { XCTFail("bad endpoint \(endpoint)"); return }
        signal(SIGPIPE, SIG_IGN) // a driver that hung up mid-reply ends its session, not the runner
        let listener = try Listener()
        var link: Link? = try Link(host: String(parts[0]), port: port)
        link?.send(["ready": true, "token": token, "pid": getpid(), "listen": Int(listener.port)])
        while true {
            guard let session = link else {
                // The next session's first line is the token and its app.
                guard let next = listener.accept(idle: 600) else { return }
                guard let line = next.next(), let hello = (try? JSONSerialization.jsonObject(with: Data(line.utf8))) as? [String: Any],
                      hello["token"] as? String == token, let named = hello["app"] as? String else { next.close(); continue }
                app = named
                next.send(["ready": true, "token": token, "pid": getpid(), "listen": Int(listener.port)])
                link = next
                continue
            }
            guard let line = session.next() else { session.close(); link = nil; continue }
            guard let req = (try? JSONSerialization.jsonObject(with: Data(line.utf8))) as? [String: Any], let op = req["op"] as? String else {
                session.send(["error": "unreadable request"]); continue
            }
            // Fresh each request: the app is launched after the runner starts.
            let target = XCUIApplication(bundleIdentifier: app)
            switch op {
            case "foreground":
                session.send(["state": Self.name(target.state)])
            case "tap":
                guard let p = req["point"] as? [Double], p.count == 2, p.allSatisfy(\.isFinite) else { session.send(["error": "tap needs a finite point"]); continue }
                guard target.state == .runningForeground else { session.send(["error": "the app is \(Self.name(target.state)), not in the foreground"]); continue }
                let start = Date().timeIntervalSince1970 * 1000
                var via = "synthesizer"
                // The public tap when the SPI is missing, refuses or never completes: a synthesized touch that did land
                // after all makes two, which the driver's barrier refuses as ambiguous rather than passing.
                if (req["screen"] as? [Double]).flatMap({ $0.count == 2 && $0.allSatisfy(\.isFinite) ? CGPoint(x: $0[0], y: $0[1]) : nil })
                    .flatMap(Synthesizer.tap) != true {
                    via = "coordinate"
                    target.coordinate(withNormalizedOffset: .zero).withOffset(CGVector(dx: p[0], dy: p[1])).tap()
                }
                session.send(["done": true, "injected": ["start": start, "end": Date().timeIntervalSince1970 * 1000, "via": via]])
            case "drag":
                guard let p = req["point"] as? [Double], p.count == 2, let q = req["to"] as? [Double], q.count == 2, (p + q).allSatisfy(\.isFinite),
                      let press = req["press"] as? Double, let hold = req["hold"] as? Double, let velocity = req["velocity"] as? Double,
                      [press, hold, velocity].allSatisfy({ $0.isFinite && $0 >= 0 }) else { session.send(["error": "drag needs finite points and non-negative press, hold and velocity"]); continue }
                guard target.state == .runningForeground else { session.send(["error": "the app is \(Self.name(target.state)), not in the foreground"]); continue }
                let zero = target.coordinate(withNormalizedOffset: .zero)
                let from = zero.withOffset(CGVector(dx: p[0], dy: p[1]))
                let start = Date().timeIntervalSince1970 * 1000
                if p == q { from.press(forDuration: press + hold) }
                else { from.press(forDuration: press, thenDragTo: zero.withOffset(CGVector(dx: q[0], dy: q[1])), withVelocity: XCUIGestureVelocity(velocity), thenHoldForDuration: hold) }
                session.send(["done": true, "injected": ["start": start, "end": Date().timeIntervalSince1970 * 1000, "via": "coordinate"]])
            case "orientation":
                let names: [String: UIDeviceOrientation] = ["portrait": .portrait, "portraitUpsideDown": .portraitUpsideDown, "landscapeLeft": .landscapeLeft, "landscapeRight": .landscapeRight]
                guard let to = (req["to"] as? String).flatMap({ names[$0] }) else { session.send(["error": "orientation: one of \(names.keys.sorted())"]); continue }
                XCUIDevice.shared.orientation = to
                session.send(["device": names.first { $0.value == XCUIDevice.shared.orientation }?.key ?? "other"])
            case "quit":
                session.send(["done": true]); session.close(); return
            default:
                session.send(["error": "unknown op \(op)"])
            }
        }
    }

    static func name(_ s: XCUIApplication.State) -> String {
        switch s {
        case .runningForeground: return "runningForeground"
        case .runningBackground: return "runningBackground"
        case .runningBackgroundSuspended: return "runningBackgroundSuspended"
        case .notRunning: return "notRunning"
        default: return "unknown"
        }
    }
}

/// XCTest's own event synthesizer, the SPI WebDriverAgent drives
/// (`XCSynthesizedEventRecord`, `XCPointerEventPath`, `XCUIDevice
/// eventSynthesizer`; LLP 1080.000 §1, P3): a tap injected as the public
/// call injects it, without their wait for the app to go idle and
/// their spacing between gestures, which cost 0.4–2 s a tap (§9, P2). The
/// driver's dispatch-log barrier still checks every touch landed.
enum Synthesizer {
    private typealias Alloc = @convention(c) (AnyClass, Selector) -> AnyObject
    private typealias InitPath = @convention(c) (AnyObject, Selector, CGPoint, Double) -> AnyObject
    private typealias Lift = @convention(c) (AnyObject, Selector, Double) -> Void
    private typealias InitRecord = @convention(c) (AnyObject, Selector, NSString, Int) -> AnyObject
    private typealias Add = @convention(c) (AnyObject, Selector, AnyObject) -> Void
    private typealias Get = @convention(c) (AnyObject, Selector) -> AnyObject?
    private typealias Synthesize = @convention(c) (AnyObject, Selector, AnyObject, @escaping @convention(block) (Bool, NSError?) -> Void) -> Void

    private static func imp<T>(_ cls: AnyClass?, _ name: String, _ type: T.Type, classMethod: Bool = false) -> (Selector, T)? {
        let sel = NSSelectorFromString(name)
        guard let cls, let method = classMethod ? class_getClassMethod(cls, sel) : class_getInstanceMethod(cls, sel) else { return nil }
        return (sel, unsafeBitCast(method_getImplementation(method), to: type))
    }

    /// A touch down at `at`, in portrait screen points, lifted 50 ms later.
    /// True once injected; false when refused or not completed in 2 s; nil
    /// when this Xcode has no such SPI.
    static func tap(_ at: CGPoint) -> Bool? {
        let pathClass: AnyClass? = NSClassFromString("XCPointerEventPath"), recordClass: AnyClass? = NSClassFromString("XCSynthesizedEventRecord")
        guard let alloc = imp(pathClass, "alloc", Alloc.self, classMethod: true), let recordAlloc = imp(recordClass, "alloc", Alloc.self, classMethod: true),
              let initPath = imp(pathClass, "initForTouchAtPoint:offset:", InitPath.self),
              let up = imp(pathClass, "liftUpAtOffset:", Lift.self), let initRecord = imp(recordClass, "initWithName:interfaceOrientation:", InitRecord.self),
              let add = imp(recordClass, "addPointerEventPath:", Add.self), let synthesizer = imp(XCUIDevice.self, "eventSynthesizer", Get.self),
              let device = synthesizer.1(XCUIDevice.shared, synthesizer.0) else { return nil }
        let syn = NSSelectorFromString("synthesizeEvent:completion:")
        guard let method = class_getInstanceMethod(object_getClass(device), syn) else { return nil }
        let path = initPath.1(alloc.1(pathClass!, alloc.0), initPath.0, at, 0)
        up.1(path, up.0, 0.05)
        let record = initRecord.1(recordAlloc.1(recordClass!, recordAlloc.0), initRecord.0, "exact" as NSString, UIInterfaceOrientation.portrait.rawValue)
        add.1(record, add.0, path)
        var result: Bool?
        unsafeBitCast(method_getImplementation(method), to: Synthesize.self)(device, syn, record) { ok, _ in DispatchQueue.main.async { result = ok } }
        // The completion comes once the tap has been played (0.3 s when it works).
        let deadline = Date().addingTimeInterval(2)
        while result == nil, Date() < deadline { RunLoop.current.run(mode: .default, before: Date().addingTimeInterval(0.005)) }
        return result ?? false
    }
}

/// The loopback port the next session connects to.
final class Listener {
    let fd: Int32
    let port: UInt16
    init() throws {
        let fd = socket(AF_INET, SOCK_STREAM, 0)
        self.fd = fd
        var addr = sockaddr_in()
        addr.sin_family = sa_family_t(AF_INET)
        addr.sin_port = 0
        inet_pton(AF_INET, "127.0.0.1", &addr.sin_addr)
        var len = socklen_t(MemoryLayout<sockaddr_in>.size)
        let bound = withUnsafeMutablePointer(to: &addr) { a in
            a.withMemoryRebound(to: sockaddr.self, capacity: 1) { bind(fd, $0, len) == 0 && listen(fd, 4) == 0 && getsockname(fd, $0, &len) == 0 }
        }
        if !bound { throw NSError(domain: "ExactTouches", code: Int(errno), userInfo: [NSLocalizedDescriptionKey: "listen on loopback failed"]) }
        port = UInt16(bigEndian: addr.sin_port)
    }
    /// The next connection, or nil after `idle` seconds without one.
    func accept(idle: Int32) -> Link? {
        var p = pollfd(fd: fd, events: Int16(POLLIN), revents: 0)
        guard poll(&p, 1, idle * 1000) > 0 else { return nil }
        let c = Darwin.accept(fd, nil, nil)
        return c >= 0 ? Link(fd: c) : nil
    }
}

/// One TCP connection, JSON lines both ways, blocking: the test's thread is
/// the main thread and does nothing else.
final class Link {
    let fd: Int32
    var buffer = Data()
    init(fd: Int32) { self.fd = fd }
    init(host: String, port: UInt16) throws {
        fd = socket(AF_INET, SOCK_STREAM, 0)
        var addr = sockaddr_in()
        addr.sin_family = sa_family_t(AF_INET)
        addr.sin_port = port.bigEndian
        inet_pton(AF_INET, host, &addr.sin_addr)
        let ok = withUnsafePointer(to: &addr) { $0.withMemoryRebound(to: sockaddr.self, capacity: 1) { connect(fd, $0, socklen_t(MemoryLayout<sockaddr_in>.size)) } }
        if ok != 0 { throw NSError(domain: "ExactTouches", code: Int(errno), userInfo: [NSLocalizedDescriptionKey: "connect \(host):\(port) failed"]) }
    }
    func send(_ object: [String: Any]) {
        guard var data = try? JSONSerialization.data(withJSONObject: object) else { return }
        data.append(0x0a)
        data.withUnsafeBytes { _ = write(fd, $0.baseAddress, data.count) }
    }
    func close() { Darwin.close(fd) }
    /// The next line, or nil when the driver hung up.
    func next() -> String? {
        while true {
            if let i = buffer.firstIndex(of: 0x0a) {
                let line = String(decoding: buffer[buffer.startIndex..<i], as: UTF8.self)
                buffer.removeSubrange(buffer.startIndex...i)
                return line
            }
            var chunk = [UInt8](repeating: 0, count: 4096)
            let n = read(fd, &chunk, chunk.count)
            if n <= 0 { return nil }
            buffer.append(contentsOf: chunk[0..<n])
        }
    }
}
