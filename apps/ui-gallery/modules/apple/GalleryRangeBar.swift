#if os(iOS)
import UIKit
import QuartzCore

final class RangeBarBackground: UIView {
    private let material = UIVisualEffectView(effect: UIBlurEffect(style: .systemMaterial))
    private let fadeMask = UIView()
    private let opacity = CAGradientLayer()

    override init(frame: CGRect) {
        super.init(frame: frame)
        isUserInteractionEnabled = false
        addSubview(material)
        opacity.colors = [UIColor.black.cgColor, UIColor.black.cgColor, UIColor.clear.cgColor]
        opacity.startPoint = CGPoint(x: 0.5, y: 0)
        opacity.endPoint = CGPoint(x: 0.5, y: 1)
        fadeMask.layer.addSublayer(opacity)
        material.mask = fadeMask
    }

    required init?(coder: NSCoder) { nil }

    override func layoutSubviews() {
        super.layoutSubviews()
        CATransaction.begin()
        CATransaction.setDisableActions(true)
        var column = bounds
        var responder: UIResponder? = superview
        while let current = responder {
            if let controller = current as? UIViewController, let navigation = controller.navigationController {
                column = convert(navigation.view.bounds, from: navigation.view)
                break
            }
            responder = current.next
        }
        // Cover the column above the bar too, including the area a collapsed header leaves
        let top = min(0, column.minY)
        material.frame = CGRect(x: column.minX, y: top, width: column.width, height: bounds.maxY + 16 - top)
        fadeMask.frame = material.bounds
        opacity.frame = material.bounds
        let start = max(0, bounds.maxY - 8 - material.frame.minY) / max(1, material.bounds.height)
        opacity.locations = [0, NSNumber(value: Double(start)), 1]
        CATransaction.commit()
    }
}
#endif
