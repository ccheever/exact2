import QuartzCore
import UIKit

/// One observation. Times are seconds since the recorder started, on the `CACurrentMediaTime()` clock.
struct Event: Encodable {
  var t: Double
  var kind: String
  /// The model time of the UI update in progress (`UIUpdateInfo.modelTime`), or nil outside one
  var update: Double?
  /// The main run loop's mode when the event was logged
  var mode: String?
  var scroll: ScrollState?
  var value: Double?
  var note: String?
}

struct ScrollState: Encodable {
  var tracking: Bool
  var dragging: Bool
  var decelerating: Bool
  var offset: Double

  init(_ scrollView: UIScrollView) {
    tracking = scrollView.isTracking
    dragging = scrollView.isDragging
    decelerating = scrollView.isDecelerating
    offset = scrollView.contentOffset.y
  }
}

/// Keeps the in-memory event log and writes it, with its summary, into the app's Documents directory
@MainActor
final class Recorder {
  static let shared = Recorder()

  let origin = CACurrentMediaTime()
  private(set) var events: [Event] = []
  /// The view whose UI update `log` reports
  weak var view: UIView?

  /// Logs an event. `lookup` false skips asking UIKit for the update in progress and the run loop's mode, to tell
  /// whether asking has side effects.
  func log(_ kind: String, update: Double? = nil, scroll: UIScrollView? = nil, value: Double? = nil, note: String? = nil, lookup: Bool = true) {
    let now = CACurrentMediaTime()
    let update = update ?? (lookup ? view.flatMap { UIUpdateInfo.current(for: $0)?.modelTime } : nil)
    events.append(Event(
      t: now - origin,
      kind: kind,
      update: update.map { $0 - origin },
      mode: lookup ? RunLoop.main.currentMode?.rawValue : nil,
      scroll: scroll.map(ScrollState.init),
      value: value,
      note: note
    ))
  }

  /// Writes `<base>.json` (environment and summary) and `<base>.events.json` (the raw log)
  func write(base: String, scenario: Scenario, repetition: Int, environment: [String: String]) throws {
    let directory = try FileManager.default.url(for: .documentDirectory, in: .userDomainMask, appropriateFor: nil, create: true)
    let report = Report(
      scenario: scenario.rawValue,
      repetition: repetition,
      environment: environment,
      summary: Summary(events: events, measuredKinds: scenario.measuredKinds)
    )
    let pretty = JSONEncoder()
    pretty.outputFormatting = [.prettyPrinted, .sortedKeys]
    try pretty.encode(report).write(to: directory.appendingPathComponent("\(base).json"))
    let compact = JSONEncoder()
    compact.outputFormatting = [.sortedKeys]
    try compact.encode(events).write(to: directory.appendingPathComponent("\(base).events.json"))
  }
}

struct Report: Encodable {
  var scenario: String
  var repetition: Int
  var environment: [String: String]
  var summary: Summary
}

/// Darwin notifications between the app and the UI test runner, which share the simulator's notify center and
/// neither of which has to query the other (an accessibility query would add main-thread work to the app)
enum Signal {
  static let prefix = "dev.exact.timer-waits."

  static func post(_ name: String) {
    let center = CFNotificationCenterGetDarwinNotifyCenter()
    CFNotificationCenterPostNotification(center, CFNotificationName((prefix + name) as CFString), nil, nil, true)
  }

  @MainActor
  static func observe(_ name: String, _ handler: @escaping @MainActor () -> Void) {
    handlers[prefix + name] = handler
    let center = CFNotificationCenterGetDarwinNotifyCenter()
    CFNotificationCenterAddObserver(center, nil, { _, _, name, _, _ in
      guard let name = name?.rawValue as String? else {
        return
      }
      DispatchQueue.main.async {
        MainActor.assumeIsolated { Signal.handlers[name]?() }
      }
    }, (prefix + name) as CFString, nil, .deliverImmediately)
  }

  @MainActor private static var handlers: [String: @MainActor () -> Void] = [:]
}
