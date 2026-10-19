// SpringBoard archive kit (LLP 1126 D14; approved apparatus).
import XCTest
import Foundation
import Darwin
final class ArchiveDriver: XCTestCase {
    func testDrive() throws {
        continueAfterFailure = true
        let fd = socket(AF_INET, SOCK_STREAM, 0)
        var address = sockaddr_in()
        address.sin_len = UInt8(MemoryLayout<sockaddr_in>.size)
        address.sin_family = sa_family_t(AF_INET)
        address.sin_port = UInt16(ProcessInfo.processInfo.environment["PROBE_PORT"]!)!.bigEndian
        inet_pton(AF_INET, "127.0.0.1", &address.sin_addr)
        let status = withUnsafePointer(to: &address) { ptr in ptr.withMemoryRebound(to: sockaddr.self, capacity: 1) { Darwin.connect(fd, $0, socklen_t(MemoryLayout<sockaddr_in>.size)) } }
        XCTAssertEqual(status, 0)
        func send(_ value: Any) {
            var data = try! JSONSerialization.data(withJSONObject: value)
            data.append(10)
            data.withUnsafeBytes { bytes in _ = Darwin.write(fd, bytes.baseAddress, bytes.count) }
        }
        send(["role": "runner", "pid": getpid()])
        let spring = XCUIApplication(bundleIdentifier: "com.apple.springboard")
        let app = XCUIApplication(bundleIdentifier: ProcessInfo.processInfo.environment["PROBE_APP_ID"] ?? "com.exact.probe.archive")
        func press(_ element: XCUIElement) throws {
            guard element.exists else { throw NSError(domain: "ProbeControl", code: 1, userInfo: [NSLocalizedDescriptionKey: "Element missing: \(element)"]) }
            element.tap()
        }
        let zero = spring.coordinate(withNormalizedOffset: .zero)
        var buffer = Data(), byte: UInt8 = 0
        while Darwin.read(fd, &byte, 1) == 1 {
            if byte != 10 { buffer.append(byte); continue }
            let steps = try JSONSerialization.jsonObject(with: buffer) as! [[String: Any]]
            buffer.removeAll()
            let start = Date()
            var extras: [String: Any] = [:]
            do { for step in steps {
                print("STEP \(step)")
                let xy = step["xy"] as? [Double] ?? []
                switch step["op"] as? String {
                case "lock":
                    let selector = NSSelectorFromString("pressLockButton")
                    guard XCUIDevice.shared.responds(to: selector) else { throw NSError(domain: "ArchiveControl", code: 2, userInfo: [NSLocalizedDescriptionKey: "XCTest has no pressLockButton"]) }
                    _ = XCUIDevice.shared.perform(selector)
                case "find":
                    let element = spring.descendants(matching: .any).matching(NSPredicate(format: "label == %@", step["label"] as! String)).firstMatch
                    if element.exists {
                        let frame = element.frame
                        extras["frame"] = [frame.minX, frame.minY, frame.width, frame.height]
                    }
                case "home": XCUIDevice.shared.press(.home)
                case "app": app.activate()
                case "tap": zero.withOffset(CGVector(dx: xy[0], dy: xy[1])).tap()
                case "hold": zero.withOffset(CGVector(dx: xy[0], dy: xy[1])).press(forDuration: step["seconds"] as? Double ?? 1.2)
                case "swipe": zero.withOffset(CGVector(dx: xy[0], dy: xy[1])).press(forDuration: 0.1,
                    thenDragTo: zero.withOffset(CGVector(dx: xy[2], dy: xy[3])))
                case "button": try press(spring.buttons.matching(NSPredicate(format: "label CONTAINS %@", step["label"] as! String)).firstMatch)
                case "appbutton": try press(app.buttons.matching(NSPredicate(format: "label CONTAINS %@", step["label"] as! String)).firstMatch)
                case "text": try press(spring.staticTexts.matching(NSPredicate(format: "label CONTAINS %@", step["label"] as! String)).firstMatch)
                case "search": let search = spring.searchFields.firstMatch; search.tap(); search.typeText(step["text"] as! String)
                case "voiceover":
                    let service = XCUIDevice.shared.voiceOverService
                    var spoken: [String] = []
                    do {
                        try service.enable()
                        defer { try? service.disable() }
                        spoken.append(try service.currentSpeech().utterance)
                        for _ in 0..<12 {
                            let output = try service.moveForward().utterance
                            spoken.append(output)
                            if output.contains("Exact preserves") {
                                if output.hasPrefix("Archive Probe") { spoken.append(try service.moveIn().utterance) }
                                if spoken.last!.contains("ending.") { break }
                            }
                            if output.contains("Archive Probe") && output.contains("Widget") {
                                spoken.append(try service.moveIn().utterance)
                                if spoken.last!.contains("Exact preserves") { break }
                            }
                        }
                    } catch { extras["voiceOverError"] = String(describing: error); try? service.disable() }
                    extras["utterances"] = spoken
                case "status": extras["status"] = app.staticTexts["status"].label
                default: break
                }
            }} catch { extras["error"] = String(describing: error) }
            send(["seconds": Date().timeIntervalSince(start), "tree": spring.debugDescription, "extras": extras])
        }
        close(fd)
    }
}
