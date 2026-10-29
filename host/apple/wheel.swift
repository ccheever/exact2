// The macOS driver's trackpad (LLP 1142 D3): a gesture's scroll-wheel events
// posted to one process through the window server, paced by a CVDisplayLink
// on the target display, so AppKit tracks the gesture and runs its scrolling,
// momentum and rubber band. `scripts/agent-wheel.mjs` builds and runs it with
// one JSON argument, what the app's agent answered (`AgentMac.swift`):
//   {"pid":P,"window":W,"point":[x,y],"screen":[x,y],"mark":M,
//    "form":"fling"|"hold"|"release","wheel":[dx,dy]}
// `point` is in the window from its top left, `screen` in the display from
// the main screen's top left, and `wheel` is the finger's travel in the web's
// sign (a positive dy scrolls down). It prints {"posted":n,"periodMs":p}.
//   fling   began, the travel over 100 ms, a zero-delta ended, then momentum
//           from the finger's last velocity, decaying by 0.95 a period, ended
//           under half a point or after 2 s
//   hold    began and the travel over 100 ms, the gesture left open
//   release the zero-delta ended that lifts a held gesture
// Posting needs the Accessibility grant of the terminal the driver runs in.
import CoreGraphics
import CoreVideo
import Foundation

struct Request: Decodable {
    let pid: Int32, window: Int, point: [Double], screen: [Double], mark: Int64, form: String, wheel: [Double]
}

func fail(_ why: String) -> Never {
    FileHandle.standardError.write(Data((why + "\n").utf8))
    exit(1)
}

guard CommandLine.arguments.count == 2, let request = try? JSONDecoder().decode(Request.self, from: Data(CommandLine.arguments[1].utf8)) else {
    fail("usage: exact-wheel '<json>'")
}
guard CGPreflightPostEventAccess() else {
    fail("this terminal may not post events: allow it under System Settings › Privacy & Security › Accessibility, then run the drive again")
}
// Undocumented, as the agent's own wheel uses them (`AgentMouseMac.swift`):
// without a window and a point in it, the window server gives a posted
// scroll event the pointer's position, outside the app's window.
let windowField = CGEventField(rawValue: 51)!
typealias PlaceInWindow = @convention(c) (CGEvent, CGPoint) -> Void
guard let place = dlsym(UnsafeMutableRawPointer(bitPattern: -2), "CGEventSetWindowLocation").map({ unsafeBitCast($0, to: PlaceInWindow.self) }) else {
    fail("CGEventSetWindowLocation is not in this macOS")
}

var display: CGDirectDisplayID = 0
var count: UInt32 = 0
guard CGGetDisplaysWithPoint(CGPoint(x: request.screen[0], y: request.screen[1]), 1, &display, &count) == .success, count == 1 else {
    fail("no display under the wheel's screen point")
}
var created: CVDisplayLink?
guard CVDisplayLinkCreateWithCGDisplay(display, &created) == kCVReturnSuccess, let link = created else {
    fail("cannot create the wheel's display link")
}

final class Pacing {
    let lock = NSLock()
    let delivered = DispatchSemaphore(value: 0)
    let measured = DispatchSemaphore(value: 0)
    var pending: CGEvent?
    var firstTick: Double?
    var intervals = 0
    var period = 0.0

    func tick(_ link: CVDisplayLink, pid: Int32) {
        let now = Double(CVGetCurrentHostTime()) / CVGetHostClockFrequency()
        lock.lock()
        defer { lock.unlock() }
        if firstTick == nil { firstTick = now } else { intervals += 1 }
        let previous = period
        // Report Core Video's actual output period, or the mean callback interval
        // if it is unavailable (never the display mode's nominal rate).
        let actual = CVDisplayLinkGetActualOutputVideoRefreshPeriod(link)
        if actual.isFinite && actual > 0 {
            period = actual
        } else if intervals > 0 {
            period = (now - firstTick!) / Double(intervals)
        }
        if previous == 0 && period > 0 { measured.signal() }
        if let event = pending {
            pending = nil
            event.postToPid(pid)
            delivered.signal()
        }
    }
}
let pacing = Pacing()
guard CVDisplayLinkSetOutputHandler(link, { link, _, _, _, _ in
    pacing.tick(link, pid: request.pid)
    return kCVReturnSuccess
}) == kCVReturnSuccess else {
    fail("cannot set the wheel's display callback")
}
let nominal = CVDisplayLinkGetNominalOutputVideoRefreshPeriod(link)
guard nominal.timeValue > 0, nominal.timeScale > 0 else {
    fail("the target display has no refresh period")
}
let period = Double(nominal.timeValue) / Double(nominal.timeScale)
var running = false
let source = CGEventSource(stateID: .hidSystemState)
var posted = 0

// CGScrollPhase: began 1, changed 2, ended 4, mayBegin 128. CGMomentumScrollPhase: begin 1, continue 2, end 3.
func post(_ dx: Double, _ dy: Double, phase: Int64 = 0, momentum: Int64 = 0) {
    guard let e = CGEvent(scrollWheelEvent2Source: source, units: .pixel, wheelCount: 2, wheel1: Int32((-dy).rounded()), wheel2: Int32((-dx).rounded()), wheel3: 0) else {
        fail("no scroll event")
    }
    e.location = CGPoint(x: request.screen[0], y: request.screen[1])
    e.setIntegerValueField(windowField, value: Int64(request.window))
    place(e, CGPoint(x: request.point[0], y: request.point[1]))
    e.setIntegerValueField(.scrollWheelEventIsContinuous, value: 1)
    e.setDoubleValueField(.scrollWheelEventPointDeltaAxis1, value: -dy)
    e.setDoubleValueField(.scrollWheelEventPointDeltaAxis2, value: -dx)
    e.setDoubleValueField(.scrollWheelEventFixedPtDeltaAxis1, value: -dy)
    e.setDoubleValueField(.scrollWheelEventFixedPtDeltaAxis2, value: -dx)
    e.setIntegerValueField(.scrollWheelEventScrollPhase, value: phase)
    e.setIntegerValueField(.scrollWheelEventMomentumPhase, value: momentum)
    e.setIntegerValueField(.eventSourceUserData, value: request.mark)
    pacing.lock.lock()
    pacing.pending = e
    pacing.lock.unlock()
    // Queue the first event before starting. Only the callback posts, at most
    // once per tick; a delayed producer cannot accumulate catch-up posts.
    if !running {
        guard CVDisplayLinkStart(link) == kCVReturnSuccess else {
            fail("cannot start the wheel's display link")
        }
        running = true
    }
    guard pacing.delivered.wait(timeout: .now() + 3) == .success else {
        CVDisplayLinkStop(link)
        fail("the wheel's display callback stopped")
    }
    posted += 1
}

let (dx, dy) = (request.wheel[0], request.wheel[1])
switch request.form {
case "fling", "hold":
    post(0, 0, phase: 128)
    pacing.lock.lock()
    let travelPeriod = pacing.period > 0 ? pacing.period : period
    pacing.lock.unlock()
    let steps = max(2, Int((0.1 / travelPeriod).rounded()))
    post(dx / Double(steps), dy / Double(steps), phase: 1)
    for _ in 1..<steps { post(dx / Double(steps), dy / Double(steps), phase: 2) }
    guard request.form == "fling" else { break }
    post(0, 0, phase: 4)
    var (vx, vy) = (dx / Double(steps), dy / Double(steps))
    post(vx, vy, momentum: 1)
    let end = Date().addingTimeInterval(2)
    while max(abs(vx), abs(vy)) >= 0.5, Date() < end {
        vx *= 0.95; vy *= 0.95
        post(vx, vy, momentum: 2)
    }
    post(0, 0, momentum: 3)
case "release":
    post(0, 0, phase: 4)
default:
    fail("form is fling, hold or release, not \(request.form)")
}
// A one-event release may need a second callback to measure its fallback period.
guard pacing.measured.wait(timeout: .now() + 3) == .success else {
    CVDisplayLinkStop(link)
    fail("the wheel's display link supplied no measured period")
}
CVDisplayLinkStop(link)
pacing.lock.lock()
let measuredPeriod = pacing.period
pacing.lock.unlock()
print(#"{"posted":\#(posted),"periodMs":\#((measuredPeriod * 1000 * 100).rounded() / 100)}"#)
