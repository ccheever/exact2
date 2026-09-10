// The emoji picker emits a selection, never its keyboard's search text or
// ordinary typing after the user switches input modes. LLP 1008 §9.
import Foundation

enum EmojiSelection {
    static func accepts(_ value: String) -> Bool {
        guard value.count == 1 else { return false }
        let scalars = value.unicodeScalars
        return scalars.contains { $0.properties.isEmojiPresentation }
            || (scalars.contains { $0.value == 0xFE0F || $0.value == 0x20E3 }
                && scalars.contains { $0.properties.isEmoji })
    }
}
