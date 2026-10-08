// @ref LLP 1104 D4–D5: AppKit's control font and real bezel measurements.
#if os(macOS)
import AppKit
import CExact
import CoreText

final class FieldChromeCache: @unchecked Sendable {
    struct Traits: Hashable {
        let appearance: String
        let scale: CGFloat
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
    private var appearance = NSAppearance(named: .aqua)!
    private var traits = Traits(appearance: NSAppearance.Name.aqua.rawValue, scale: 1)
    private(set) var body = NSFont.systemFont(ofSize: NSFont.systemFontSize(for: .regular))
    private(set) var presentedProvisional = 0
    private(set) var misses = 0

    @discardableResult
    func configure(_ next: NSAppearance, scale: CGFloat) -> Bool {
        precondition(Thread.isMainThread)
        var font = body
        next.performAsCurrentDrawingAppearance {
            font = NSFont.systemFont(ofSize: NSFont.systemFontSize(for: .regular))
        }
        let nextTraits = Traits(appearance: next.name.rawValue, scale: max(1, scale))
        lock.lock(); defer { lock.unlock() }
        let changed = traits != nextTraits || body != font
        appearance = next; traits = nextTraits; body = font
        return changed
    }
    func key(_ request: ExactFieldChromeRequest, font: NSFont) -> Key {
        lock.lock(); defer { lock.unlock() }
        return Key(kind: request.kind, family: request.family_id, name: font.fontName,
                   size: font.pointSize, weight: request.weight, italic: request.italic != 0, traits: traits)
    }
    func prefill(family: UInt16, font: NSFont, weight: UInt16, italic: Bool) {
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
    func answer(_ request: ExactFieldChromeRequest, font: NSFont) -> ExactFieldChrome {
        let k = key(request, font: font)
        lock.lock()
        if let exact = entries[k] { lock.unlock(); return exact }
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
    // Search keeps today's NSTextField. Its kind still has its own cache entry.
    static func platformField(kind: UInt8) -> NSTextField {
        kind == 1 ? NSSecureTextField(frame: .zero) : NSTextField(frame: .zero)
    }
    /// Preserve NSTextView's standard space around glyphs while the kernel
    /// owns the wrapping width (and Exact keeps lineFragmentPadding at zero).
    private(set) static var textareaInsetMeasurements = 0
    private static var textareaInsets: [Traits: NSSize] = [:]
    static var textareaInset: NSSize { textareaInset(for: NSAppearance(named: .aqua)!, scale: 1) }
    static func textareaInset(for appearance: NSAppearance, scale: CGFloat) -> NSSize {
        precondition(Thread.isMainThread)
        let traits = Traits(appearance: appearance.name.rawValue, scale: max(1, scale))
        if let inset = textareaInsets[traits] { return inset }
        var inset = NSSize.zero
        appearance.performAsCurrentDrawingAppearance {
            let editor = NSTextView(usingTextLayoutManager: true)
            let standard = editor.textContainerInset
            inset = NSSize(width: standard.width + (editor.textContainer?.lineFragmentPadding ?? 0), height: standard.height)
        }
        textareaInsetMeasurements += 1
        textareaInsets[traits] = inset
        return inset
    }
    func measure(_ key: Key, font: NSFont) -> ExactFieldChrome {
        precondition(Thread.isMainThread)
        lock.lock(); let current = appearance; lock.unlock()
        var result = ExactFieldChrome()
        current.performAsCurrentDrawingAppearance {
            if key.kind == 3 {
                let scroll = NSScrollView(frame: NSRect(x: 0, y: 0, width: 320, height: 100))
                scroll.borderType = .bezelBorder
                let inset = Self.textareaInset(for: current, scale: key.traits.scale)
                let rect = scroll.contentView.frame.insetBy(dx: inset.width, dy: inset.height)
                result = ExactFieldChrome(top: Float(rect.minY), right: Float(320 - rect.maxX),
                    bottom: Float(100 - rect.maxY), left: Float(rect.minX), minimum_height: 0, provisional: 0)
                return
            }
            let field = Self.platformField(kind: key.kind)
            field.font = font; field.stringValue = "Hg"
            let height = field.fittingSize.height
            let bounds = NSRect(x: 0, y: 0, width: 320, height: height)
            let rect = field.cell!.drawingRect(forBounds: bounds)
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
    func registerControlFont(_ font: NSFont) -> UInt16 {
        let id = platformControlID ?? UInt16((catalog.keys.max() ?? 7) + 1)
        platformControlID = id; platformControlFont = font
        platformControlName = (font.familyName ?? font.fontName).data(using: .utf8)! as NSData
        fonts.removeAll(keepingCapacity: true)
        return id
    }
    func controlFont(size: CGFloat, weight: Int, family: Int, italic: Bool) -> NSFont? {
        guard family == platformControlID.map(Int.init), let base = platformControlFont else { return nil }
        if size == base.pointSize, weight == Self.controlWeight(base), italic == base.fontDescriptor.symbolicTraits.contains(.italic) { return base }
        let key = "control/\(family)/\(size)/\(weight)/\(italic)"
        if let font = fonts[key] { return font }
        let generic = self.font(size: size, weight: weight, family: 0, italic: italic)
        let traits = generic.fontDescriptor.object(forKey: .traits) as? [NSFontDescriptor.TraitKey: Any] ?? [:]
        var descriptor = base.fontDescriptor.addingAttributes([.traits: traits])
        if italic { descriptor = descriptor.withSymbolicTraits(.italic) }
        let font = TextEngine.cssWeight(NSFont(descriptor: descriptor, size: size) ?? generic, weight: weight, size: size)
        fonts[key] = font
        return font
    }
    static func controlWeight(_ font: NSFont) -> Int {
        if let axes = CTFontCopyVariation(font as CTFont) as? [NSNumber: NSNumber], let weight = axes[0x77676874] { return weight.intValue }
        let traits = font.fontDescriptor.object(forKey: .traits) as? [NSFontDescriptor.TraitKey: Any]
        let weight = traits?[.weight] as? CGFloat ?? 0
        return weight >= NSFont.Weight.bold.rawValue ? 700 : weight >= NSFont.Weight.semibold.rawValue ? 600 : 400
    }
    static let controlText: ExactControlTextFn = { ctx in
        guard let ctx else { return ExactControlFont() }
        let engine = Unmanaged<TextEngine>.fromOpaque(ctx).takeUnretainedValue()
        guard let cache = engine.fieldChrome else { return ExactControlFont() }
        let font = Owner.shared.callMain { cache.body }
        let id = engine.registerControlFont(font)
        let weight = UInt16(controlWeight(font)), italic = font.fontDescriptor.symbolicTraits.contains(.italic)
        Owner.shared.callMain {
            if let painter = engine.painter { _ = painter.registerControlFont(font) }
            cache.prefill(family: id, font: font, weight: weight, italic: italic)
        }
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
