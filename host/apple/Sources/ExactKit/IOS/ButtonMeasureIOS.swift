// @ref LLP 1069.011.001 D11: cached real UIKit height-for-width before publication.
#if os(iOS) || os(tvOS)
import UIKit
import CExact

final class ButtonMeasureCache: @unchecked Sendable {
    struct Key: Hashable {
        let face: Data
        let widthKind: UInt8
        let width: Float
        let traits: FieldChromeCache.Traits
    }
    private let lock = NSLock()
    private var traits = UITraitCollection.current
    private var entries: [Key: ExactButtonMeasure] = [:]
    private(set) var misses = 0
    @discardableResult
    func configure(_ next: UITraitCollection) -> Bool {
        precondition(Thread.isMainThread)
        lock.lock(); defer { lock.unlock() }
        let changed = FieldChromeCache.Traits(next) != FieldChromeCache.Traits(traits)
        traits = next
        return changed
    }
    func answer(face data: Data, widthKind: UInt8, width: Float) -> ExactButtonMeasure {
        lock.lock()
        let current = traits
        let key = Key(face: data, widthKind: widthKind, width: width, traits: .init(current))
        if let exact = entries[key] { lock.unlock(); return exact }
        misses += 1
        lock.unlock()
        let measured = Owner.shared.callMain { self.measure(ButtonFace(json: data), widthKind: widthKind, width: CGFloat(width), traits: current) }
        lock.lock(); entries[key] = measured; lock.unlock()
        // The existing Rust layout loop retries and withholds every provisional frame.
        var provisional = measured; provisional.provisional = 1
        return provisional
    }
    func measure(_ face: ButtonFace, widthKind: UInt8, width: CGFloat, traits: UITraitCollection) -> ExactButtonMeasure {
        precondition(Thread.isMainThread)
        var result = ExactButtonMeasure()
        traits.performAsCurrent {
            let button = UIButton(configuration: ControlHost.configuration(face).0)
            button.traitOverrides.preferredContentSizeCategory = traits.preferredContentSizeCategory
            button.traitOverrides.legibilityWeight = traits.legibilityWeight
            button.traitOverrides.displayScale = max(1, traits.displayScale)
            ButtonConfigurationIOS.apply(face, to: button, traits: traits, accent: nil)
            let size: CGSize
            if widthKind == 0 {
                button.bounds = CGRect(x: 0, y: 0, width: max(0, width), height: 0)
                if let config = button.configuration {
                    var contentWidth = width - config.contentInsets.leading - config.contentInsets.trailing
                    if config.imagePlacement == .leading || config.imagePlacement == .trailing, let image = config.image {
                        let configured = config.preferredSymbolConfigurationForImage.flatMap { image.applyingSymbolConfiguration($0) } ?? image
                        contentWidth -= configured.size.width + config.imagePadding
                    }
                    button.titleLabel?.preferredMaxLayoutWidth = max(1, contentWidth)
                    button.subtitleLabel?.preferredMaxLayoutWidth = max(1, contentWidth)
                }
                button.setNeedsLayout(); button.layoutIfNeeded()

                size = button.systemLayoutSizeFitting(CGSize(width: max(0, width), height: UIView.layoutFittingCompressedSize.height),
                    withHorizontalFittingPriority: .required, verticalFittingPriority: .fittingSizeLevel)
            } else {
                size = button.systemLayoutSizeFitting(widthKind == 1 ? UIView.layoutFittingCompressedSize : UIView.layoutFittingExpandedSize,
                    withHorizontalFittingPriority: .fittingSizeLevel, verticalFittingPriority: .fittingSizeLevel)
            }
            let scale = max(1, traits.displayScale)
            result = ExactButtonMeasure(width: Float(ceil(max(0, size.width) * scale) / scale),
                height: Float(ceil(max(0, size.height) * scale) / scale), provisional: 0)
        }
        return result
    }
}

extension TextEngine {
    static let buttonMeasure: ExactButtonMeasureFn = { ctx, pointer in
        guard let ctx, let request = pointer?.pointee, let bytes = request.face else { return ExactButtonMeasure() }
        let engine = Unmanaged<TextEngine>.fromOpaque(ctx).takeUnretainedValue()
        let data = Data(bytes: bytes, count: request.face_len)
        return engine.buttonMeasurements?.answer(face: data, widthKind: request.width_kind, width: request.width) ?? ExactButtonMeasure()
    }
}
#endif
