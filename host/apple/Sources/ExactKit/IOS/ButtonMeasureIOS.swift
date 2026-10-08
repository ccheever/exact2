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
    private weak var surface: UIView?
    private var entries: [Key: ExactButtonMeasure] = [:]
    private(set) var misses = 0
    @discardableResult
    func configure(_ next: UITraitCollection, in surface: UIView) -> Bool {
        precondition(Thread.isMainThread)
        lock.lock(); defer { lock.unlock() }
        let changed = FieldChromeCache.Traits(next) != FieldChromeCache.Traits(traits)
        traits = next
        self.surface = surface
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
            // A configured button reads Dynamic Type from its window during fitting.
            // Keep the probe in that hierarchy, hidden, without publishing any geometry.
            let container = UIView()
            container.isHidden = true
            surface?.addSubview(container)
            defer { container.removeFromSuperview() }
            container.traitOverrides.preferredContentSizeCategory = traits.preferredContentSizeCategory
            container.traitOverrides.legibilityWeight = traits.legibilityWeight
            container.traitOverrides.displayScale = max(1, traits.displayScale)
            let button = UIButton(configuration: ControlHost.configuration(face).0)
            container.addSubview(button)
            button.updateTraitsIfNeeded()
            ButtonConfigurationIOS.apply(face, to: button, traits: traits, accent: nil)
            // Auto Layout fitting returns a single-line height even for a wrapped
            // configured title. sizeThatFits matches required-width window layout.
            let offeredWidth = widthKind == 0 ? max(0, width)
                : widthKind == 1 ? 0 : CGFloat.greatestFiniteMagnitude
            button.layoutIfNeeded()
            let size = button.sizeThatFits(CGSize(width: offeredWidth, height: .greatestFiniteMagnitude))
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
