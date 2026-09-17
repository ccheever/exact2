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

    func encode(view: UInt32, revision: UInt64, sequence: UInt64) -> Data {
        var bytes = Data()
        func integer<T: FixedWidthInteger>(_ value: T) {
            var le = value.littleEndian
            withUnsafeBytes(of: &le) { bytes.append(contentsOf: $0) }
        }
        integer(UInt32(1)); integer(view); integer(revision); integer(sequence)
        for value in [top, portWidth, portHeight, rowWidth] { integer(value.bitPattern) }
        integer(focus ?? 0); integer(interaction ?? 0); integer(UInt32(measurements.count))
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

    init?(_ value: [String: Any]) {
        guard let view = Self.viewID(value["view"]), let revision = Self.uint(value["revision"]),
              let sequence = Self.uint(value["scrollSequence"]), let extent = Self.number(value["totalExtent"]),
              let rawRows = value["rows"] as? [[String: Any]] else { return nil }
        var seen = Set<UInt32>()
        var rows: [Row] = []
        for item in rawRows {
            guard let id = Self.viewID(item["view"]), let root = Self.viewID(item["root"]),
                  let epoch = Self.uint(item["epoch"]), seen.insert(id).inserted else { return nil }
            rows.append(Row(view: id, root: root, epoch: epoch))
        }
        var correction: Correction?
        if let raw = value["correction"], !(raw is NSNull) {
            guard let raw = raw as? [String: Any], let seq = Self.uint(raw["scrollSequence"]),
                  let top = Self.number(raw["scrollTop"]) else { return nil }
            correction = Correction(sequence: seq, top: top)
        }
        self.view = view; self.revision = revision; self.sequence = sequence
        self.extent = extent; self.rows = rows; self.correction = correction
    }
    private static func uint(_ value: Any?) -> UInt64? {
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
    // Platform hooks remove event monitors/recognizers when the adapter resets.
    var stopTracking: (() -> Void)?
    init(_ presenter: Presenter) { self.presenter = presenter }
    deinit { stopTracking?() }

    func reset() {
        gestureContact = nil
        generation += 1; queued = false; batchDepth = 0; correcting = false
        stopTracking?(); stopTracking = nil
        entries.removeAll(); dirty.removeAll(); interaction = nil; contactEvent = nil
        budget = CollectionTurnBudget()
        refreshPins = false; lastVisited = 0
    }
    func beginBatch(_ batch: Batch) {
        batchDepth += 1
        for op in batch.ops where op["op"] as? String == "collections" {
            guard let items = op["items"] as? [[String: Any]] else { continue }
            let snapshots = items.compactMap(CollectionSnapshot.init)
            guard snapshots.count == items.count else { continue }
            let live = Set(snapshots.map(\.view))
            guard live.count == snapshots.count else { continue }
            entries = entries.filter { live.contains($0.key) }
            dirty.formIntersection(live)
            for snapshot in snapshots {
                if let entry = entries[snapshot.view] {
                    if snapshot.revision >= entry.snapshot.revision { entry.snapshot = snapshot }
                } else { entries[snapshot.view] = Entry(snapshot) }
                dirty.insert(snapshot.view)
            }
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
    /// A nested collection owns its descendants' pins; ancestors must not
    /// pin the outer row as well. Native container views may sit between nodes.
    func owningCollection(_ descendant: UInt32?) -> UInt32? {
        guard let descendant, let node = presenter?.views[descendant] else { return nil }
        if entries[descendant] != nil { return descendant }
        var parent = node.superview
        while let current = parent {
            if let node = current as? NodeView, entries[node.id] != nil,
               presenter?.views[node.id] === node { return node.id }
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
        if batchDepth == 0 { if user { flush() } else { schedule() } }
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
                if entry.lastFacts != facts || entry.lastSequence != entry.cursor.sequence {
                    entry.lastFacts = facts
                    entry.lastSequence = entry.cursor.sequence
                    onFeedback(facts.encode(view: id, revision: entry.snapshot.revision, sequence: entry.cursor.sequence))
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
