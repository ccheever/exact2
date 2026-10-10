// @ref LLP 1069.011.001 D11: AppKit fitting answers precede publication.
#if os(macOS)
import AppKit
import CExact

final class ButtonMeasureCache: @unchecked Sendable {
    struct Key: Hashable {
        let face: Data
        let widthKind: UInt8
        let width: Float
        let traits: FieldChromeCache.Traits
    }
    private let lock = NSLock()
    private var appearance = NSAppearance(named: .aqua)!
    private var traits = FieldChromeCache.Traits(appearance: NSAppearance.Name.aqua.rawValue, scale: 1)
    private var entries: [Key: ExactButtonMeasure] = [:]
    private(set) var misses = 0
    @discardableResult
    func configure(_ next: NSAppearance, scale: CGFloat) -> Bool {
        precondition(Thread.isMainThread)
        let nextTraits = FieldChromeCache.Traits(appearance: next.name.rawValue, scale: max(1, scale))
        lock.lock(); defer { lock.unlock() }
        let changed = traits != nextTraits
        appearance = next; traits = nextTraits
        return changed
    }
    func answer(face data: Data, widthKind: UInt8, width: Float) -> ExactButtonMeasure {
        lock.lock()
        let current = appearance, currentTraits = traits
        let key = Key(face: data, widthKind: widthKind, width: width, traits: currentTraits)
        if let exact = entries[key] { lock.unlock(); return exact }
        misses += 1
        lock.unlock()
        let measured = Owner.shared.callMain {
            self.measure(ButtonFace(json: data), widthKind: widthKind, width: CGFloat(width), appearance: current, scale: currentTraits.scale)
        }
        lock.lock(); entries[key] = measured; lock.unlock()
        var provisional = measured; provisional.provisional = 1
        return provisional
    }
    func measure(_ face: ButtonFace, widthKind: UInt8, width: CGFloat, appearance: NSAppearance, scale: CGFloat) -> ExactButtonMeasure {
        precondition(Thread.isMainThread)
        var result = ExactButtonMeasure()
        appearance.performAsCurrentDrawingAppearance {
            let button = NSButton(title: "", target: nil, action: nil)
            ButtonConfigurationMac.apply(face, to: button, appearance: appearance, accent: nil)
            if widthKind == 1 {
                var minimum = face
                minimum.title = TextEngine.widestButtonRun(face.title, whiteSpace: face.rows.title["white_space"]?.string) { run in
                    var candidate = face; candidate.title = run
                    ButtonConfigurationMac.apply(candidate, to: button, appearance: appearance, accent: nil)
                    return button.fittingSize.width
                }
                // AppKit's declared single-line stand-in truncates in this box;
                // it must not give the flex item a max-content auto minimum.
                ButtonConfigurationMac.apply(minimum, to: button, appearance: appearance, accent: nil)
            }
            // Push buttons do not wrap at the offered width. A required width
            // constraint makes fittingSize answer the real constrained frame;
            // window/Auto Layout ground truth is covered by the macOS tests.
            if widthKind == 0 {
                button.translatesAutoresizingMaskIntoConstraints = false
                button.widthAnchor.constraint(equalToConstant: max(0, width)).isActive = true
            }
            let size = button.fittingSize
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
        return engine.buttonMeasurements?.answer(face: Data(bytes: bytes, count: request.face_len),
            widthKind: request.width_kind, width: request.width) ?? ExactButtonMeasure()
    }
}
#endif
