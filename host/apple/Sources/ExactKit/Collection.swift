// Host metadata and LE feedback for runner/src/instance/collection/api.rs.
// No record data, keys, height index, or row lifetime policy lives in Swift.
import Foundation

struct CollectionMeasurement: Equatable {
    var view: UInt32
    var epoch: UInt64
    var height: Double
}

struct CollectionFacts: Equatable {
    var top: Double
    var portWidth: Double
    var portHeight: Double
    var rowWidth: Double
    var measurements: [CollectionMeasurement]
    var focus: UInt32?
    var interaction: UInt32?

    /// Wire version 2: `velocity` (points/s, positive toward the end) and
    /// `limit`, the rows past what it owes this report may build (nil: any).
    func encode(view: UInt32, revision: UInt64, sequence: UInt64, velocity: Double = 0, limit: UInt32? = nil) -> Data {
        var bytes = Data()
        func integer<T: FixedWidthInteger>(_ value: T) {
            var le = value.littleEndian
            withUnsafeBytes(of: &le) { bytes.append(contentsOf: $0) }
        }
        integer(UInt32(2)); integer(view); integer(revision); integer(sequence)
        for value in [top, portWidth, portHeight, rowWidth] { integer(value.bitPattern) }
        integer(focus ?? 0); integer(interaction ?? 0)
        integer((velocity.isFinite ? velocity : 0).bitPattern); integer(limit.map { Swift.min($0, UInt32.max - 1) } ?? UInt32.max)
        integer(UInt32(measurements.count))
        for row in measurements { integer(row.view); integer(row.epoch); integer(row.height.bitPattern) }
        return bytes
    }
}

struct CollectionCursor {
    private(set) var sequence: UInt64 = 0
    private var correctedRevision: UInt64?
    mutating func advance() { if sequence < UInt64.max { sequence += 1 } }
    mutating func takeCorrection(revision: UInt64, sequence: UInt64) -> Bool {
        guard sequence == self.sequence, correctedRevision.map({ revision > $0 }) ?? true else { return false }
        correctedRevision = revision
        return true
    }
}

struct CollectionTurnBudget {
    private var passes = 0
    private var busy = false
    mutating func begin() -> Bool {
        guard !busy, passes < 2 else { return false }
        passes += 1; busy = true
        return true
    }
    mutating func end() { busy = false }
    mutating func nextTurn() { passes = 0 }
}

struct CollectionSnapshot {
    struct Row {
        let view: UInt32
        let root: UInt32
        let epoch: UInt64
        /// Logical position; -1 when a snapshot omits it.
        let index: Int
    }
    struct Correction {
        let sequence: UInt64
        let top: Double
    }
    let view: UInt32
    let revision: UInt64
    let sequence: UInt64
    let extent: Double
    let rows: [Row]
    let correction: Correction?
    /// Logical item count, mounted or not.
    let count: Int
    /// A limited report left window rows unbuilt: another report is owed.
    let pending: Bool

    init?(_ value: [String: Any]) {
        guard let view = Self.viewID(value["view"]), let revision = Self.uint(value["revision"]),
              let sequence = Self.uint(value["scrollSequence"]), let extent = Self.number(value["totalExtent"]),
              let rawRows = value["rows"] as? [[String: Any]] else { return nil }
        var seen = Set<UInt32>()
        var rows: [Row] = []
        for item in rawRows {
            guard let id = Self.viewID(item["view"]), let root = Self.viewID(item["root"]),
                  let epoch = Self.uint(item["epoch"]), seen.insert(id).inserted else { return nil }
            rows.append(Row(view: id, root: root, epoch: epoch, index: Self.uint(item["index"]).map { Int(clamping: $0) } ?? -1))
        }
        var correction: Correction?
        if let raw = value["correction"], !(raw is NSNull) {
            guard let raw = raw as? [String: Any], let seq = Self.uint(raw["scrollSequence"]),
                  let top = Self.number(raw["scrollTop"]) else { return nil }
            correction = Correction(sequence: seq, top: top)
        }
        self.view = view; self.revision = revision; self.sequence = sequence
        self.extent = extent; self.rows = rows; self.correction = correction
        self.count = Self.uint(value["count"]).map { Int(clamping: $0) } ?? rows.count
        self.pending = (value["pending"] as? NSNumber)?.boolValue ?? false
    }
    private static func uint(_ value: Any?) -> UInt64? {
        // Integral JSON numbers read directly; a fraction or sign refuses.
        if let n = value as? NSNumber, CFNumberIsFloatType(n) == false {
            return n.int64Value >= 0 ? n.uint64Value : nil
        }
        if let n = value as? NSNumber { return UInt64(n.stringValue) }
        if let s = value as? String { return UInt64(s) }
        return nil
    }
    private static func viewID(_ value: Any?) -> UInt32? {
        guard let n = uint(value), n > 0 else { return nil }
        return UInt32(exactly: n)
    }
    private static func number(_ value: Any?) -> Double? {
        guard let n = value as? NSNumber else { return nil }
        let d = n.doubleValue
        return d.isFinite && d >= 0 && d <= Double(Float.greatestFiniteMagnitude) ? d : nil
    }
}

/// At most one pending callback and two feedback calls per main-queue turn.
/// All caches are proportional to active collections and their mounted wrappers.
final class CollectionHost {
    final class Entry {
        var snapshot: CollectionSnapshot
        var cursor = CollectionCursor()
        var lastFacts: CollectionFacts?
        var lastSequence: UInt64?
        var port: [Double]?
        init(_ snapshot: CollectionSnapshot) { self.snapshot = snapshot }
    }
    weak var presenter: Presenter?
    var onFeedback: ((Data) -> Void)?
    var entries: [UInt32: Entry] = [:]
    var interaction: UInt32?
    var contactEvent: AnyObject?
    var batchDepth = 0
    var correcting = false
    private var dirty = Set<UInt32>()
    private var budget = CollectionTurnBudget()
    private var queued = false
    private var generation = 0
    private var contactSequence: UInt64 = 0
    private var gestureContact: UInt64?
    private var lastVisited: UInt32 = 0
    private var refreshPins = false
    /// LLP 1050.000's fill. A platform with a pump reports each moving
    /// collection's velocity (nil at rest) and builds `fillPending` in slices;
    /// one without leaves `motion` nil, and every report is unlimited.
    var motion: ((UInt32) -> Double?)?
    var requestFill: (() -> Void)?
    private(set) var fillPending = Set<UInt32>()
    private var sliceLimits: [UInt32: UInt32] = [:]
    // Platform hooks remove event monitors/recognizers when the adapter resets.
    var stopTracking: (() -> Void)?
    init(_ presenter: Presenter) { self.presenter = presenter }
    deinit { stopTracking?() }

    func reset() {
        gestureContact = nil
        generation += 1; queued = false; batchDepth = 0; correcting = false
        stopTracking?(); stopTracking = nil
        entries.removeAll(); dirty.removeAll(); interaction = nil; contactEvent = nil
        fillPending.removeAll(); sliceLimits.removeAll()
        budget = CollectionTurnBudget()
        refreshPins = false; lastVisited = 0
    }
    func beginBatch(_ batch: Batch) {
        batchDepth += 1
        for op in batch.ops where op.op == .collections {
            guard let items = op.payload["items"] as? [[String: Any]] else { continue }
            let snapshots = items.compactMap(CollectionSnapshot.init)
            guard snapshots.count == items.count else { continue }
            let live = Set(snapshots.map(\.view))
            guard live.count == snapshots.count else { continue }
            entries = entries.filter { live.contains($0.key) }
            dirty.formIntersection(live)
            fillPending.formIntersection(live)
            for snapshot in snapshots {
                if let entry = entries[snapshot.view] {
                    if snapshot.revision >= entry.snapshot.revision { entry.snapshot = snapshot }
                } else { entries[snapshot.view] = Entry(snapshot) }
                dirty.insert(snapshot.view)
                if entries[snapshot.view]!.snapshot.pending { fillPending.insert(snapshot.view) }
            }
            if !fillPending.isEmpty { requestFill?() }
        }
        if !entries.isEmpty, stopTracking == nil { startTracking() }
        if entries.isEmpty { stopTracking?(); stopTracking = nil; interaction = nil }
    }
    func endBatch() {
        // Frames/content have all landed before offsets change; platform methods
        // clamp against this coherent extent and suppress correction notifications.
        for (view, entry) in entries {
            guard let port = geometry(view) else { continue }
            let dimensions = [port.portWidth, port.portHeight, port.rowWidth]
            if let previous = entry.port, previous != dimensions { entry.cursor.advance() }
            entry.port = dimensions
            if let correction = entry.snapshot.correction,
               entry.cursor.takeCorrection(revision: entry.snapshot.revision, sequence: correction.sequence) {
                correcting = true
                correct(view, top: correction.top, extent: entry.snapshot.extent)
                correcting = false
            }
            dirty.insert(view)
        }
        batchDepth = max(0, batchDepth - 1)
    }
    /// The collection whose mounted row holds `descendant`: the nearest row
    /// wrapper at or above it names the owner, as the web host's `liveView`
    /// does, so a nested collection owns its descendants' pins and ancestors
    /// never pin the outer row as well. A view in no mounted row (a list
    /// itself, a spacer, the scroller a press landed on) pins nothing; the
    /// runner discards a report whose pin is outside its rows, and the window
    /// would stop following the scroll. Native container views may sit
    /// between nodes.
    func owningCollection(_ descendant: UInt32?) -> UInt32? {
        guard let descendant, let node = presenter?.views[descendant] else { return nil }
        func owner(_ node: NodeView) -> UInt32? {
            guard presenter?.views[node.id] === node else { return nil }
            return entries.first { $0.value.snapshot.rows.contains { $0.view == node.id } }?.key
        }
        if let id = owner(node) { return id }
        var parent = node.superview
        while let current = parent {
            if let node = current as? NodeView, let id = owner(node) { return id }
            parent = current.superview
        }
        return nil
    }
    func owns(_ view: UInt32) -> Bool { entries[view] != nil }
    func userIntent(_ view: UInt32) {
        guard let entry = entries[view], !correcting else { return }
        entry.cursor.advance(); dirty.insert(view)
        schedule()
    }
    func changed(_ view: UInt32, user: Bool = false) {
        guard let entry = entries[view], !correcting else { return }
        if user && batchDepth == 0 { entry.cursor.advance() }
        dirty.insert(view)
        guard batchDepth == 0 else { return }
        if !user { schedule(); return }
        // While mounted rows cover the port, the pump reports after the
        // scroll callback returns, building ahead in its slice; otherwise
        // the rows the reader sees are built now (LLP 1050.000 D1).
        if motion != nil, covers(view) {
            fillPending.insert(view)
            requestFill?()
        } else {
            // What shows is owed by estimated heights; two more rows cover
            // a port the estimates overstate (a jump into unmeasured rows).
            if motion != nil { sliceLimits[view] = 2 }
            flush()
            sliceLimits[view] = nil
        }
    }
    /// One slice of fill for `view`: a report that may build `limit` rows
    /// past what it owes, and its measurement pass. The slice is its own
    /// main-queue turn. Returns the rows it created.
    @discardableResult
    func fillSlice(_ view: UInt32, limit: UInt32) -> Int {
        fillPending.remove(view)
        guard let entry = entries[view], batchDepth == 0 else { return 0 }
        let before = Set(entry.snapshot.rows.map(\.view))
        budget.nextTurn()
        dirty.insert(view)
        sliceLimits[view] = limit
        flush()
        sliceLimits[view] = nil
        return (entries[view]?.snapshot.rows ?? []).filter { !before.contains($0.view) }.count
    }
    func dataReady() {
        // Retry armed edges once after deferred activation without forgetting
        // the accepted pin reservations used to order transfers.
        for entry in entries.values { entry.lastSequence = nil }
        dirty.formUnion(entries.keys)
        schedule()
    }
    func pinsChanged() {
        guard !entries.isEmpty else { return }
        dirty.formUnion(entries.keys)
        // Responder callbacks may precede AppKit/UIKit committing the new
        // first responder. Recheck once after that native event completes.
        refreshPins = true
        schedule()
        if batchDepth == 0 { flush() }
    }
    func pointer(_ view: UInt32?) {
        gestureContact = nil
        contactSequence &+= 1
        interaction = view
        pinsChanged()
    }
    func holdPointer(_ view: UInt32) -> UInt64 {
        pointer(view)
        gestureContact = contactSequence
        return contactSequence
    }
    func releaseInteractionLater(ifCurrent expected: UInt64? = nil) {
        if let expected {
            guard contactSequence == expected else { return }
            gestureContact = nil
        } else if gestureContact == contactSequence { return }
        let captured = generation
        let prior = contactSequence
        DispatchQueue.main.async { [weak self] in
            guard let self, generation == captured, contactSequence == prior else { return }
            pointer(nil)
        }
    }

    func flush() {
        guard batchDepth == 0, let onFeedback, !dirty.isEmpty else { return }
        // Reserve the continuation before calling Rust: its batch can reenter us.
        schedule()
        while !dirty.isEmpty {
            guard budget.begin() else { return }
            let focus = focusedView()
            let focusOwner = owningCollection(focus)
            let interactionOwner = owningCollection(interaction)
            // Retire old owners before publishing replacement pins, even when
            // the two-pass budget carries the new owner to a later callback.
            let retiring = dirty.filter { id in
                guard let facts = entries[id]?.lastFacts else { return false }
                return (facts.focus != nil && focusOwner != id) ||
                    (facts.interaction != nil && interactionOwner != id)
            }
            let candidates = retiring.isEmpty ? dirty : retiring
            // Round-robin keeps one refining viewport from starving another.
            let id = candidates.filter { $0 > lastVisited }.min() ?? candidates.min()!
            lastVisited = id
            dirty.remove(id)
            // Cached geometry permits a hidden previous owner to release its
            // pin. Measurements below still come only from visible live rows.
            if let entry = entries[id], var facts = geometry(id) ?? entry.lastFacts {
                facts.focus = focusOwner == id ? focus : nil
                facts.interaction = interactionOwner == id ? interaction : nil
                if !retiring.isEmpty {
                    // Focus and interaction can swap owners together. Clearing
                    // one old pin must not publish the other new pin early.
                    if facts.focus != entry.lastFacts?.focus { facts.focus = nil }
                    if facts.interaction != entry.lastFacts?.interaction { facts.interaction = nil }
                    if (focusOwner == id && facts.focus != focus) ||
                       (interactionOwner == id && facts.interaction != interaction) { dirty.insert(id) }
                }
                facts.measurements = entry.snapshot.rows.compactMap { row in
                    guard let height = height(row.view), height.isFinite, height >= 0,
                          rowWidth(row.view) == facts.rowWidth else { return nil }
                    return CollectionMeasurement(view: row.view, epoch: row.epoch, height: height)
                }
                // A slice builds its limit once; later passes and every
                // report while moving or owed a continuation only measure
                // and rescue what shows. At rest a report is unlimited.
                let velocity = motion?(id)
                let limit: UInt32? = sliceLimits.removeValue(forKey: id)
                    ?? (motion != nil && (velocity != nil || entry.snapshot.pending) ? 0 : nil)
                if entry.lastFacts != facts || entry.lastSequence != entry.cursor.sequence
                    || (entry.snapshot.pending && limit != 0) {
                    entry.lastFacts = facts
                    entry.lastSequence = entry.cursor.sequence
                    onFeedback(facts.encode(view: id, revision: entry.snapshot.revision, sequence: entry.cursor.sequence,
                        velocity: velocity ?? 0, limit: limit))
                }
            }
            budget.end()
        }
    }
    private func schedule() {
        guard !queued else { return }
        queued = true
        let captured = generation
        DispatchQueue.main.async { [weak self] in
            guard let self, generation == captured else { return }
            queued = false
            budget.nextTurn()
            if refreshPins { refreshPins = false; dirty.formUnion(entries.keys) }
            flush()
        }
    }
}
