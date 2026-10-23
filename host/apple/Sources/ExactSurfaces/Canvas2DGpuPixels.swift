import CoreGraphics
import Foundation

/// RGBA staging and its source image share a 32 MiB cost budget. A callback
/// leases its entry until its next call, even if another replay evicts it.
final class Canvas2DGpuPixels {
    static let shared = Canvas2DGpuPixels()
    static let budget = 32 << 20
    final class Entry {
        let image: CGImage, bytes: UnsafeMutableRawPointer
        let identity: UInt64, cost: Int
        weak var previous: Entry?
        var next: Entry?
        let key: String
        init(_ image: CGImage, _ bytes: UnsafeMutableRawPointer, identity: UInt64, cost: Int, key: String) {
            self.image = image; self.bytes = bytes; self.identity = identity; self.cost = cost; self.key = key
        }
        deinit { bytes.deallocate() }
    }
    private let lock = NSLock()
    private var pixels: [String: Entry] = [:]
    private var oldest: Entry?, newest: Entry?
    private var cost = 0
    private var identity: UInt64 = 0

    var retainedBytes: Int { lock.lock(); defer { lock.unlock() }; return cost }

    private func unlink(_ entry: Entry) {
        if let previous = entry.previous { previous.next = entry.next } else { oldest = entry.next }
        if let next = entry.next { next.previous = entry.previous } else { newest = entry.previous }
        entry.previous = nil; entry.next = nil
    }
    private func append(_ entry: Entry) {
        entry.previous = newest
        newest?.next = entry
        newest = entry
        if oldest == nil { oldest = entry }
    }
    private func remove(_ entry: Entry) {
        unlink(entry); pixels.removeValue(forKey: entry.key); cost -= entry.cost
    }
    func trim() {
        lock.lock(); defer { lock.unlock() }
        while let entry = oldest { remove(entry) }
    }
    func rgba(_ src: String, _ image: CGImage, p3: Bool) -> Entry? {
        lock.lock(); defer { lock.unlock() }
        let key = (p3 ? "p3 " : "srgb ") + src
        if let hit = pixels[key] {
            if hit.image === image { unlink(hit); append(hit); return hit }
            remove(hit)
        }
        let (w, h) = (image.width, image.height)
        guard w > 0, h > 0, w <= Int.max / 4 / h,
              image.bytesPerRow <= (Int.max - w * h * 4 - 256) / h else { return nil }
        // Charge the retained CGImage's decoded storage and both indexes too.
        let cost = w * h * 4 + image.bytesPerRow * h + 256
        while self.cost + min(cost, Self.budget) > Self.budget, let entry = oldest { remove(entry) }
        let bytes = UnsafeMutableRawPointer.allocate(byteCount: w * h * 4, alignment: 16)
        guard let c = CGContext(data: bytes, width: w, height: h, bitsPerComponent: 8, bytesPerRow: w * 4,
                                space: Canvas2DSpace(p3: p3).space,
                                bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue | CGBitmapInfo.byteOrder32Big.rawValue) else {
            bytes.deallocate(); return nil
        }
        c.clear(CGRect(x: 0, y: 0, width: w, height: h))
        c.draw(image, in: CGRect(x: 0, y: 0, width: w, height: h))
        // The ABI identity never recycles an allocation's address, including
        // after trim or same-handle replacement. Rust owns its copied pixels.
        identity += 1
        let entry = Entry(image, bytes, identity: identity, cost: cost, key: key)
        if cost <= Self.budget { pixels[key] = entry; append(entry); self.cost += cost }
        return entry
    }
}
