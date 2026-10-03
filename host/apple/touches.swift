// The touch runner (LLP 1080.000 D1): one XCTest UI test that stays alive
// for an agent session and taps the device's screen with public
// `XCUICoordinate`, so a touch comes from the HID system and reaches the app
// through UIKit's own hit-testing and gesture recognizers. It connects out
// to the driver (`host/apple/touches.mjs`) with the token it was launched
// with, then answers JSON lines until the driver hangs up:
//   {"op":"tap","point":[x,y]}   a tap at a point in the app's scene, points
//                                → {"done":true,"injected":{"start":ms,"end":ms}}
//   {"op":"foreground"}          → {"state":"runningForeground"|…}
//   {"op":"orientation","to":O}  turns the device (portrait, portraitUpsideDown,
//                                landscapeLeft, landscapeRight) → {"device":O}
// It never launches, activates or terminates the app: the driver launched it.
import XCTest
import UIKit
import Foundation

final class ExactTouches: XCTestCase {
    func testDrive() throws {
        let env = ProcessInfo.processInfo.environment
        guard let endpoint = env["EXACT_TOUCH_CONNECT"], let token = env["EXACT_TOUCH_TOKEN"],
              let app = env["EXACT_TOUCH_APP"] else { XCTFail("EXACT_TOUCH_CONNECT, EXACT_TOUCH_TOKEN and EXACT_TOUCH_APP are required"); return }
        let parts = endpoint.split(separator: ":")
        guard parts.count == 2, let port = UInt16(parts[1]) else { XCTFail("bad endpoint \(endpoint)"); return }
        let link = try Link(host: String(parts[0]), port: port)
        link.send(["ready": true, "token": token, "pid": getpid()])
        while let line = link.next() {
            guard let req = (try? JSONSerialization.jsonObject(with: Data(line.utf8))) as? [String: Any], let op = req["op"] as? String else {
                link.send(["error": "unreadable request"]); continue
            }
            // Fresh each request: the app is launched after the runner starts.
            let target = XCUIApplication(bundleIdentifier: app)
            switch op {
            case "foreground":
                link.send(["state": Self.name(target.state)])
            case "tap":
                guard let p = req["point"] as? [Double], p.count == 2, p.allSatisfy(\.isFinite) else { link.send(["error": "tap needs a finite point"]); continue }
                guard target.state == .runningForeground else { link.send(["error": "the app is \(Self.name(target.state)), not in the foreground"]); continue }
                let start = Date().timeIntervalSince1970 * 1000
                target.coordinate(withNormalizedOffset: .zero).withOffset(CGVector(dx: p[0], dy: p[1])).tap()
                link.send(["done": true, "injected": ["start": start, "end": Date().timeIntervalSince1970 * 1000]])
            case "orientation":
                let names: [String: UIDeviceOrientation] = ["portrait": .portrait, "portraitUpsideDown": .portraitUpsideDown, "landscapeLeft": .landscapeLeft, "landscapeRight": .landscapeRight]
                guard let to = (req["to"] as? String).flatMap({ names[$0] }) else { link.send(["error": "orientation: one of \(names.keys.sorted())"]); continue }
                XCUIDevice.shared.orientation = to
                link.send(["device": names.first { $0.value == XCUIDevice.shared.orientation }?.key ?? "other"])
            default:
                link.send(["error": "unknown op \(op)"])
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

/// One TCP connection, JSON lines both ways, blocking: the test's thread is
/// the main thread and does nothing else.
final class Link {
    let fd: Int32
    var buffer = Data()
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
