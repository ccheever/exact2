// The archive's baked compatibility document (LLP 1030 D3a), parsed: what
// the core reads of its own build (its trust), and the GPU module checks.
import Foundation

package enum BakedCompatibility {
    /// `compat.json`, as the archive carries it.
    package static var json: [String: Any] {
        let bytes = Runtime.bakedCompat()
        return (try? JSONSerialization.jsonObject(with: bytes) as? [String: Any]) ?? [:]
    }
}
