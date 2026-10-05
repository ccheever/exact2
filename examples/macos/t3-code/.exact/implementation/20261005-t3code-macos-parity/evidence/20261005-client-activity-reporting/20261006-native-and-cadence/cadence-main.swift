import Foundation
import AppKit
let start = Date()
let reporter = T3ActivityReporter(persistent: false, dataDirectory: nil, observeWindows: false)
let a = UUID(), b = UUID(), scope = UUID()
let lock = NSLock()
var records: [[String: Any]] = []
func sender(_ payload: [String: Any], _ finished: @escaping () -> Void) {
    var row = payload
    row["elapsed"] = Date().timeIntervalSince(start)
    lock.lock(); records.append(row); lock.unlock()
    print(String(data: try! JSONSerialization.data(withJSONObject: row, options: [.sortedKeys]), encoding: .utf8)!)
    fflush(stdout)
    finished()
}
reporter.connect(a, environment: "environment:a", send: sender)
reporter.connect(b, environment: "environment:b", send: sender)
DispatchQueue.main.asyncAfter(deadline: .now() + 2) { reporter.retain(scope, environment: "environment:a", method: "subscribeVcsStatus", payload: ["cwd": "/repo:delimiter"]) }
DispatchQueue.main.asyncAfter(deadline: .now() + 3) { reporter.release(scope) }
DispatchQueue.main.asyncAfter(deadline: .now() + 4) { reporter.disconnect(b) }
DispatchQueue.main.asyncAfter(deadline: .now() + 5) { reporter.connect(b, environment: "environment:b", send: sender) }
DispatchQueue.main.asyncAfter(deadline: .now() + 6) { reporter.disconnect(b) }
DispatchQueue.main.asyncAfter(deadline: .now() + 80) {
    lock.lock(); let all = records; lock.unlock()
    let ar = all.filter { ($0["environmentId"] as? String) == "environment:a" }
    func has(_ second: Double) -> Bool { ar.contains { abs(($0["elapsed"] as! Double) - second - 0.25) < 0.4 } }
    let valid = [0.0, 25, 50, 75].allSatisfy(has)
      && !all.contains { ($0["environmentId"] as? String) == "environment:b" && ($0["elapsed"] as! Double) > 6 }
      && ar.contains { ($0["elapsed"] as! Double) > 45 && ($0["recentlyInteracted"] as? Bool) == false }
      && ar.contains { ($0["scopes"] as! [[String: Any]]).count == 2 }
      && ar.filter { ($0["elapsed"] as! Double) > 3.1 }.allSatisfy { ($0["scopes"] as! [[String: Any]]).count == 1 }
    print("ASSERTIONS cadence25s scope-release disconnected-environment interaction-expiry: \(valid ? "PASS" : "FAIL")")
    reporter.destroy(); exit(valid ? 0 : 1)
}
RunLoop.main.run()
