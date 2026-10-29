import Foundation
import QuartzCore

/// Coalesce reader updates across the process. Common run-loop modes keep scrolling live;
/// existing hooks and lifecycle changes supply updates without a polling timer.
enum FrameHealthPublication {
    private final class Reader { weak var health: FrameHealth? }
    private static var readers = (0..<16).map { _ in Reader() }
    private static var next: Double = 0
    private static var pending = false

    static func update(_ health: FrameHealth) {
        guard Thread.isMainThread else { return }
        for reader in readers where reader.health?.handle == 0 || (reader.health?.changed == nil && reader.health?.inspected == nil) { reader.health = nil }
        if readers.allSatisfy({ $0.health == nil }) { next = 0 }
        guard health.enabled, health.changed != nil || health.inspected != nil,
              !readers.contains(where: { $0.health === health }) else { return }
        readers.first(where: { $0.health == nil })?.health = health
    }

    static func request(at now: Double, force: Bool = false) {
        guard !pending, force || now >= next else { return }
        if !force { next = now + 0.5 }
        pending = true
        CFRunLoopPerformBlock(CFRunLoopGetMain(), CFRunLoopMode.commonModes.rawValue) {
            pending = false
            for reader in readers {
                guard let health = reader.health, health.publicationDirty else { continue }
                health.publicationDirty = false
                autoreleasepool {
                    health.changed?()
                    health.inspected?()
                }
            }
        }
        CFRunLoopWakeUp(CFRunLoopGetMain())
    }
}
