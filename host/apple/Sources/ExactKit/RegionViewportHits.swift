// Bounded optional interaction acceleration. Missing coverage is not an index
// and never prevents the worker from drawing the complete text.
import Foundation
import CoreText

struct RegionHitStats { var bytes = 0; var owners = 0; var peak = 0; var drops = 0 }
final class RegionHitAccount: @unchecked Sendable {
    static let outputBytes = 4 * 1024 * 1024
    private let lock = NSLock()
    private var value = RegionHitStats()
    var stats: RegionHitStats { lock.lock(); defer { lock.unlock() }; return value }
    func reserve() -> RegionHitCharge? {
        lock.lock(); defer { lock.unlock() }
        guard value.owners < 2, value.bytes <= 8 * 1024 * 1024 - Self.outputBytes else { return nil }
        value.bytes += Self.outputBytes; value.owners += 1; value.peak = max(value.peak,value.bytes)
        return RegionHitCharge(self)
    }
    fileprivate func release() {
        lock.lock(); value.bytes -= Self.outputBytes; value.owners -= 1; value.drops += 1; lock.unlock()
    }
}
final class RegionHitCharge: Sendable {
    private let account: RegionHitAccount
    init(_ account: RegionHitAccount) { self.account = account }
    deinit { account.release() }
}
private struct RegionHitRecord {
    var artifact: UInt64 = 0
    var line = 0, count = 0
    var edges: UnsafeMutablePointer<CGFloat>?
    var points: UnsafeMutablePointer<CFIndex>?
    var intervals: UnsafeMutablePointer<CFIndex>?
}
private struct RegionHitArena {
    let memory: UnsafeMutableRawPointer
    var cursor = 0
    mutating func allocate<T>(_ type: T.Type, count: Int, initial: T) -> UnsafeMutablePointer<T>? {
        guard count >= 0 else { return nil }
        let alignment = MemoryLayout<T>.alignment
        let start = (cursor + alignment - 1) & ~(alignment - 1)
        let (bytes, overflow) = count.multipliedReportingOverflow(by: MemoryLayout<T>.stride)
        guard !overflow, start <= RegionHitAccount.outputBytes,
              bytes <= RegionHitAccount.outputBytes - start else { return nil }
        cursor = start + bytes
        let result = memory.advanced(by: start).bindMemory(to: T.self, capacity: count)
        result.initialize(repeating: initial,count: count)
        return result
    }
}
/// All mutation ends before publication. The one slab includes the directory,
/// edges, answers AND temporary sort/enumeration space (retained and charged).
/// No CoreText objects, borrowed source buffers or mutable shared cache escape.
final class RegionViewportHits: @unchecked Sendable {
    static let maximumLines = 512
    static let maximumCaretUnits = 8192
    private let charge: RegionHitCharge?
    private let memory: UnsafeMutableRawPointer?
    private let records: UnsafeMutablePointer<RegionHitRecord>?
    private(set) var cachedLines = 0
    private(set) var caretUnits = 0

    init(request: RegionRasterRequest, lookup: (UInt64) -> RegionWorkerLayout?, account: RegionHitAccount) {
        precondition(!Thread.isMainThread)
        charge = account.reserve()
        guard charge != nil else { memory = nil; records = nil; return }
        let memory = UnsafeMutableRawPointer.allocate(byteCount: RegionHitAccount.outputBytes,alignment: 16)
        self.memory = memory
        var arena = RegionHitArena(memory: memory)
        records = arena.allocate(RegionHitRecord.self,count: Self.maximumLines,initial: RegionHitRecord())!
        var visited = 0
        for row in request.rows {
            guard let layout = lookup(row.artifact), !layout.lines.isEmpty else { continue }
            let p = layout.metadata
            let top = request.scroll.y - row.box.minY
            let bottom = top + request.size.height
            // Upper-bound lookup is the hit rule, not the conservative ink rule.
            guard bottom >= 0, top <= row.box.height,
                  let lo = p.lineIndex(at: max(0,top)),
                  let hi = p.lineIndex(at: min(row.box.height,bottom)) else { continue }
            guard lo <= hi else { continue }
            // Zero-height/overlapping lines cannot turn admission into a full scan.
            for i in lo...hi {
                guard visited < Self.maximumLines else { return }
                visited += 1
                let line = layout.lines[i]
                let range = CTLineGetStringRange(line)
                guard range.location >= 0, range.length >= 0,
                      range.length < Self.maximumCaretUnits - caretUnits else { continue }
                let units = range.length + 1
                caretUnits += units
                if let record = Self.capture(line,artifact: row.artifact,lineIndex: i,units: units,arena: &arena) {
                    records![cachedLines] = record; cachedLines += 1
                } // Failed scratch remains in the charged slab; never rebind live typed storage.
            }
        }
    }
    deinit { memory?.deallocate() } // charge retires only after actual backing destruction

    private static func capture(_ line: CTLine, artifact: UInt64, lineIndex: Int,
                                units: Int, arena: inout RegionHitArena) -> RegionHitRecord? {
        let range = CTLineGetStringRange(line)
        // A public callback stop bounds extra caret enumeration too. Exceeding
        // this cache limit chooses exact-point fallback, never a partial table.
        let enumLimit = 4 * units + 16
        let capacity = 2 * units + enumLimit + 2
        guard let offsets = arena.allocate(CGFloat.self,count: capacity,initial: 0) else { return nil }
        var count = 0
        for index in range.location...(range.location + range.length) {
            var secondary: CGFloat = 0
            let primary = CTLineGetOffsetForStringIndex(line,index,&secondary)
            if primary.isFinite { offsets[count] = primary; count += 1 }
            if secondary.isFinite { offsets[count] = secondary; count += 1 }
        }
        var enumerated = 0, complete = true
        CTLineEnumerateCaretOffsets(line) { offset, _, _, stop in
            guard enumerated < enumLimit else { complete = false; stop.pointee = true; return }
            enumerated += 1
            if offset.isFinite { offsets[count] = CGFloat(offset); count += 1 }
        }
        guard complete else { return nil }
        offsets[count] = 0; count += 1
        let width = CGFloat(CTLineGetTypographicBounds(line,nil,nil,nil))
        if width.isFinite { offsets[count] = width; count += 1 }
        count = sortedUnique(offsets,count: count)
        guard count > 0,
              let edges = arena.allocate(CGFloat.self,count: 2 * count - 1,initial: 0) else { return nil }
        for i in 0..<count { edges[i] = offsets[i] }
        if count > 1 { for i in 1..<count { edges[count+i-1] = (offsets[i-1]+offsets[i])/2 } }
        let edgeCount = sortedUnique(edges,count: 2 * count - 1)
        guard let points = arena.allocate(CFIndex.self,count: edgeCount,initial: 0),
              let intervals = arena.allocate(CFIndex.self,count: edgeCount+1,initial: 0) else { return nil }
        func hit(_ x: CGFloat) -> CFIndex { CTLineGetStringIndexForPosition(line,CGPoint(x: x,y: 0)) }
        for i in 0..<edgeCount { points[i] = hit(edges[i]) }
        intervals[0] = hit(edges[0]-1)
        if edgeCount > 1 { for i in 1..<edgeCount { intervals[i] = hit((edges[i-1]+edges[i])/2) } }
        intervals[edgeCount] = hit(edges[edgeCount-1]+1)
        return RegionHitRecord(artifact: artifact,line: lineIndex,count: edgeCount,
                               edges: edges,points: points,intervals: intervals)
    }
    static func sortedUnique(_ buffer: UnsafeMutablePointer<CGFloat>, count: Int) -> Int {
        // Swift's stable sort allocates auxiliary arrays even for an unsafe
        // buffer. Heap-sort these finite scalars in their charged storage;
        // equal values are deduplicated below, so stability is immaterial.
        func swap(_ a: Int, _ b: Int) {
            let value = buffer[a]; buffer[a] = buffer[b]; buffer[b] = value
        }
        func sift(_ start: Int, _ end: Int) {
            var root = start
            while root < end / 2 {
                var child = 2 * root + 1
                if child + 1 < end, buffer[child] < buffer[child+1] { child += 1 }
                if buffer[root] >= buffer[child] { return }
                swap(root,child); root = child
            }
        }
        if count > 1 {
            for root in stride(from: count / 2 - 1, through: 0, by: -1) { sift(root,count) }
            var end = count
            while end > 1 { end -= 1; swap(0,end); sift(0,end) }
        }
        var n = 0
        for i in 0..<count {
            if n == 0 || buffer[i] != buffer[n-1] { buffer[n] = buffer[i]; n += 1 }
        }
        return n
    }
    func index(artifact: UInt64, line: Int, x: CGFloat) -> CFIndex? {
        guard x.isFinite, let records else { return nil }
        for i in 0..<cachedLines where records[i].artifact == artifact && records[i].line == line {
            let row = records[i], edges = row.edges!
            var lo = 0, hi = row.count
            while lo < hi {
                let mid = lo + (hi-lo)/2
                if edges[mid] < x { lo = mid+1 } else { hi = mid }
            }
            if lo < row.count, edges[lo] == x { return row.points![lo] }
            return row.intervals![lo]
        }
        return nil
    }
}

// Only scalar points/indices cross the queue; the enclosing raster request
// certifies generation/publication/artifacts and the complete native phase.
struct RegionPointRequest: Sendable, Equatable {
    let gesture: UInt64
    let sequence: UInt64
    let artifact: UInt64
    let anchor: Int?
    let begin: CGPoint?
    let point: CGPoint
    let terminal: Bool
    let dragged: Bool
    let selectAll: Bool
}
struct RegionPointReply: Sendable {
    let query: RegionPointRequest
    let anchor: Int
    let index: Int
    let link: String?
    let selection: NSRange?
}
struct RegionContact {
    let gesture: UInt64
    let artifact: UInt64
    private(set) var sequence: UInt64 = 1
    private var accepted: UInt64 = 0
    private var begin: CGPoint?
    private var point: CGPoint
    private var anchor: Int?
    private var dragged = false
    private var terminal = false
    private var selectAll = false
    private(set) var finished = false
    init(gesture: UInt64, artifact: UInt64, point: CGPoint, selectAll: Bool = false) {
        self.gesture = gesture; self.artifact = artifact; begin = point; self.point = point; self.selectAll = selectAll
    }
    var query: RegionPointRequest? {
        guard !finished, accepted != sequence else { return nil }
        return RegionPointRequest(gesture: gesture,sequence: sequence,artifact: artifact,anchor: anchor,
            begin: begin,point: point,terminal: terminal,dragged: dragged,selectAll: selectAll)
    }
    mutating func update(point: CGPoint, dragged: Bool, terminal: Bool) -> Bool {
        guard !finished, !self.terminal, sequence < UInt64.max, point.x.isFinite, point.y.isFinite else { return false }
        sequence += 1; self.point = point; self.dragged = self.dragged || dragged; self.terminal = terminal
        return true
    }
    mutating func accept(_ reply: RegionPointReply) -> Bool {
        guard let query, reply.query == query, reply.anchor >= 0, reply.index >= 0 else { return false }
        anchor = reply.anchor; begin = nil; accepted = sequence
        finished = query.terminal
        return true
    }
}
