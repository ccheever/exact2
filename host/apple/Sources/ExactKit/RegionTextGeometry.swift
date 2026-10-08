// Queue-neutral owned result. No CTLine/CTRun/CTTypesetter or TextEngine alias.
import Foundation
import CoreText
#if canImport(UIKit)
import UIKit
#else
import AppKit
#endif

struct RegionCaret: Sendable {
    let primary: CGFloat
    let secondary: CGFloat
}

struct RegionLine: Sendable {
    let range: NSRange
    let carets: [RegionCaret]
    let ink: CGRect
    let ascent: CGFloat
    let descent: CGFloat
    let leading: CGFloat
    let typographicWidth: CGFloat
    let flushOffset: CGFloat
    /// Point answers and open-interval answers are kept separately. The worker
    /// asks CoreText at caret boundaries; the UI never invokes CTLine APIs.
    private let hitEdges: [CGFloat]
    private let edgeIndices: [CFIndex]
    private let intervalIndices: [CFIndex]

    init(_ line: CTLine, flush: CGFloat, width: CGFloat, captureHits: Bool = true, conservativeInk: CGRect? = nil) {
        let r = CTLineGetStringRange(line)
        range = NSRange(location: r.location, length: r.length)
        var above: CGFloat = 0, below: CGFloat = 0, extra: CGFloat = 0
        typographicWidth = CGFloat(CTLineGetTypographicBounds(line, &above, &below, &extra))
        ascent = above; descent = below; leading = extra
        if let conservativeInk {
            ink = CGRect(x: conservativeInk.minX, y: conservativeInk.minY,
                         width: typographicWidth + conservativeInk.width, height: conservativeInk.height)
        } else { ink = CTLineGetBoundsWithOptions(line, .useGlyphPathBounds) }
        flushOffset = CGFloat(CTLineGetPenOffsetForFlush(line, flush, Double(width)))
        if !captureHits {
            self.carets = []; hitEdges = []; edgeIndices = []; intervalIndices = []
            return
        }
        var carets: [RegionCaret] = []
        var offsets: [CGFloat] = []
        for index in r.location...(r.location + r.length) {
            var secondary: CGFloat = 0
            let primary = CTLineGetOffsetForStringIndex(line, index, &secondary)
            carets.append(RegionCaret(primary: primary, secondary: secondary))
            offsets += [primary, secondary]
        }
        self.carets = carets
        CTLineEnumerateCaretOffsets(line) { offset, _, _, _ in offsets.append(offset) }
        offsets += [0, typographicWidth]
        offsets = Array(Set(offsets.filter(\.isFinite))).sorted()
        var edges = offsets
        for i in 1..<offsets.count { edges.append((offsets[i - 1] + offsets[i]) / 2) }
        edges = Array(Set(edges)).sorted()
        func hit(_ x: CGFloat) -> CFIndex {
            CTLineGetStringIndexForPosition(line, CGPoint(x: x, y: 0))
        }
        hitEdges = edges
        edgeIndices = edges.map(hit)
        var intervals = [hit((edges.first ?? 0) - 1)]
        for i in 1..<edges.count { intervals.append(hit((edges[i - 1] + edges[i]) / 2)) }
        intervals.append(hit((edges.last ?? 0) + 1))
        intervalIndices = intervals
    }

    func offsets(at index: Int) -> RegionCaret {
        let local = index - range.location
        guard carets.indices.contains(local) else { return RegionCaret(primary: 0, secondary: 0) }
        return carets[local]
    }

    func index(at x: CGFloat) -> CFIndex {
        var low = 0, high = hitEdges.count
        while low < high {
            let mid = low + (high - low) / 2
            if hitEdges[mid] < x { low = mid + 1 } else { high = mid }
        }
        if low < hitEdges.count, hitEdges[low] == x { return edgeIndices[low] }
        return intervalIndices[low]
    }

    var arrayBytes: Int {
        carets.count * MemoryLayout<RegionCaret>.stride
            + hitEdges.count * MemoryLayout<CGFloat>.stride
            + (edgeIndices.count + intervalIndices.count) * MemoryLayout<CFIndex>.stride
    }
}

final class RegionParagraph: Sendable {
    let source: RegionTextSource
    let sourceSHA256: String
    let lines: [RegionLine]
    let baselines: [CGFloat]
    let lineBottoms: [CGFloat]
    let width: CGFloat
    let height: CGFloat
    let offeredWidth: CGFloat
    let shapedOnMainThread: Bool
    var firstBaseline: CGFloat { baselines.first ?? 0 }
    /// Logical array payload only; font/platform internals and array slack are
    /// opaque, source storage is reported separately, not a peak RSS guarantee.
    var arrayBytes: Int {
        lines.reduce(0) { $0 + $1.arrayBytes } + lines.count * MemoryLayout<RegionLine>.stride
            + (baselines.count + lineBottoms.count) * MemoryLayout<CGFloat>.stride
    }

    convenience init(source: RegionTextSource, sourceSHA256: String, lines: [CTLine], baselines: [CGFloat], width: CGFloat,
                     height: CGFloat, lineBottoms: [CGFloat], offeredWidth: CGFloat, retainHits: Bool = true, captureHits: Bool = true) {
        self.init(source: source, sourceSHA256: sourceSHA256, lines: lines, baselines: baselines, width: width,
                  height: height, lineBottoms: lineBottoms, offeredWidth: offeredWidth, retainHits: retainHits,
                  captureHits: captureHits, metadataCheckpoint: {})
    }
    init(source: RegionTextSource, sourceSHA256: String, lines: [CTLine], baselines: [CGFloat], width: CGFloat,
         height: CGFloat, lineBottoms: [CGFloat], offeredWidth: CGFloat, retainHits: Bool = true, captureHits: Bool = true,
         metadataCheckpoint: () throws -> Void) rethrows {
        precondition(!Thread.isMainThread)
        self.source = source; self.baselines = baselines; self.lineBottoms = lineBottoms
        self.sourceSHA256 = sourceSHA256
        self.width = width; self.height = height; self.offeredWidth = offeredWidth
        shapedOnMainThread = Thread.isMainThread
        let flush: CGFloat = source.align == 1 ? 0.5 : source.align == 2 ? 1 : 0
        var captured: [RegionLine] = []
        if retainHits {
            captured.reserveCapacity(lines.count)
            for (index, line) in lines.enumerated() {
                if index > 0 && index % 128 == 0 { try metadataCheckpoint() }
                captured.append(RegionLine(line, flush: flush, width: offeredWidth, captureHits: captureHits))
            }
            // Include the final short tail. No partially initialized metadata
            // or layout binding escapes if the owning request was superseded.
            if !lines.isEmpty { try metadataCheckpoint() }
        }
        self.lines = captured
    }

    init(source: RegionTextSource, sourceSHA256: String, lines: [RegionLine], baselines: [CGFloat],
         lineBottoms: [CGFloat], width: CGFloat, height: CGFloat, offeredWidth: CGFloat) {
        precondition(!Thread.isMainThread)
        self.source = source; self.sourceSHA256 = sourceSHA256; self.lines = lines
        self.baselines = baselines; self.lineBottoms = lineBottoms
        self.width = width; self.height = height; self.offeredWidth = offeredWidth
        shapedOnMainThread = false
    }

    func copy(_ range: NSRange) -> String {
        guard range.location >= 0, range.length >= 0, range.location <= source.utf16Count,
              range.length <= source.utf16Count - range.location else { return "" }
        return (source.text as NSString).substring(with: range)
    }

    func lineIndex(at y: CGFloat) -> Int? {
        guard !lines.isEmpty else { return nil }
        var lo = 0, hi = lineBottoms.count
        while lo < hi {
            let mid = lo + (hi - lo) / 2
            if lineBottoms[mid] <= y { lo = mid + 1 } else { hi = mid }
        }
        return min(lo, lines.count - 1)
    }
    private func line(at point: CGPoint, in bounds: CGRect) -> RegionLine? {
        lineIndex(at: point.y - bounds.minY).map { lines[$0] }
    }
    func cachedIndex(at point: CGPoint, in bounds: CGRect, artifact: UInt64, hits: RegionViewportHits) -> Int? {
        guard point.x.isFinite, point.y.isFinite else { return nil }
        if point.y < bounds.minY { return 0 }
        if point.y > bounds.maxY { return source.utf16Count }
        guard let i = lineIndex(at: point.y - bounds.minY) else { return 0 }
        guard let index = hits.index(artifact: artifact,line: i,x: point.x - bounds.minX - lines[i].flushOffset) else { return nil }
        return index == kCFNotFound ? source.utf16Count : min(max(0,index),source.utf16Count)
    }
    func link(at point: CGPoint, in bounds: CGRect, exactIndex: Int) -> String? {
        guard bounds.contains(point), let line = line(at: point,in: bounds) else { return nil }
        let x = bounds.minX + line.flushOffset
        guard point.x >= x, point.x <= x + line.typographicWidth else { return nil }
        return source.link(at: exactIndex)
    }

    func index(at point: CGPoint, in bounds: CGRect) -> Int {
        if point.y < bounds.minY { return 0 }
        if point.y > bounds.maxY { return source.utf16Count }
        guard let line = line(at: point, in: bounds) else { return 0 }
        let index = line.index(at: point.x - bounds.minX - line.flushOffset)
        return index == kCFNotFound ? source.utf16Count : min(max(0, index), source.utf16Count)
    }

    func link(at point: CGPoint, in bounds: CGRect) -> String? {
        guard bounds.contains(point), let line = line(at: point, in: bounds) else { return nil }
        let x = bounds.minX + line.flushOffset
        guard point.x >= x, point.x <= x + line.typographicWidth else { return nil }
        return source.link(at: index(at: point, in: bounds))
    }

    func selectionRects(_ selection: NSRange, in bounds: CGRect, dirty: CGRect) -> [CGRect] {
        guard selection.location >= 0, selection.length > 0,
              selection.location <= source.utf16Count,
              selection.length <= source.utf16Count - selection.location else { return [] }
        var result: [CGRect] = []
        for (line, baseline) in zip(lines, baselines) {
            let lo = max(selection.location, line.range.location)
            let hi = min(NSMaxRange(selection), NSMaxRange(line.range))
            guard hi > lo else { continue }
            let y = bounds.minY + baseline.rounded()
            guard y + line.descent >= dirty.minY, y - line.ascent <= dirty.maxY else { continue }
            let x0 = line.offsets(at: lo).primary, x1 = line.offsets(at: hi).primary
            result.append(CGRect(x: bounds.minX + line.flushOffset + min(x0, x1), y: y - line.ascent,
                                 width: max(1, abs(x1 - x0)), height: line.ascent + line.descent))
        }
        return result
    }

}
