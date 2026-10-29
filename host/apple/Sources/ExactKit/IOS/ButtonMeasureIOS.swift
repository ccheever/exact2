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
    /// Read UIKit's configured title font, including buttonSize and current traits.
    func font(_ kind: UInt8) -> UIFont {
        precondition(Thread.isMainThread)
        let container = UIView(); container.isHidden = true
        surface?.addSubview(container); defer { container.removeFromSuperview() }
        container.traitOverrides.preferredContentSizeCategory = traits.preferredContentSizeCategory
        container.traitOverrides.legibilityWeight = traits.legibilityWeight
        var config = UIButton.Configuration.bordered(); config.title = "Title"
        let sizes: [UIButton.Configuration.Size] = [.mini, .small, .medium, .large]
        config.buttonSize = sizes[Int(kind) - 1]
        let button = UIButton(configuration: config); container.addSubview(button)
        button.updateTraitsIfNeeded(); button.layoutIfNeeded()
        return button.titleLabel!.font
    }
    /// A miss is measured on main before it is answered, so the first layout
    /// with a button of a new face, offer or traits is already exact. Only
    /// when the traits change between the key and the measurement is the
    /// answer provisional, and the host lays out again under the new key.
    func answer(face data: Data, widthKind: UInt8, width: Float) -> ExactButtonMeasure {
        lock.lock()
        let key = Key(face: data, widthKind: widthKind, width: width, traits: .init(traits))
        if let exact = entries[key] { lock.unlock(); return exact }
        let numeral = Self.numeral(data).map { (key: Key(face: $0.face, widthKind: widthKind, width: width, traits: key.traits), title: $0.title) }
        if let numeral, let known = numerals[numeral.key], let measured = known.answer(numeral.title, widthKind: widthKind, width: width) {
            entries[key] = measured
            lock.unlock()
            return measured
        }
        misses += 1
        lock.unlock()
        return Owner.shared.callMain {
            lock.lock(); let current = traits; lock.unlock()
            let now = Key(face: data, widthKind: widthKind, width: width, traits: .init(current))
            var title: NSAttributedString?
            var measured = measure(ButtonFace(json: data), widthKind: widthKind, width: CGFloat(width), traits: current, probe: data, title: &title)
            lock.lock()
            entries[now] = measured
            if let numeral, now == key, let title { numerals[numeral.key] = Numeral(measured, title: title, scale: max(1, current.displayScale)) }
            lock.unlock()
            measured.provisional = now == key ? 0 : 1
            return measured
        }
    }

    /// A count's button (a like's, a reply's) measured once for its face:
    /// its title is digits, on one line it cannot break, so another count's
    /// button is that one wider or narrower by the difference of the two
    /// titles' ink, as UIKit lays its label out at its text's width rounded
    /// up to the pixel. Measured in the title's own attributes, off the main
    /// thread: every count is a new face, and each asked a configured button
    /// on main (a feed's posts, ~1 ms each).
    private struct Numeral {
        let measured: ExactButtonMeasure
        let title: NSAttributedString
        let scale: CGFloat
        let ink: CGFloat
        init(_ measured: ExactButtonMeasure, title: NSAttributedString, scale: CGFloat) {
            self.measured = measured; self.title = title; self.scale = scale
            ink = Self.ink(title, scale: scale)
        }
        static func ink(_ title: NSAttributedString, scale: CGFloat) -> CGFloat {
            let width = title.boundingRect(with: CGSize(width: CGFloat.greatestFiniteMagnitude, height: .greatestFiniteMagnitude),
                                           options: [.usesLineFragmentOrigin, .usesFontLeading], context: nil).width
            return ceil(width * scale) / scale
        }
        func answer(_ text: String, widthKind: UInt8, width: Float) -> ExactButtonMeasure? {
            guard title.length > 0 else { return nil }
            let other = NSMutableAttributedString(attributedString: title)
            other.replaceCharacters(in: NSRange(location: 0, length: other.length), with: text)
            var result = measured
            result.width += Float(Self.ink(other, scale: scale) - ink)
            // A definite offer it would not fit is measured, wrapped or not.
            if widthKind == 0 && result.width > width { return nil }
            return result
        }
    }
    private var numerals: [Key: Numeral] = [:]
    /// A face whose title is a count (digits, with `,` or `.`), keyed
    /// without its title or its label, which measures nothing.
    static func numeral(_ data: Data) -> (face: Data, title: String)? {
        guard var o = try? JSONSerialization.jsonObject(with: data) as? [String: Any], let title = o["title"] as? String,
              title.contains(where: \.isASCIIDigit), title.allSatisfy({ $0.isASCIIDigit || $0 == "," || $0 == "." }),
              (o["subtitle"] as? String)?.isEmpty ?? true else { return nil }
        o["title"] = "#"
        o["label"] = nil
        guard let face = try? JSONSerialization.data(withJSONObject: o, options: [.sortedKeys]) else { return nil }
        return (face, title)
    }
    /// The configured buttons a run of measurements shares, by face. A layout
    /// asks one face for its max-content, min-content and definite widths in
    /// turn; each is answered by the same configured button. The probes go at
    /// the end of the main turn that made them.
    private var probes: [Data: UIButton] = [:]
    private var probeContainer: UIView?
    private var probeTraits: FieldChromeCache.Traits?
    private func probe(_ face: ButtonFace, key: Data?, traits: UITraitCollection) -> (UIButton, configured: Bool) {
        let keyed = FieldChromeCache.Traits(traits)
        if probeTraits != keyed { dropProbes() }
        if let key, let button = probes[key] { return (button, true) }
        let container: UIView
        if let reused = probeContainer { container = reused } else {
            // A configured button reads Dynamic Type from its window during fitting.
            // Keep the probe in that hierarchy, hidden, without publishing any geometry.
            container = UIView()
            container.isHidden = true
            surface?.addSubview(container)
            container.traitOverrides.preferredContentSizeCategory = traits.preferredContentSizeCategory
            container.traitOverrides.legibilityWeight = traits.legibilityWeight
            container.traitOverrides.displayScale = max(1, traits.displayScale)
            probeContainer = container; probeTraits = keyed
            DispatchQueue.main.async { [weak self] in self?.dropProbes() }
        }
        let button = UIButton(configuration: ControlHost.configuration(face).0)
        container.addSubview(button)
        button.updateTraitsIfNeeded()
        if let key { probes[key] = button }
        return (button, false)
    }
    private func dropProbes() {
        probeContainer?.removeFromSuperview()
        probeContainer = nil; probeTraits = nil
        probes = [:]
    }
    /// `probe` names the face's bytes, whose configured button later
    /// measurements in this main turn reuse; without it the button is fresh.
    func measure(_ face: ButtonFace, widthKind: UInt8, width: CGFloat, traits: UITraitCollection, probe key: Data? = nil) -> ExactButtonMeasure {
        var title: NSAttributedString?
        return measure(face, widthKind: widthKind, width: width, traits: traits, probe: key, title: &title)
    }
    /// `title`: the configured title as its label lays it out.
    func measure(_ face: ButtonFace, widthKind: UInt8, width: CGFloat, traits: UITraitCollection, probe key: Data?, title: inout NSAttributedString?) -> ExactButtonMeasure {
        HostPhases.shared.count(.nativeControlMeasurements)
        precondition(Thread.isMainThread)
        var result = ExactButtonMeasure()
        var shown: NSAttributedString?
        traits.performAsCurrent {
            let (button, configured) = probe(face, key: key, traits: traits)
            defer { if key == nil { button.removeFromSuperview() } }
            if !configured { ButtonConfigurationIOS.apply(face, to: button, traits: traits, accent: nil) }
            // Auto Layout fitting returns a single-line height even for a wrapped
            // configured title. sizeThatFits matches required-width window layout.
            var minimumWidth: CGFloat?
            if widthKind == 1 {
                // sizeThatFits(0) returns the unwrapped title, not min-content.
                // Ask the same configured control for its widest unbreakable
                // title/subtitle, including the native insets, symbol and gap.
                var minimum = face
                minimum.title = TextEngine.widestButtonRun(face.title, whiteSpace: face.rows.title["white_space"]?.string) { run in
                    var candidate = face; candidate.title = run; candidate.subtitle = nil
                    ButtonConfigurationIOS.apply(candidate, to: button, traits: traits, accent: nil)
                    return button.sizeThatFits(CGSize(width: CGFloat.greatestFiniteMagnitude, height: CGFloat.greatestFiniteMagnitude)).width
                }
                minimum.subtitle = TextEngine.widestButtonRun(face.subtitle, whiteSpace: face.rows.subtitle["white_space"]?.string) { run in
                    var candidate = face; candidate.title = nil; candidate.subtitle = run
                    ButtonConfigurationIOS.apply(candidate, to: button, traits: traits, accent: nil)
                    return button.sizeThatFits(CGSize(width: CGFloat.greatestFiniteMagnitude, height: CGFloat.greatestFiniteMagnitude)).width
                }
                ButtonConfigurationIOS.apply(minimum, to: button, traits: traits, accent: nil)
                minimumWidth = button.sizeThatFits(CGSize(width: CGFloat.greatestFiniteMagnitude, height: CGFloat.greatestFiniteMagnitude)).width
                ButtonConfigurationIOS.apply(face, to: button, traits: traits, accent: nil)
            }
            let offeredWidth = widthKind == 0 ? max(0, width) : minimumWidth ?? CGFloat.greatestFiniteMagnitude
            button.layoutIfNeeded()
            let size = button.sizeThatFits(CGSize(width: offeredWidth, height: .greatestFiniteMagnitude))
            shown = button.titleLabel?.attributedText
            let scale = max(1, traits.displayScale)
            // A content width carries one device pixel more than the title
            // needs. The box is drawn between its edges rounded to the pixel
            // grid (PixelGrid), which can come out a pixel narrower than
            // layout's width, and UIKit wraps a title that does not fit its
            // drawn bounds: Recipe Finder's centred "Find a recipe" broke
            // onto a second line spilling out of its capsule (round 27).
            let slack = widthKind == 0 ? 0 : 1 / scale
            result = ExactButtonMeasure(width: Float((ceil(max(0, minimumWidth ?? size.width) * scale) / scale) + slack),
                height: Float(ceil(max(0, size.height) * scale) / scale), provisional: 0)
        }
        title = shown
        return result
    }
}

private extension Character {
    var isASCIIDigit: Bool { isASCII && isWholeNumber }
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
