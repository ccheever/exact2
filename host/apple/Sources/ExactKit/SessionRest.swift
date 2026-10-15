// What a session gives back once scrolling settles (moved from Session.swift for its cap).
import Foundation

extension ExactSession {
    /// Scrolling has settled (iOS: `ScrollPump.restDelay`): the decoded
    /// images, shaped text and text pixels no view shows go, the tree's
    /// storage comes back to its live nodes, and the allocator returns the
    /// pages they leave. What shows keeps its own.
    func rest() {
        runtime.trim()
        rasters.trimCold()
        text.dropColdShaped()
        if let m = text.measurer { Owner.shared.post { m.dropColdShaped() } }
        #if os(iOS) || os(tvOS)
        presenter.textRasters.dropKept()
        #endif
        DispatchQueue.global(qos: .utility).async { malloc_zone_pressure_relief(nil, 0) }
    }
}
