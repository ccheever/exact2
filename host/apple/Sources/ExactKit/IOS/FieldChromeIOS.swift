// @ref LLP 1104 D4–D5: real UIKit measurements, shared by the two text engines.
#if os(iOS) || os(tvOS)
import UIKit
import CExact
import CoreText

final class FieldChromeCache: @unchecked Sendable {
    struct Traits: Hashable {
        let category: String
        let legibility: Int
        let scale: CGFloat
        let appearance: Int
        let contrast: Int
        init(_ traits: UITraitCollection) {
            category = traits.preferredContentSizeCategory.rawValue
            legibility = traits.legibilityWeight.rawValue
            scale = max(1, traits.displayScale)
            appearance = traits.userInterfaceStyle.rawValue
            contrast = traits.accessibilityContrast.rawValue
        }
    }
    struct Key: Hashable {
        let kind: UInt8
        let family: UInt16
        let name: String
        let size: CGFloat
        let weight: UInt16
        let italic: Bool
        let traits: Traits
    }
    private let lock = NSLock()
    private var entries: [Key: ExactFieldChrome] = [:]
    private var traits = UITraitCollection.current
    private(set) var body = UIFont.preferredFont(forTextStyle: .body)
    private(set) var presentedProvisional = 0
    private(set) var misses = 0
    // UITextField exposes text/editing rects, but no public bezel radius or
    // stroke API. This UITextView extension uses a 5pt radius and 0.5pt
    // separator stroke, checked beside a roundedRect field in the fixture.
    static let textareaRadius: CGFloat = 5
    static let textareaStroke: CGFloat = 0.5

    @discardableResult
    func configure(_ next: UITraitCollection) -> Bool {
        precondition(Thread.isMainThread)
        let font = UIFont.preferredFont(forTextStyle: .body, compatibleWith: next)
        lock.lock(); defer { lock.unlock() }
        let changed = Traits(traits) != Traits(next) || body != font
        traits = next; body = font
        return changed
    }
    func key(_ request: ExactFieldChromeRequest, font: UIFont) -> Key {
        lock.lock(); defer { lock.unlock() }
        return Key(kind: request.kind, family: request.family_id, name: font.fontName,
                   size: font.pointSize, weight: request.weight, italic: request.italic != 0,
                   traits: Traits(traits))
    }
    func prefill(family: UInt16, font: UIFont, weight: UInt16, italic: Bool) {
        precondition(Thread.isMainThread)
        for kind in UInt8(0)...UInt8(3) {
            let request = ExactFieldChromeRequest(kind: kind, family_id: family, size: Float(font.pointSize), weight: weight, italic: italic ? 1 : 0)
            let k = key(request, font: font)
            lock.lock(); let known = entries[k] != nil; lock.unlock()
            if !known { store(measure(k, font: font), for: k) }
        }
    }
    private func store(_ chrome: ExactFieldChrome, for key: Key) {
        lock.lock(); entries[key] = chrome; lock.unlock()
    }
    func answer(_ request: ExactFieldChromeRequest, font: UIFont) -> ExactFieldChrome {
        let k = key(request, font: font)
        lock.lock()
        if let exact = entries[k] { lock.unlock(); return exact }
        // Kind and current traits take priority; font size then chooses the
        // nearest entry. This answer is never emitted as a presented frame.
        let nearest = entries.min { a, b in
            func distance(_ other: Key) -> CGFloat {
                (other.kind == k.kind ? 0 : 10000) + (other.traits == k.traits ? 0 : 1000)
                    + (other.family == k.family ? 0 : 100) + abs(other.size - k.size)
            }
            return distance(a.key) < distance(b.key)
        }?.value ?? ExactFieldChrome()
        misses += 1
        lock.unlock()
        let measured = Owner.shared.callMain { self.measure(k, font: font) }
        store(measured, for: k)
        var provisional = nearest
        provisional.provisional = 1
        return provisional
    }
    func measure(_ key: Key, font: UIFont) -> ExactFieldChrome {
        precondition(Thread.isMainThread)
        lock.lock(); let current = traits; lock.unlock()
        var result = ExactFieldChrome()
        current.performAsCurrent {
            if key.kind == 3 {
                #if os(tvOS)
                // The read-only tvOS text view keeps today's zero-inset
                // presentation, with no iOS textarea bezel or editor chrome.
                result = ExactFieldChrome()
                #else
                let view = TextArea(frame: CGRect(x: 0, y: 0, width: 320, height: 100))
                view.font = font
                view.configureNativeChrome(true)
                let inset = view.textContainerInset
                result = ExactFieldChrome(top: Float(inset.top), right: Float(inset.right), bottom: Float(inset.bottom), left: Float(inset.left), minimum_height: 0, provisional: 0)
                #endif
                return
            }
            let field = UITextField(frame: .zero)
            #if !os(tvOS)
            field.borderStyle = .roundedRect
            #endif
            field.font = font
            field.isSecureTextEntry = key.kind == 1
            if key.kind == 2 { field.keyboardType = .webSearch }
            field.text = "Hg"
            let fitted = field.sizeThatFits(CGSize(width: 320, height: CGFloat.greatestFiniteMagnitude))
            let height = max(font.lineHeight, fitted.height)
            field.bounds = CGRect(x: 0, y: 0, width: 320, height: height)
            let rect = field.textRect(forBounds: field.bounds)
            result = ExactFieldChrome(top: Float(max(0, rect.minY)), right: Float(max(0, 320 - rect.maxX)),
                bottom: Float(max(0, height - rect.maxY)), left: Float(max(0, rect.minX)),
                minimum_height: Float(height), provisional: 0)
        }
        return result
    }
    func presented(_ provisional: Bool) {
        precondition(Thread.isMainThread)
        if provisional { presentedProvisional += 1 }
    }
}

extension TextEngine {
    // Register the platform descriptor alongside declared families. Its id is
    // picked after the plan catalog, so no Contract stack can collide with it.
    func registerControlFont(_ font: UIFont) -> UInt16 {
        let id = platformControlID ?? UInt16((catalog.keys.max() ?? 7) + 1)
        platformControlID = id; platformControlFont = font
        platformControlName = font.familyName.data(using: .utf8)! as NSData
        fonts.removeAll(keepingCapacity: true)
        return id
    }
    func controlFont(size: CGFloat, weight: Int, family: Int, italic: Bool) -> UIFont? {
        guard family == platformControlID.map(Int.init), let base = platformControlFont else { return nil }
        if size == base.pointSize, weight == Self.controlWeight(base), italic == base.fontDescriptor.symbolicTraits.contains(.traitItalic) { return base }
        let key = "control/\(family)/\(size)/\(weight)/\(italic)"
        if let font = fonts[key] { return font }
        let generic = self.font(size: size, weight: weight, family: 0, italic: italic)
        let traits = generic.fontDescriptor.object(forKey: .traits) as? [UIFontDescriptor.TraitKey: Any] ?? [:]
        var descriptor = base.fontDescriptor.addingAttributes([.traits: traits])
        if italic, let d = descriptor.withSymbolicTraits(.traitItalic) { descriptor = d }
        let font = TextEngine.cssWeight(UIFont(descriptor: descriptor, size: size), weight: weight, size: size)
        fonts[key] = font
        return font
    }
    static func controlWeight(_ font: UIFont) -> Int {
        if let axes = CTFontCopyVariation(font as CTFont) as? [NSNumber: NSNumber], let weight = axes[0x77676874] { return weight.intValue }
        let traits = font.fontDescriptor.object(forKey: .traits) as? [UIFontDescriptor.TraitKey: Any]
        let weight = traits?[.weight] as? CGFloat ?? 0
        return weight >= UIFont.Weight.bold.rawValue ? 700 : weight >= UIFont.Weight.semibold.rawValue ? 600 : 400
    }
    static let controlText: ExactControlTextFn = { ctx, kind in
        guard let ctx else { return ExactControlFont() }
        let engine = Unmanaged<TextEngine>.fromOpaque(ctx).takeUnretainedValue()
        guard let cache = engine.fieldChrome else { return ExactControlFont() }
        let font = Owner.shared.callMain {
            if kind == 0 { return cache.body }
            return engine.buttonMeasurements?.font(kind) ?? cache.body
        }
        let id = kind == 0 ? engine.registerControlFont(font) : engine.platformControlID ?? engine.registerControlFont(font)
        let weight = UInt16(controlWeight(font)), italic = font.fontDescriptor.symbolicTraits.contains(.traitItalic)
        if kind == 0 { Owner.shared.callMain {
            if let painter = engine.painter { _ = painter.registerControlFont(font) }
            cache.prefill(family: id, font: font, weight: weight, italic: italic)
        } }
        return ExactControlFont(family: engine.platformControlName.bytes.assumingMemoryBound(to: UInt8.self), family_len: engine.platformControlName.length,
                                family_id: id, size: Float(font.pointSize), weight: weight, italic: italic ? 1 : 0)
    }
    static let fieldChromeMeasure: ExactFieldChromeFn = { ctx, request in
        guard let ctx, let request = request?.pointee else { return ExactFieldChrome() }
        let engine = Unmanaged<TextEngine>.fromOpaque(ctx).takeUnretainedValue()
        let font = engine.font(size: CGFloat(request.size), weight: Int(request.weight), family: Int(request.family_id), italic: request.italic != 0)
        return engine.fieldChrome?.answer(request, font: font) ?? ExactFieldChrome()
    }
}
#endif
