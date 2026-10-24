// The archive's baked compatibility document (LLP 1030 D3a), parsed: what
// the core reads of its own build (its trust), and the GPU module checks.
import Foundation

package enum BakedCompatibility {
    /// `compat.json`, as the archive carries it.
    package static var json: [String: Any] {
        let bytes = Runtime.bakedCompat()
        return (try? JSONSerialization.jsonObject(with: bytes) as? [String: Any]) ?? [:]
    }
    /// Whether `compat` is a production build's: its `embedded.trust`, the
    /// build's resolved trust, which bake records beside the cohort id (an
    /// updater-free binary's `inputs.trust` is null, LLP 1030 D4). The one
    /// reading every production policy goes through, as
    /// `exact_runner::delivery::production` is on the Rust side.
    package static func production(_ compat: [String: Any]) -> Bool {
        (compat["embedded"] as? [String: Any])?["trust"] as? String == "production"
    }
}
