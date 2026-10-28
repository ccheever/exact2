import Foundation

/// Counts of the measured callbacks in each phase of a run, and the order of events within UI updates
struct Summary: Encodable {
  var phases: [PhaseSummary]
  /// The order of events in each UI update, for the `updateLink` scenario's touch and scroll phases
  var ordering: [String: Ordering]?

  init(events: [Event], measuredKinds: [String]) {
    let windows = Self.windows(events)
    phases = windows.map { PhaseSummary(window: $0, events: events, measuredKinds: measuredKinds) }
    if measuredKinds.contains("link.afterUpdateScheduled") {
      let names = ["heldDrag.moving", "heldDrag.held", "fling.moving", "fling.deceleration"]
      ordering = Dictionary(uniqueKeysWithValues: windows.filter { names.contains($0.name) }.map { ($0.name, Ordering(window: $0, events: events)) })
    }
  }

  /// Finds the phases from the script's marks, the test's gesture labels, touches and scroll callbacks
  static func windows(_ events: [Event]) -> [Window] {
    func first(_ kind: String, from start: Double = -.infinity, to end: Double = .infinity, where test: (Event) -> Bool = { _ in true }) -> Event? {
      events.first { $0.kind == kind && $0.t >= start && $0.t < end && test($0) }
    }
    var windows: [Window] = []
    func add(_ name: String, _ start: Double?, _ end: Double?) {
      if let start, let end, end >= start {
        windows.append(Window(name: name, start: start, end: end))
      }
    }

    let stillEnd = first("still.end")?.t
    let animateEnd = first("animate.end")?.t
    let animatorBegin = first("animator.begin")?.t
    let animatorEnd = first("animator.end")?.t
    add("still", first("still.begin")?.t, stillEnd)
    add("animate", first("animate.begin")?.t, animateEnd)
    add("animate.tail", animateEnd, animatorBegin)
    add("animator", animatorBegin, animatorEnd)
    add("animator.tail", animatorEnd, first("segment.begin")?.t ?? first("scripted")?.t)
    // A segment runs until the next starts, 500 ms after its last firing, so it holds any update that firing caused
    for begin in events where begin.kind == "segment.begin" {
      let next = events.first { ($0.kind == "segment.begin" || $0.kind == "scripted") && $0.t > begin.t }
      add("segment.\(begin.note ?? "")", begin.t, next?.t)
    }

    let marks = events.filter { $0.kind == "gesture" }
    let finish = first("dump")?.t ?? events.last?.t ?? 0
    for (index, mark) in marks.enumerated() {
      let name = mark.note ?? "gesture"
      let end = index + 1 < marks.count ? marks[index + 1].t : finish
      guard let down = first("touch.began", from: mark.t, to: end), let up = first("touch.ended", from: down.t, to: end) else {
        continue
      }
      let lastMove = events.last { $0.kind == "touch.moved" && $0.t > down.t && $0.t < up.t }?.t ?? down.t
      add("\(name).moving", down.t, lastMove)
      add("\(name).held", lastMove, up.t)
      var motionEnd = up.t
      if let release = first("didEndDragging", from: up.t, to: end, where: { $0.value == 1 }) {
        let stop = first("didEndDecelerating", from: release.t, to: end)?.t
        add("\(name).deceleration", release.t, stop ?? end)
        motionEnd = stop ?? end
      }
      add("\(name).tail", motionEnd, first("settled", from: motionEnd, to: end)?.t)
      if let background = first("didEnterBackground", from: mark.t, to: end) {
        add("\(name).background", background.t, first("willEnterForeground", from: background.t, to: end)?.t)
      }
    }
    return windows
  }
}

struct Window {
  var name: String
  var start: Double
  var end: Double

  func contains(_ event: Event) -> Bool {
    event.t >= start && event.t < end
  }
}

struct PhaseSummary: Encodable {
  var name: String
  var start: Double
  var duration: Double
  var counts: [String: Int]
  var ratesHz: [String: Double]
  /// Median interval between consecutive events of a kind, for kinds with at least three events
  var medianIntervalMs: [String: Double]
  /// Offsets from the phase's start, for kinds with at most 12 events, to show when sparse callbacks happen
  var offsetsMs: [String: [Double]]
  /// The main run loop's mode at each event, by kind
  var modes: [String: [String: Int]]

  init(window: Window, events: [Event], measuredKinds: [String]) {
    name = window.name
    start = window.start
    duration = window.end - window.start
    let inside = events.filter(window.contains)
    counts = [:]
    ratesHz = [:]
    medianIntervalMs = [:]
    offsetsMs = [:]
    modes = [:]
    for kind in measuredKinds {
      let times = inside.filter { $0.kind == kind }.map(\.t)
      counts[kind] = times.count
      if duration > 0 {
        ratesHz[kind] = (Double(times.count) / duration).rounded(places: 1)
      }
      if times.count >= 3 {
        let intervals = zip(times.dropFirst(), times).map { ($0 - $1) * 1000 }.sorted()
        medianIntervalMs[kind] = intervals[intervals.count / 2].rounded(places: 2)
      }
      if !times.isEmpty, times.count <= 12 {
        offsetsMs[kind] = times.map { (($0 - window.start) * 1000).rounded(places: 1) }
      }
      let byMode = Dictionary(grouping: inside.filter { $0.kind == kind }, by: { $0.mode ?? "none" }).mapValues(\.count)
      if !byMode.isEmpty {
        modes[kind] = byMode
      }
    }
  }
}

/// How touches and scroll callbacks fall relative to the passive link's phases. Events group into UI updates by
/// `UIUpdateInfo.modelTime`; an event logged outside any update has no model time.
struct Ordering: Encodable {
  var updates: Int
  var updatesWithTouch: Int
  /// Updates with a touch in which `afterUpdateScheduled` ran before the first touch
  var afterUpdateScheduledBeforeFirstTouch: Int
  var updatesWithScroll: Int
  var afterUpdateScheduledBeforeFirstScroll: Int
  var touchesOutsideUpdates: Int
  var scrollsOutsideUpdates: Int
  /// The most common sequences of event kinds within one update, with their counts
  var commonSequences: [String: Int]

  init(window: Window, events: [Event]) {
    let relevant = events.filter { $0.kind.hasPrefix("link.") || $0.kind.hasPrefix("touch.") || $0.kind.hasPrefix("pan.") || $0.kind == "scroll" }
    let inside = relevant.filter(window.contains)
    touchesOutsideUpdates = inside.filter { $0.kind.hasPrefix("touch.") && $0.update == nil }.count
    scrollsOutsideUpdates = inside.filter { $0.kind == "scroll" && $0.update == nil }.count
    // Take whole updates, including their events just outside the window, so a window's edge does not cut one
    let touched = Set(inside.compactMap(\.update))
    let groups = Dictionary(grouping: relevant.filter { $0.update.map(touched.contains) ?? false }, by: { $0.update! }).values.map { $0.sorted { $0.t < $1.t } }
    updates = groups.count
    func precedes(_ prefix: String, in group: [Event]) -> Bool? {
      guard let target = group.firstIndex(where: { $0.kind.hasPrefix(prefix) }) else {
        return nil
      }
      guard let scheduled = group.firstIndex(where: { $0.kind == "link.afterUpdateScheduled" }) else {
        return false
      }
      return scheduled < target
    }
    let touch = groups.compactMap { precedes("touch.", in: $0) }
    updatesWithTouch = touch.count
    afterUpdateScheduledBeforeFirstTouch = touch.filter { $0 }.count
    let scroll = groups.compactMap { precedes("scroll", in: $0) }
    updatesWithScroll = scroll.count
    afterUpdateScheduledBeforeFirstScroll = scroll.filter { $0 }.count
    let sequences = Dictionary(grouping: groups.map { $0.map(\.kind).joined(separator: " > ") }, by: { $0 }).mapValues(\.count)
    commonSequences = Dictionary(uniqueKeysWithValues: sequences.sorted { $0.value > $1.value }.prefix(4).map { ($0.key, $0.value) })
  }
}

private extension Double {
  func rounded(places: Int) -> Double {
    let scale = pow(10, Double(places))
    return (self * scale).rounded() / scale
  }
}
