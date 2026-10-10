#if os(iOS)
// @ref llp/1109.002-design-system-parity.spec.md#icons-and-imagery
// upstream 365aa87982 AppSymbol.ios.tsx: preserve SF name, point size and weight.
// GAP 002: the portable symbol role roster cannot express these SF names.
import UIKit

final class T3SymbolView: ExactNativeInstance {
    static let factory = ExactNativeFactory { props, events in
        let instance = T3SymbolView(events: events)
        try instance.setProps(props)
        return instance
    }
    private let image = UIImageView()
    override var view: UIView { image }

    override init(events: ExactNativeEvents) {
        super.init(events: events)
        image.contentMode = .center
        image.isUserInteractionEnabled = false
        image.isAccessibilityElement = false
    }

    override func setProps(_ props: [String: String]) throws {
        let name = props["symbol-name"] ?? ""
        let size = Double(props["point-size"] ?? "16") ?? 16
        let weights: [String: UIImage.SymbolWeight] = [
            "ultraLight": .ultraLight, "thin": .thin, "light": .light,
            "regular": .regular, "medium": .medium, "semibold": .semibold,
            "bold": .bold, "heavy": .heavy, "black": .black,
        ]
        guard size.isFinite, size > 0, size <= 512,
              let weight = weights[props["symbol-weight"] ?? "regular"],
              let symbol = UIImage(systemName: name, withConfiguration:
                UIImage.SymbolConfiguration(pointSize: size, weight: weight, scale: .medium)) else {
            throw ExactNativeRefusal("Invalid mobile SF Symbol configuration: \(name)")
        }
        image.image = symbol.withRenderingMode(.alwaysTemplate)
        image.tintColor = try Self.color(props["symbol-tint"] ?? "#27272a")
    }

    static func color(_ text: String) throws -> UIColor {
        if text.hasPrefix("#") {
            let hex = String(text.dropFirst())
            if let value = UInt64(hex, radix: 16), hex.count == 6 || hex.count == 8 {
                let rgb = hex.count == 8 ? value >> 8 : value
                return UIColor(red: CGFloat((rgb >> 16) & 255) / 255,
                               green: CGFloat((rgb >> 8) & 255) / 255,
                               blue: CGFloat(rgb & 255) / 255,
                               alpha: hex.count == 8 ? CGFloat(value & 255) / 255 : 1)
            }
        }
        if text.hasPrefix("rgba("), text.hasSuffix(")") {
            let channels = text.dropFirst(5).dropLast().split(separator: ",").compactMap {
                Double($0.trimmingCharacters(in: .whitespaces))
            }
            if channels.count == 4, channels.allSatisfy({ $0.isFinite }) {
                return UIColor(red: channels[0] / 255, green: channels[1] / 255,
                               blue: channels[2] / 255, alpha: channels[3])
            }
        }
        throw ExactNativeRefusal("Unsupported mobile symbol tint: \(text)")
    }
}

#endif
