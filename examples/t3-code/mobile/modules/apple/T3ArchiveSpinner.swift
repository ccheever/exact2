#if os(iOS)
// @ref llp/1109.004-home-projection.decision.md#decision
// Pinned ArchivedThreadsScreen uses the native small ActivityIndicator, accent-icon.
import UIKit

final class T3ArchiveSpinner: ExactNativeInstance {
    static let factory = ExactNativeFactory { props, events in
        let instance = T3ArchiveSpinner(events: events)
        try instance.setProps(props)
        return instance
    }
    private let indicator = UIActivityIndicatorView(style: .medium)
    override var view: UIView { indicator }

    override init(events: ExactNativeEvents) {
        super.init(events: events)
        indicator.isUserInteractionEnabled = false
        indicator.isAccessibilityElement = false
        indicator.startAnimating()
    }

    override func setProps(_ props: [String: String]) throws {
        let text = props["spinner-tint"] ?? "#71717a"
        if text.hasPrefix("#"), let hex = UInt64(text.dropFirst(), radix: 16), text.count == 7 || text.count == 9 {
            let rgb = text.count == 9 ? hex >> 8 : hex
            indicator.color = UIColor(red: CGFloat((rgb >> 16) & 255) / 255,
                green: CGFloat((rgb >> 8) & 255) / 255, blue: CGFloat(rgb & 255) / 255,
                alpha: text.count == 9 ? CGFloat(hex & 255) / 255 : 1)
        } else if text.hasPrefix("rgba("), text.hasSuffix(")") {
            let channels = text.dropFirst(5).dropLast().split(separator: ",").compactMap { Double($0.trimmingCharacters(in: .whitespaces)) }
            guard channels.count == 4, channels.allSatisfy({ $0.isFinite }) else {
                throw ExactNativeRefusal("Invalid archive indicator tint")
            }
            indicator.color = UIColor(red: channels[0] / 255, green: channels[1] / 255, blue: channels[2] / 255, alpha: channels[3])
        } else {
            throw ExactNativeRefusal("Unsupported archive indicator tint: \(text)")
        }
    }

    override func destroy() { indicator.stopAnimating() }
}
#endif
