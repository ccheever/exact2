// Host metadata and LE feedback for runner/src/instance/collection/api.rs.
// No record data, keys, height index, or row lifetime policy lives in Swift.
import Foundation

struct CollectionMeasurement: Equatable {
    var view: UInt32
    var epoch: UInt64
    /// The wrapper's border-box size on the list's main axis.
    var size: Double
}

/// A list's port on its own axes (LLP 1070 H1): `offset` along the main
/// axis, the port's main and cross sizes, and the rows' cross size.
struct CollectionFacts: Equatable {
    var offset: Double
    var portMain: Double
    var portCross: Double
    var cross: Double
    var measurements: [CollectionMeasurement]
    var focus: UInt32?
    var interaction: UInt32?

    /// Wire version 3: `velocity` (points/s, positive toward the end),
    /// `limit`, the rows past what it owes this report may build (nil: any),
    /// and whether an enclosing list is moving (LLP 1070 F2).
    /// `createOnly` and `noBuild` split a moving list's report around the
    /// owner thread (LLP 1072 §5): build only, off main; then retire only.
    func encode(view: UInt32, revision: UInt64, sequence: UInt64, velocity: Double = 0, limit: UInt32? = nil, ancestorMoving: Bool = false, createOnly: Bool = false, noBuild: Bool = false) -> Data {
        var bytes = Data()
        func integer<T: FixedWidthInteger>(_ value: T) {
            var le = value.littleEndian
            withUnsafeBytes(of: &le) { bytes.append(contentsOf: $0) }
        }
        integer(UInt32(3)); integer(view); integer(revision); integer(sequence)
        for value in [offset, portMain, portCross, cross] { integer(value.bitPattern) }
        integer(focus ?? 0); integer(interaction ?? 0)
        integer((velocity.isFinite ? velocity : 0).bitPattern); integer(limit.map { Swift.min($0, UInt32.max - 1) } ?? UInt32.max)
        integer(UInt32(ancestorMoving ? 1 : 0) | UInt32(createOnly ? 2 : 0) | UInt32(noBuild ? 4 : 0))
        integer(UInt32(measurements.count))
        for row in measurements { integer(row.view); integer(row.epoch); integer(row.size.bitPattern) }
        return bytes
    }
}

struct CollectionCursor {
    private(set) var sequence: UInt64 = 0
    private var correctedRevision: UInt64?
    mutating func advance() { if sequence < UInt64.max { sequence += 1 } }
    /// The sequence of the last move that was not the reader's travel (an
    /// authored offset, a port resize): a relative correction from before
    /// it is stale, as an absolute one is.
    private(set) var jumpedAt: UInt64 = 0
    mutating func jump() { advance(); jumpedAt = sequence }
    mutating func takeCorrection(revision: UInt64, sequence: UInt64) -> Bool {
        guard sequence == self.sequence, correctedRevision.map({ revision > $0 }) ?? true else { return false }
        correctedRevision = revision
        return true
    }
    /// The correction an anchor's (`from`) still owes, once per revision:
    /// a relative move, whatever the port did since. A later revision
    /// carrying the same anchor's correction owes only what it adds.
    private var shifted: (sequence: UInt64, from: Double, offset: Double)?
    /// `jumpedBefore`: where `jumpedAt` was before a port resize in this
    /// same batch; a correction planned since then still lands.
    mutating func takeShift(revision: UInt64, _ c: CollectionSnapshot.Correction, jumpedBefore: UInt64? = nil) -> Double? {
        guard let from = c.from, c.sequence >= (jumpedBefore ?? jumpedAt),
              correctedRevision.map({ revision > $0 }) ?? true else { return nil }
        correctedRevision = revision
        let done = shifted.flatMap { $0.sequence == c.sequence && $0.from == from ? $0.offset : nil } ?? from
        shifted = (c.sequence, from, c.offset)
        return c.offset - done
    }
}

struct CollectionTurnBudget {
    private var passes = 0
    private var busy = false
    /// Passes refused, by why: the turn had spent its two (`spent`), or a
    /// pass was running and a report's batch reentered (`busy`).
    private(set) var refusedSpent = 0, refusedBusy = 0
    /// A rescue (what shows is uncovered, LLP 1050.000 D1) is never refused
    /// for the turn's passes: a turn that spent them on other lists, or on
    /// a slice, would otherwise leave the port blank until the next slice.
    mutating func begin(rescue: Bool = false) -> Bool {
        if busy { refusedBusy += 1; return false }
        if passes >= 2 && !rescue { refusedSpent += 1; return false }
        passes += 1; busy = true
        return true
    }
    /// A pass that sent no report (an unchanged list, visited because a
    /// batch dirtied every list) is given back: with nested lists most
    /// visits are those, and they would spend the pass a slice needs.
    mutating func end(reported: Bool = true) {
        busy = false
        if !reported && passes > 0 { passes -= 1 }
    }
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
        let offset: Double
        /// An anchor's correction: the offset it was taken at (relative).
        var from: Double? = nil
        /// Animate there (LLP 1070.000 §6.2): a smooth `scrollIntoView`, or
        /// a `scroll-behavior: smooth` list following its end.
        var smooth = false
    }
    let view: UInt32
    /// The main axis (LLP 1070 H1): a row list scrolls on x, and `extent`,
    /// a correction's offset and every measured size run along it.
    let horizontal: Bool
    /// The outer list whose mounted row holds this one (LLP 1070 N1).
    let parent: UInt32?
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
                  let offset = Self.number(raw["offset"]) else { return nil }
            correction = Correction(sequence: seq, offset: offset, from: Self.number(raw["from"]), smooth: raw["smooth"] as? Bool == true)
        }
        self.view = view; self.revision = revision; self.sequence = sequence
        self.horizontal = (value["axis"] as? String) == "x"
        self.parent = Self.viewID(value["parent"])
        self.extent = extent; self.rows = rows; self.correction = correction
        self.count = Self.uint(value["count"]).map { Int(clamping: $0) } ?? rows.count
        self.pending = (value["pending"] as? NSNumber)?.boolValue ?? false
    }
    private static func uint(_ value: Any?) -> UInt64? {
        // Integral JSON numbers read directly; a fraction or sign refuses.
        if let n = value as? NSNumber, CFNumberIsFloatType(n) == false {
            return n.int64Value >= 0 ? n.uint64Value : nil
        }
        if let n = value as? NSNumber {
            // An integral double below 2^53 is its own digits; formatting
            // one to parse it back cost a tenth of a built row.
            let d = n.doubleValue
            if d.sign == .plus, d < 9_007_199_254_740_992, d.rounded(.towardZero) == d { return UInt64(d) }
            return UInt64(n.stringValue)
        }
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
        /// The limit the last report carried: a slice that may build more
        /// reports again with the same facts (a rescue just reported them,
        /// building only what shows).
        var lastLimit: UInt32?
        var port: [Double]?
        /// The offset when the batch began: where the reader is. Rows that
        /// leave above a deep offset shrink the document first, and the
        /// platform's clamp to it is not a scroll (`endBatch`'s shift).
        var batchStart: Double?
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
    private(set) var budget = CollectionTurnBudget()
    /// Lists whose uncovered port a rescue reports first, whatever the
    /// turn has spent (`CollectionTurnBudget.begin(rescue:)`).
    private var rescuing = Set<UInt32>()
    /// Each turn of the main run loop gets its passes: a continuation
    /// (`schedule`) or a slice is not the only start of a turn.
    private var turnObserver: CFRunLoopObserver?
    private var queued = false
    private var generation = 0
    private var contactSequence: UInt64 = 0
    private var gestureContact: UInt64?
    private var lastVisited: UInt32 = 0
    private var refreshPins = false
    #if os(iOS) || os(tvOS)
    /// `focusedView()` walks every node; a report asks each frame. UIKit's
    /// responder changes all reach `pinsChanged` (a node's become and
    /// resign, an input's begin and end editing), which forgets it.
    private var focusFound: UInt32??
    #endif
    /// LLP 1050.000's fill. A platform with a pump reports each moving
    /// collection's velocity (nil at rest) and builds `fillPending` in slices;
    /// one without leaves `motion` nil, and every report is unlimited.
    var motion: ((UInt32) -> Double?)?
    /// Lists the host is animating to a smooth correction: their offset
    /// moves by the platform's scroll animation, not the reader, so the fill
    /// reports no travel for them (which would cancel a `scrollIntoView`).
    ///
    /// While one runs the list reports where it is headed, and its scroll
    /// ticks are not the reader's travel. A report from mid-way told the
    /// runner the reader had left the end, and each tick advanced the
    /// scroll sequence past the corrections still to come: a message sent
    /// while the port's own follow ran was never followed.
    var animating = Set<UInt32>()
    /// Where each running animation is headed, and a later target taken
    /// when it lands (a correction, or a relative shift, that arrived
    /// while it ran).
    var animationTargets: [UInt32: CGPoint] = [:]
    var owedTargets: [UInt32: CGPoint] = [:]
    /// iOS: lists whose smooth correction begins on the next turn.
    var startOwed = Set<UInt32>()
    /// The running animation's number, for each animating list only: a
    /// callback for one that has since been stopped, or replaced, is not
    /// this one's.
    private(set) var animationSerial: [UInt32: Int] = [:]
    private var lastAnimation = 0
    @discardableResult
    func beginAnimation(_ view: UInt32, to target: CGPoint) -> Int {
        animating.insert(view)
        animationTargets[view] = target
        lastAnimation += 1
        animationSerial[view] = lastAnimation
        animationMoved.remove(view)
        return lastAnimation
    }
    /// An animation stops: by a drag, an ordinary correction, or the list's
    /// retirement. What it owed goes with it.
    func stopAnimation(_ view: UInt32) {
        animating.remove(view)
        animationTargets[view] = nil; owedTargets[view] = nil; animationSerial[view] = nil
        animationMoved.remove(view)
    }
    /// Animating lists whose port has moved since their animation began.
    private(set) var animationMoved = Set<UInt32>()
    /// `atTarget`: whether the platform's end is at the running animation's
    /// target, clamped to the content as it is now (UIKit's delegate). An end
    /// elsewhere before this animation moved the port is a stopped one's; one
    /// after it moved is this one's, stopped short (a snap, a clamp).
    func animationEnded(_ view: UInt32, dragging: Bool = false, atTarget: Bool? = nil) {
        guard animating.contains(view) else { return }
        if atTarget == false, !dragging, !animationMoved.contains(view) { return }
        if !dragging, owedTargets[view] != nil {
            // UIKit starts no new animation from inside the callback that
            // ends one: the owed target goes on the next turn, still headed
            // there in the meantime.
            let serial = animationSerial[view]
            DispatchQueue.main.async { [weak self] in
                guard let self, self.animationSerial[view] == serial, self.animating.remove(view) != nil else { return }
                self.animationTargets[view] = nil; self.animationSerial[view] = nil
                self.landAnimation(view)
                self.dirty.insert(view)
                self.schedule()
            }
            return
        }
        stopAnimation(view)
        dirty.insert(view)
        schedule()
    }
    var requestFill: (() -> Void)?
    /// After a report that built what shows inside the scroll callback: the
    /// platform paints what those rows show before the frame commits.
    var rescued: (() -> Void)?
    private(set) var fillPending = Set<UInt32>()
    private var sliceLimits: [UInt32: UInt32] = [:]
    /// @ref LLP 1072 §3, §5 — a moving list's slice is built off main. The
    /// session sends it (`onFill`), says whether one is in flight (`filling`,
    /// when every report waits), and lands it (`drain`: wait and apply).
    var onFill: ((UInt32, Data) -> Void)?
    var filling: (() -> Bool)?
    var drain: (() -> Void)?
    /// The slice a report sends off main, and the retire-only report a
    /// landed one owes, each with its limit.
    private var building: (view: UInt32, limit: UInt32)?
    private var retireOwed: [UInt32: UInt32] = [:]
    private var fillLimits: [UInt32: UInt32] = [:]
    /// Seconds from a slice's send to its landing, per list: the lead a
    /// moving list builds ahead covers it.
    private(set) var fillLatency: [UInt32: Double] = [:]
    private var fillSent: [UInt32: Double] = [:]
    // Platform hooks remove event monitors/recognizers when the adapter resets.
    var stopTracking: (() -> Void)?
    init(_ presenter: Presenter) { self.presenter = presenter }
    deinit {
        stopTracking?()
        if let turnObserver { CFRunLoopRemoveObserver(CFRunLoopGetMain(), turnObserver, .commonModes) }
    }
    private func observeTurns() {
        guard turnObserver == nil else { return }
        let observer = CFRunLoopObserverCreateWithHandler(nil, CFRunLoopActivity.afterWaiting.rawValue, true, 0) { [weak self] _, _ in
            self?.budget.nextTurn()
        }
        CFRunLoopAddObserver(CFRunLoopGetMain(), observer, .commonModes)
        turnObserver = observer
    }

    func reset() {
        gestureContact = nil
        generation += 1; queued = false; batchDepth = 0; correcting = false
        stopTracking?(); stopTracking = nil
        entries.removeAll(); dirty.removeAll(); interaction = nil; contactEvent = nil
        for view in animating { stopAnimation(view) }
        fillPending.removeAll(); sliceLimits.removeAll()
        building = nil; retireOwed.removeAll(); fillLimits.removeAll(); fillSent.removeAll()
        budget = CollectionTurnBudget(); rescuing.removeAll()
        refreshPins = false; lastVisited = 0
        #if os(iOS) || os(tvOS)
        focusFound = nil
        #endif
    }
    func beginBatch(_ batch: Batch) {
        if batchDepth == 0 { for (view, entry) in entries { entry.batchStart = geometry(view)?.offset } }
        batchDepth += 1
        for op in batch.ops where op.op == .collections {
            guard let items = op.payload["items"] as? [[String: Any]] else { continue }
            let snapshots = items.compactMap(CollectionSnapshot.init)
            guard snapshots.count == items.count else { continue }
            let live = Set(snapshots.map(\.view))
            guard live.count == snapshots.count else { continue }
            entries = entries.filter { live.contains($0.key) }
            for view in animating.subtracting(live) { stopAnimation(view) }
            retireOwed = retireOwed.filter { live.contains($0.key) }
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
            let dimensions = [port.portCross, port.portMain, port.cross]
            let planned = entry.cursor.sequence, jumped = entry.cursor.jumpedAt
            if let previous = entry.port, previous != dimensions { entry.cursor.jump() }
            entry.port = dimensions
            if let correction = entry.snapshot.correction, correction.from != nil {
                // Rows before the anchor changed size in this batch: the
                // offset moves with them before this frame displays, even
                // under a pan or a fling, which go on from there. A port this
                // same batch resized is not the reader moving either: rows
                // put above the reader as a pull-to-refresh zone closes stay
                // put on the page (feed F14), so a correction planned
                // before it still lands.
                if let delta = entry.cursor.takeShift(revision: entry.snapshot.revision, correction, jumpedBefore: jumped) {
                    correcting = true
                    shift(view, by: delta, extent: entry.snapshot.extent, from: entry.batchStart)
                    correcting = false
                }
            } else if let correction = entry.snapshot.correction,
               // The port this batch resized was not the reader moving: an
               // absolute correction planned at the sequence before it (a
               // sent message's end-follow as the composer shrinks back)
               // still lands, clamped to the new port.
               entry.cursor.takeCorrection(revision: entry.snapshot.revision,
                   sequence: correction.sequence == planned ? entry.cursor.sequence : correction.sequence) {
                correcting = true
                correct(view, top: correction.offset, extent: entry.snapshot.extent, smooth: correction.smooth)
                correcting = false
            }
            entry.batchStart = nil
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
    /// An inner list whose outer list moves builds only what it owes (LLP
    /// 1070 F2): a slice for its pending remainder would report and build
    /// nothing, once a frame for every mounted inner list.
    func ancestorMoving(_ view: UInt32) -> Bool {
        entries[view]?.snapshot.parent.flatMap { motion?($0) } != nil
    }
    /// The port is about to move: by the reader (`travel`, a drag or a
    /// wheel beginning) or to an authored offset.
    func userIntent(_ view: UInt32, travel: Bool = false) {
        guard let entry = entries[view], !correcting else { return }
        if travel { entry.cursor.advance() } else { entry.cursor.jump() }
        dirty.insert(view)
        schedule()
    }
    func changed(_ view: UInt32, user: Bool = false) {
        guard let entry = entries[view], !correcting else { return }
        // The host's own animation, not the reader: reported as where it
        // is headed (`geometry`), and the sequence stays.
        if user && animating.contains(view) { animationMoved.insert(view); dirty.insert(view); schedule(); return }
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
            lastVisited = view &- 1
            // What shows cannot wait for a slice in flight (T7): land it,
            // then report from the geometry that shows now.
            if filling?() == true { drain?() }
            rescuing.insert(view)
            flush()
            rescuing.remove(view)
            sliceLimits[view] = nil
            if motion != nil { rescued?() }
        }
    }
    /// One slice of fill for `view`: a report that may build `limit` rows
    /// past what it owes, and its measurement pass. The slice is its own
    /// main-queue turn. Returns the rows it created.
    @discardableResult
    func fillSlice(_ view: UInt32, limit: UInt32) -> Int {
        // One slice in flight at a time, and none before the last one's
        // retirement: this one stays owed (LLP 1072 §3.1, §5).
        if filling?() == true { return 0 }
        if !retireOwed.isEmpty {
            dirty.formUnion(retireOwed.keys)
            flush()
            return 0
        }
        fillPending.remove(view)
        guard let entry = entries[view], batchDepth == 0 else { return 0 }
        let before = Set(entry.snapshot.rows.map(\.view))
        budget.nextTurn()
        dirty.insert(view)
        sliceLimits[view] = limit
        // The slice's own list first: the round-robin would visit it last.
        lastVisited = view &- 1
        if onFill != nil { building = (view, limit) }
        flush()
        building = nil
        sliceLimits[view] = nil
        return (entries[view]?.snapshot.rows ?? []).filter { !before.contains($0.view) }.count
    }
    /// A slice built off main has been applied (LLP 1072 §5): what it left
    /// owed (rows past the window, an edge) is settled now, on main, by a
    /// retire-only report from fresh facts and the pins that hold now.
    func landed(_ view: UInt32, at now: Double) {
        if let sent = fillSent.removeValue(forKey: view) { fillLatency[view] = now - sent }
        // The reports the slice held run in the next main-queue turn.
        if !dirty.isEmpty { schedule() }
        guard let limit = fillLimits.removeValue(forKey: view), entries[view] != nil else { return }
        retireOwed[view] = limit
        // The pump's next frame: the slice's apply and its retirement are
        // two frames' work, not one (LLP 1072 §5).
        fillPending.insert(view)
        requestFill?()
    }
    /// The agent's `clock settle`: every list reports until none is owed a
    /// report, so rows a reply mounted are measured before the agent reads
    /// or moves them, as a turn of the main queue would do between a
    /// reader's frames. Bounded.
    func settle() {
        for _ in 0..<8 {
            guard !dirty.isEmpty, batchDepth == 0 else { return }
            budget.nextTurn()
            flush()
        }
    }
    func dataReady() {
        // Retry armed edges once after deferred activation without forgetting
        // the accepted pin reservations used to order transfers.
        for entry in entries.values { entry.lastSequence = nil }
        dirty.formUnion(entries.keys)
        schedule()
    }
    func pinsChanged() {
        #if os(iOS) || os(tvOS)
        focusFound = nil
        #endif
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
        // A slice in flight holds every report until it lands (T4), and its
        // landing flushes again (`landed`). A continuation now would find it
        // still in flight and queue another, every main-queue turn until it
        // lands (LLP 1072 §3.2).
        if filling?() == true { return }
        observeTurns()
        // Reserve the continuation before calling Rust: its batch can reenter us.
        schedule()
        // Each list once per flush, at most, without a report (bounded
        // even if a visit re-dirties its list).
        var free = entries.count
        while !dirty.isEmpty {
            // A slice in flight holds every report until it lands (T4).
            if filling?() == true { return }
            let rescue = !rescuing.isDisjoint(with: dirty)
            guard budget.begin(rescue: rescue) else { return }
            #if os(iOS) || os(tvOS)
            let focus = focusFound ?? focusedView()
            focusFound = focus
            #else
            let focus = focusedView()
            #endif
            let focusOwner = owningCollection(focus)
            let interactionOwner = owningCollection(interaction)
            // Retire old owners before publishing replacement pins, even when
            // the two-pass budget carries the new owner to a later callback.
            let retiring = dirty.filter { id in
                guard let facts = entries[id]?.lastFacts else { return false }
                return (facts.focus != nil && focusOwner != id) ||
                    (facts.interaction != nil && interactionOwner != id)
            }
            // Retiring owners first (their pins go before new ones), then a
            // rescue's list, then any.
            let rescued = rescue ? dirty.intersection(rescuing) : []
            let candidates = !retiring.isEmpty ? retiring : !rescued.isEmpty ? rescued : dirty
            // Round-robin keeps one refining viewport from starving another.
            let id = candidates.filter { $0 > lastVisited }.min() ?? candidates.min()!
            lastVisited = id
            dirty.remove(id)
            let retireLimit = retireOwed.removeValue(forKey: id)
            // Cached geometry permits a hidden previous owner to release its
            // pin. Measurements below still come only from visible live rows.
            var reported = false
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
                let horizontal = entry.snapshot.horizontal
                facts.measurements = entry.snapshot.rows.compactMap { row in
                    guard let size = size(row.view, horizontal: horizontal), size.isFinite, size >= 0,
                          crossSize(row.view, horizontal: horizontal) == facts.cross else { return nil }
                    return CollectionMeasurement(view: row.view, epoch: row.epoch, size: size)
                }
                // A slice builds its limit once; later passes and every
                // report while moving or owed a continuation only measure
                // and rescue what shows. At rest a report is unlimited.
                let velocity = motion?(id)
                let limit: UInt32? = retireLimit ?? sliceLimits.removeValue(forKey: id)
                    ?? (motion != nil && (velocity != nil || entry.snapshot.pending) ? 0 : nil)
                let further = limit.map { $0 > 0 && $0 > (entry.lastLimit ?? .max) } ?? (entry.lastLimit != nil)
                // Off main only when the report keeps the port, the width and
                // the pins: a report that changes them may retire (§5).
                let same = entry.lastFacts.map { ($0.portMain, $0.portCross, $0.cross, $0.focus, $0.interaction)
                    == (facts.portMain, facts.portCross, facts.cross, facts.focus, facts.interaction) } ?? false
                let offMain = building?.view == id && same && retireLimit == nil ? building : nil
                if entry.lastFacts != facts || entry.lastSequence != entry.cursor.sequence
                    || (entry.snapshot.pending && limit != 0) || further || retireLimit != nil {
                    entry.lastFacts = facts
                    entry.lastSequence = entry.cursor.sequence
                    entry.lastLimit = limit
                    reported = true
                    // While its outer list moves, an inner list builds only
                    // what it owes (LLP 1070 F2); its pending report
                    // continues once the outer list rests.
                    let ancestorMoving = entry.snapshot.parent.flatMap { motion?($0) } != nil
                    if let offMain, let onFill {
                        fillLimits[id] = offMain.limit
                        fillSent[id] = ProcessInfo.processInfo.systemUptime
                        onFill(id, facts.encode(view: id, revision: entry.snapshot.revision, sequence: entry.cursor.sequence,
                            velocity: velocity ?? 0, limit: limit, ancestorMoving: ancestorMoving, createOnly: true))
                        budget.end()
                        return
                    }
                    onFeedback(facts.encode(view: id, revision: entry.snapshot.revision, sequence: entry.cursor.sequence,
                        velocity: velocity ?? 0, limit: limit, ancestorMoving: ancestorMoving, noBuild: retireLimit != nil))
                }
            }
            free -= reported ? 0 : 1
            budget.end(reported: reported || free < 0)
        }
    }
    /// Continuations run (`schedule`), for tests.
    private(set) var continuations = 0
    private func schedule() {
        guard !queued else { return }
        queued = true
        let captured = generation
        DispatchQueue.main.async { [weak self] in
            guard let self, generation == captured else { return }
            queued = false
            continuations += 1
            budget.nextTurn()
            if refreshPins {
                refreshPins = false; dirty.formUnion(entries.keys)
                #if os(iOS) || os(tvOS)
                focusFound = nil
                #endif
            }
            flush()
        }
    }
}
