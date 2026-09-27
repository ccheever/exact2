import Foundation

final class Fixture: ExactNativeModule {
  let grants: String
  var data = ""
  var pending: ((Result<[String: Any], Error>) -> Void)?
  init(_ grants: String) { self.grants = grants; probe.live += 1 }
  deinit { probe.live -= 1 }
  func configure(data: URL, cache: URL, temporary: URL) { self.data = data.path }
  func call(_ request: [String: Any]) throws -> [String: Any] {
    probe.lock.lock(); probe.active += 1; probe.peak = max(probe.peak, probe.active); probe.lock.unlock()
    Thread.sleep(forTimeInterval: 0.005)
    probe.lock.lock(); probe.active -= 1; probe.lock.unlock()
    return ["data": data, "grants": grants]
  }
  func later(_ request: [String: Any], done: @escaping (Result<[String: Any], Error>) -> Void) {
    switch request["op"] as? String {
    case "twice": done(.success([:])); done(.success([:]))
    case "race": DispatchQueue.concurrentPerform(iterations: 8) { _ in done(.success([:])) }
    case "drop": break
    case "retain": pending = done
    default: done(Result { try call(request) })
    }
  }
}

final class Probe {
  let lock = NSLock()
  var active = 0, peak = 0, live = 0
}
let probe = Probe()
func exactNativeModule(grants: String) -> ExactNativeModule { Fixture(grants) }
var failures = 0
func check(_ condition: Bool, _ message: String) {
  if !condition { failures += 1; print("FAIL: \(message)") }
}
final class Completion {
  let lock = NSLock()
  var answered = 0, aborted = 0
  var context: UnsafeMutableRawPointer { Unmanaged.passUnretained(self).toOpaque() }
}
func completed(_ context: UnsafeMutableRawPointer?, _ reply: UnsafePointer<CChar>?, _ failed: Int32) {
  let count = Unmanaged<Completion>.fromOpaque(context!).takeUnretainedValue()
  count.lock.lock(); count.answered += 1; count.lock.unlock()
}
func discarded(_ context: UnsafeMutableRawPointer?) {
  let count = Unmanaged<Completion>.fromOpaque(context!).takeUnretainedValue()
  count.lock.lock(); count.aborted += 1; count.lock.unlock()
}
func create(_ grants: String) -> UnsafeMutableRawPointer {
  Array(grants.utf8).withUnsafeBufferPointer { exactNativeCreate($0.baseAddress!, $0.count) }
}
func call(_ instance: UnsafeMutableRawPointer) -> [String: Any] {
  var failed: Int32 = 0
  let reply = exactNativeCall(instance, "{}", &failed)!
  defer { exactNativeFree(reply) }
  check(failed == 0, "call succeeds")
  return try! JSONSerialization.jsonObject(with: Data(String(cString: reply).utf8)) as! [String: Any]
}
let first = create("fs.read app:/data"), second = create("fs.write app:/data")
for op in ["twice", "race", "drop"] {
  let count = Completion()
  exactNativeLater(first, "{\"op\":\"\(op)\"}", count.context, completed, discarded)
  check(count.answered == (op == "drop" ? 0 : 1), "\(op): done is one-shot")
  check(count.aborted == (op == "drop" ? 1 : 0), "\(op): only an unanswered drop aborts")
}
DispatchQueue.concurrentPerform(iterations: 20) { index in
  if index % 2 == 0 { _ = call(first) }
  else {
    let count = Completion()
    exactNativeLater(first, "{}", count.context, completed, discarded)
  }
}
check(probe.peak == 1, "call and later serialize (peak \(probe.peak))")
exactNativeConfigure(first, "/session-a", "/cache-a", "/tmp-a")
exactNativeConfigure(second, "/session-b", "/cache-b", "/tmp-b")
check(call(first)["data"] as? String == "/session-a", "A keeps its directory after B configures")
check(call(second)["data"] as? String == "/session-b", "B has its directory")
check(call(first)["grants"] as? String == "fs.read app:/data", "A receives its effective grants")
check(call(second)["grants"] as? String == "fs.write app:/data", "B receives its effective grants")

let a = UnsafeMutablePointer<Int>.allocate(capacity: 1), b = UnsafeMutablePointer<Int>.allocate(capacity: 1)
a.initialize(to: 0); b.initialize(to: 0)
func announced(_ context: UnsafeMutableRawPointer?, _ topic: UnsafePointer<CChar>) {
  context!.assumingMemoryBound(to: Int.self).pointee += 1
}
exactNativeListen(first, a, announced)
exactNativeListen(second, b, announced)
ExactNative.changed("meter")
check(a.pointee == 1 && b.pointee == 1, "both sessions hear announcements")
exactNativeUnlisten(first)
ExactNative.changed("meter")
check(a.pointee == 1 && b.pointee == 2, "unlisten leaves B live")
let pending = Completion()
exactNativeLater(first, "{\"op\":\"retain\"}", pending.context, completed, discarded)
check(pending.aborted == 0, "retained completion stays pending")
exactNativeDestroy(first)
check(pending.aborted == 1, "teardown drops an unanswered retained completion")
exactNativeDestroy(second)
ExactNative.changed("meter")
check(b.pointee == 2, "destroy unregisters B")
for _ in 0..<100 {
  let handle = create("fs.read app:/data")
  exactNativeListen(handle, a, announced)
  exactNativeDestroy(handle)
}
ExactNative.changed("meter")
check(a.pointee == 1 && probe.live == 0, "reloads release every module and listener")
a.deallocate(); b.deallocate()
print("Swift native bridge: \(failures) failure(s)")
exit(failures == 0 ? 0 : 1)
