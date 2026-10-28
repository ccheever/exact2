// The Apple implementation's hub fed recorded readings (LLP 1133.031 §12
// stage 2): compiled with Device.swift by apple.mjs and run on this Mac. It
// checks what a drive cannot make the device do on cue.
//
// - A level change within a hundredth announces nothing; one past it
//   announces once.
// - A thermal change announces once.
// - A foreground after a change announces once, and one with nothing
//   changed announces nothing.
// - Two sessions share one observer; it stops when the last one leaves.
// - A read before the network's first path waits for it.
import ExactPackage
import Foundation

nonisolated(unsafe) var failures = 0
func check(_ name: String, _ ok: Bool, _ detail: @autoclosure () -> String = "") {
    if !ok { failures += 1 }
    print("\(ok ? "PASS" : "FAIL") apple-hub \(name)\(ok ? "" : ": \(detail())")")
}

/// A platform that answers what the suite last set, and counts its starts and stops.
final class Recorded {
    var now: [String: [String: Any]] = [:]
    var heard: [String: ([String: Any]) -> Void] = [:]
    var starts: [String: Int] = [:], stops: [String: Int] = [:]
    var platform: DeviceHub.Platform {
        DeviceHub.Platform(
            start: { [self] source, heard in starts[source, default: 0] += 1; self.heard[source] = heard },
            stop: { [self] source in stops[source, default: 0] += 1; heard[source] = nil },
            read: { [self] source in now[source] },
            foreground: {})
    }
    /// The platform changes and says so, as a notification would.
    func post(_ source: String, _ answer: [String: Any]) {
        now[source] = answer
        heard[source]?(answer)
    }
}

@main
struct HubSuite {
    static func main() {
        let recorded = Recorded()
        let hub = DeviceHub(platform: recorded.platform)
        var announced: [String] = []
        let a = { (topic: String) in announced.append("a \(topic)") }
        let b = { (topic: String) in announced.append("b \(topic)") }
        var answers: [[String: Any]] = []
        let keep = { (answer: [String: Any]) in answers.append(answer) }

        // The battery: rounded to hundredths before it is compared.
        recorded.now["getBattery"] = DeviceAnswers.battery(level: 0.5, state: "unplugged")
        hub.read("getBattery", session: "1", changed: a, reply: keep)
        check("the first read answers the level in hundredths", answers.last?["level"] as? Double == 0.5 && answers.last?["charging"] as? Bool == false, "\(answers)")
        recorded.post("getBattery", DeviceAnswers.battery(level: 0.504, state: "unplugged"))
        check("a level change within a hundredth announces nothing", announced.isEmpty, "\(announced)")
        recorded.post("getBattery", DeviceAnswers.battery(level: 0.51, state: "unplugged"))
        check("a level change past a hundredth announces once", announced == ["a device.getBattery"], "\(announced)")
        recorded.post("getBattery", DeviceAnswers.battery(level: 0.51, state: "charging"))
        check("plugged in, charging announces", announced.count == 2 && (DeviceAnswers.battery(level: 0.51, state: "full")["charging"] as? Bool) == true, "\(announced)")
        check("an unknown state answers the specification's full battery", (DeviceAnswers.battery(level: -1, state: "unknown")["level"] as? Double) == 1)

        // The power state: a thermal change announces once.
        announced = []
        recorded.now["powerState"] = DeviceAnswers.power(lowPower: false, thermal: "nominal")
        hub.read("powerState", session: "1", changed: a, reply: keep)
        recorded.post("powerState", DeviceAnswers.power(lowPower: false, thermal: "serious"))
        recorded.post("powerState", DeviceAnswers.power(lowPower: false, thermal: "serious"))
        check("a thermal change announces once", announced == ["a device.powerState"], "\(announced)")

        // The foreground reads again: a change missed while suspended announces once.
        announced = []
        recorded.now["powerState"] = DeviceAnswers.power(lowPower: true, thermal: "serious")
        hub.foreground()
        check("a foreground after a change announces once", announced == ["a device.powerState"], "\(announced)")
        hub.foreground()
        check("a foreground with nothing changed announces nothing", announced == ["a device.powerState"], "\(announced)")

        // Two sessions, one observer.
        announced = []
        hub.read("powerState", session: "2", changed: b, reply: keep)
        check("a second session's read starts no second observer", recorded.starts["powerState"] == 1, "\(recorded.starts)")
        recorded.post("powerState", DeviceAnswers.power(lowPower: false, thermal: "fair"))
        check("a change announces to each session", Set(announced) == ["a device.powerState", "b device.powerState"], "\(announced)")
        hub.leave("powerState", session: "1")
        check("one session's release keeps the observer", recorded.stops["powerState"] == nil, "\(recorded.stops)")
        hub.leave(nil, session: "2")
        check("the last session's end stops it", recorded.stops["powerState"] == 1, "\(recorded.stops)")

        // The network: a read before the first path waits for it.
        answers = []
        hub.read("connection", session: "1", changed: a, reply: keep)
        check("a read before the first path waits", answers.isEmpty, "\(answers)")
        recorded.post("connection", DeviceAnswers.network(satisfied: true, kinds: [("utun4", "other"), ("en0", "wifi")], constrained: true, expensive: false))
        check("the first path answers it: a tunnel's Wi-Fi is wifi, Low Data Mode is saveData", answers.last?["type"] as? String == "wifi" && answers.last?["saveData"] as? Bool == true, "\(answers)")
        check("no path is none", DeviceAnswers.network(satisfied: false, kinds: [], constrained: false, expensive: false)["type"] as? String == "none")
        check("a tunnel alone is other", DeviceAnswers.network(satisfied: true, kinds: [("utun0", "other")], constrained: false, expensive: false)["type"] as? String == "other")

        print(failures == 0 ? "all passed" : "\(failures) failed")
        exit(failures == 0 ? 0 : 1)
    }
}
