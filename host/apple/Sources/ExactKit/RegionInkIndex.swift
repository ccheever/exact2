// Queue-confined conservative ink index; no CT/CoreText or UI owner.
// One slab owns all span/node/list/build/query storage; callbacks borrow results.
import Foundation

struct InkSpan { let top: Double; let bottom: Double }
enum InkRefusal: Error { case capacity, budget }
struct InkAccountStats { var bytes = 0; var owners = 0; var peak = 0; var allocations = 0; var drops = 0 }
final class InkAccount: @unchecked Sendable {
    private let lock = NSLock()
    private var value = InkAccountStats()
    var stats: InkAccountStats { lock.lock(); defer { lock.unlock() }; return value }
    fileprivate func reserve(_ bytes: Int) -> InkCharge? {
        lock.lock(); defer { lock.unlock() }
        guard bytes > 0, bytes <= 8 * 1024 * 1024, value.owners < 2,
              bytes <= 16 * 1024 * 1024 - value.bytes else { return nil }
        value.bytes += bytes; value.owners += 1; value.allocations += 1
        value.peak = max(value.peak, value.bytes)
        return InkCharge(self, bytes)
    }
    fileprivate func release(_ bytes: Int) {
        lock.lock(); value.bytes -= bytes; value.owners -= 1; value.drops += 1; lock.unlock()
    }
}
private final class InkCharge {
    let account: InkAccount
    let bytes: Int
    init(_ account: InkAccount, _ bytes: Int) { self.account = account; self.bytes = bytes }
    deinit { account.release(bytes) }
}
struct InkQueryStats { var nodes = 0; var endpoints = 0; var selected = 0 }
private struct InkNode {
    var center: Double
    var left: Int32 = -1
    var right: Int32 = -1
    var start: Int32
    var count: Int32
}
private struct InkBuildTask { var lo: Int32; var hi: Int32; var parent: Int32; var side: Int32 }

final class WorkerInkIndex {
    // All stored POD types have alignment <=8. Their slab regions have strides
    // divisible by the following region's alignment; no hidden Array capacities.
    private static var unitBytes: Int {
        MemoryLayout<InkSpan>.stride + MemoryLayout<InkNode>.stride
            + 6 * MemoryLayout<Int32>.stride + MemoryLayout<InkBuildTask>.stride
    }
    static var maximumCount: Int { (8 * 1024 * 1024) / unitBytes }
    static func requiredBytes(count: Int) -> Int? {
        guard count >= 0, count <= maximumCount else { return nil }
        let (bytes, overflow) = max(1, count).multipliedReportingOverflow(by: unitBytes)
        return overflow ? nil : bytes
    }
    let count: Int
    let capacityBytes: Int
    let buildCount = 1
    private(set) var queryCount = 0
    private(set) var unboundedCount = 0
    private var nodeCount = 0
    private var querying = false
    private let charge: InkCharge
    private let slab: UnsafeMutableRawPointer
    private let spans: UnsafeMutablePointer<InkSpan>
    private let nodes: UnsafeMutablePointer<InkNode>
    private let starts: UnsafeMutablePointer<Int32>
    private let ends: UnsafeMutablePointer<Int32>
    private let order: UnsafeMutablePointer<Int32>
    private let scratch: UnsafeMutablePointer<Int32>
    private let selected: UnsafeMutablePointer<Int32>
    private let tasks: UnsafeMutablePointer<InkBuildTask>
    private let stack: UnsafeMutablePointer<Int32>

    init(count: Int, account: InkAccount, span: (Int) -> InkSpan) throws {
        guard let bytes = Self.requiredBytes(count: count) else { throw InkRefusal.capacity }
        guard let charge = account.reserve(bytes) else { throw InkRefusal.budget }
        self.charge = charge; self.count = count; capacityBytes = bytes
        let memory = UnsafeMutableRawPointer.allocate(byteCount: bytes, alignment: 8)
        slab = memory
        let n = max(1, count)
        var offset = 0
        func region<T>(_ type: T.Type, _ initial: T) -> UnsafeMutablePointer<T> {
            precondition(offset % MemoryLayout<T>.alignment == 0)
            let result = memory.advanced(by: offset).bindMemory(to: T.self, capacity: n)
            offset += n * MemoryLayout<T>.stride
            result.initialize(repeating: initial, count: n)
            return result
        }
        spans = region(InkSpan.self, InkSpan(top: 0, bottom: 0)); nodes = region(InkNode.self, InkNode(center: 0, start: 0, count: 0))
        starts = region(Int32.self, 0); ends = region(Int32.self, 0)
        order = region(Int32.self, 0); scratch = region(Int32.self, 0); selected = region(Int32.self, 0)
        tasks = region(InkBuildTask.self, InkBuildTask(lo: 0, hi: 0, parent: -1, side: 0)); stack = region(Int32.self, 0)
        precondition(offset == bytes)
        for i in 0..<count {
            let value = span(i)
            if value.top.isFinite && value.bottom.isFinite && value.top <= value.bottom {
                spans[i] = value
            } else {
                spans[i] = InkSpan(top: -.infinity, bottom: .infinity)
                unboundedCount += 1
            }
            order[i] = Int32(i)
        }
        build()
    }
    deinit { slab.deallocate() } // charge releases only after allocation destruction

    // In-place heapsort uses no allocator and bounded scalar call state.
    private func sort(_ ids: UnsafeMutablePointer<Int32>, count: Int,
                      before: (Int32, Int32) -> Bool) {
        guard count > 1 else { return }
        func sift(_ root: Int, _ end: Int) {
            var p = root
            while p * 2 + 1 < end {
                var child = p * 2 + 1
                if child + 1 < end && before(ids[child], ids[child + 1]) { child += 1 }
                if !before(ids[p], ids[child]) { return }
                let temporary = ids[p]; ids[p] = ids[child]; ids[child] = temporary
                p = child
            }
        }
        for root in stride(from: count / 2 - 1, through: 0, by: -1) { sift(root, count) }
        for end in stride(from: count - 1, through: 1, by: -1) {
            let temporary = ids[0]; ids[0] = ids[end]; ids[end] = temporary
            sift(0, end)
        }
    }

    private func build() {
        guard count > 0 else { return }
        sort(order, count: count) {
            let a = spans[Int($0)].top, b = spans[Int($1)].top
            return a == b ? $0 < $1 : a < b
        }
        tasks[0] = InkBuildTask(lo: 0, hi: Int32(count), parent: -1, side: 0)
        var pending = 1, cursor = 0
        while pending > 0 {
            pending -= 1
            let task = tasks[pending], lo = Int(task.lo), hi = Int(task.hi)
            let mid = spans[Int(order[(lo + hi) / 2])].top
            let center = mid.isFinite ? mid : -Double.greatestFiniteMagnitude
            var leftCount = 0, bucketCount = 0
            for i in lo..<hi {
                let value = spans[Int(order[i])]
                if value.bottom < center { leftCount += 1 }
                else if value.top <= center { bucketCount += 1 }
            }
            precondition(bucketCount > 0)
            var l = lo, b = lo + leftCount, r = b + bucketCount
            for i in lo..<hi {
                let id = order[i], value = spans[Int(id)]
                if value.bottom < center { scratch[l] = id; l += 1 }
                else if value.top > center { scratch[r] = id; r += 1 }
                else { scratch[b] = id; b += 1 }
            }
            for i in lo..<hi { order[i] = scratch[i] }
            let node = nodeCount; nodeCount += 1
            nodes[node] = InkNode(center: center, start: Int32(cursor), count: Int32(bucketCount))
            for i in 0..<bucketCount {
                let id = order[lo + leftCount + i]
                starts[cursor + i] = id; ends[cursor + i] = id
            }
            sort(ends.advanced(by: cursor), count: bucketCount) {
                let a = spans[Int($0)].bottom, b = spans[Int($1)].bottom
                return a == b ? $0 < $1 : a > b
            }
            cursor += bucketCount
            if task.parent >= 0 {
                if task.side == 0 { nodes[Int(task.parent)].left = Int32(node) }
                else { nodes[Int(task.parent)].right = Int32(node) }
            }
            if leftCount > 0 {
                tasks[pending] = InkBuildTask(lo: Int32(lo), hi: Int32(lo + leftCount), parent: Int32(node), side: 0)
                pending += 1
            }
            if lo + leftCount + bucketCount < hi {
                tasks[pending] = InkBuildTask(lo: Int32(lo + leftCount + bucketCount), hi: Int32(hi), parent: Int32(node), side: 1)
                pending += 1
            }
        }
        precondition(cursor == count)
    }

    func query(top: Double, bottom: Double,
               _ body: (UnsafeBufferPointer<Int32>, InkQueryStats) -> Void) {
        precondition(top.isFinite && bottom.isFinite && top <= bottom && !querying)
        querying = true; defer { querying = false }
        queryCount += 1
        var stats = InkQueryStats(), pending = 0
        if nodeCount > 0 { stack[0] = 0; pending = 1 }
        while pending > 0 {
            pending -= 1
            let node = nodes[Int(stack[pending])]
            stats.nodes += 1
            let start = Int(node.start), count = Int(node.count)
            if bottom < node.center {
                for i in 0..<count {
                    let id = starts[start + i]; stats.endpoints += 1
                    if spans[Int(id)].top > bottom { break }
                    selected[stats.selected] = id; stats.selected += 1
                }
                if node.left >= 0 { stack[pending] = node.left; pending += 1 }
            } else if top > node.center {
                for i in 0..<count {
                    let id = ends[start + i]; stats.endpoints += 1
                    if spans[Int(id)].bottom < top { break }
                    selected[stats.selected] = id; stats.selected += 1
                }
                if node.right >= 0 { stack[pending] = node.right; pending += 1 }
            } else {
                for i in 0..<count { selected[stats.selected] = starts[start + i]; stats.selected += 1 }
                if node.left >= 0 { stack[pending] = node.left; pending += 1 }
                if node.right >= 0 { stack[pending] = node.right; pending += 1 }
            }
        }
        sort(selected, count: stats.selected, before: <)
        body(UnsafeBufferPointer(start: selected, count: stats.selected), stats)
    }
}
