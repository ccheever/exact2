// A textarea's decorations (LLP 1114.007): the app's spans painted over the
// field's TextKit 2 layout, never in its text storage, so the value, typing
// attributes, undo history, layout and intrinsic size are the undecorated
// field's. Colours are rendering attributes on the text layout manager
// (NSTextLayoutManager `addRenderingAttribute(_:value:for:)`), applied again
// to each fragment TextKit lays out anew; TextKit 2 draws a rendering
// attribute's foreground and background colour but not its underline or
// strikethrough, so the field draws those lines itself, at its font's
// underline position, under the range's visual segments. Nothing is
// repainted while text is marked; the paint catches up when it commits.
// `text-shadow` is not painted here.
#if os(macOS)
import AppKit
#else
import UIKit
#endif
import CExact

final class FieldDecorations {
    /// What the agent's `state` reports: the record's text, whether the
    /// spans painted are provisional, and the spans as painted.
    private(set) var state: [String: Any] = [:]
    private var painted = false
    private var warned: Set<String> = []
    /// The colours painted, again for each fragment TextKit lays out anew,
    /// which drops the rendering attributes it held (`renderingAttributesValidator`).
    private var paints: [(range: NSTextRange, attributes: [NSAttributedString.Key: Any])] = []
    /// Underlined and struck runs, UTF-16 ranges with their ink, for `drawLines`.
    private var lines: [(range: NSRange, underline: Bool, strike: Bool, ink: PlatformColor?)] = []

    func restyle(_ node: NodeView) {
        guard let field = node.textArea, !node.fieldComposing else { return }
        guard let manager = field.textLayoutManager, let content = manager.textContentManager else {
            if warned.insert("").inserted { node.presenter?.session?.log("decorations: this field has no TextKit 2 layout manager, so nothing is painted") }
            return
        }
        let source = node.fieldText
        let reply = node.props["decorations"].flatMap { record in
            Self.resolve(record: record, current: source, styles: node.props["decorationStyles"] ?? "{}")
        }
        if painted { manager.setRenderingAttributes([:], for: content.documentRange) }
        painted = false
        defer { (field as? TextArea)?.setNeedsDisplayOfWholeField() }
        lines = []
        paints = []
        // Each painter owns the validator; one detached leaves none behind.
        manager.renderingAttributesValidator = { [weak self, weak node] manager, fragment in
            guard let self, node?.fieldComposing == false else { return }
            for paint in paints {
                guard let range = paint.range.intersection(fragment.rangeInElement) else { continue }
                for (key, value) in paint.attributes { manager.addRenderingAttribute(key, value: value, for: range) }
            }
        }
        guard let reply, let spans = reply["spans"] as? [[String: Any]] else {
            if node.props["decorations"] != nil, warned.insert("record").inserted { node.presenter?.session?.log("decorations refused: the record or its declarations do not parse") }
            state = [:]
            return
        }
        let styles = reply["styles"] as? [String: [String: Any]] ?? [:]
        var shown: [[String: Any]] = []
        // Per code unit, what the lines are and their ink, as the spans leave them.
        let length = source.utf16.count
        var line = [(underline: Bool, strike: Bool)?](repeating: nil, count: length)
        var ink = [PlatformColor?](repeating: nil, count: length)
        // Spans apply in order, attribute by attribute: a later span wins
        // where two overlap, as later highlights paint over earlier ones.
        for span in spans {
            guard let name = span["name"] as? String, let start = span["start"] as? Int, let end = span["end"] as? Int,
                  let range = Self.textRange(content, start, end) else { continue }
            guard let rows = styles[name] else {
                if warned.insert(name).inserted { node.presenter?.session?.log("decorations: no decoration \"\(name)\"") }
                continue
            }
            let attributes = Self.attributes(rows, dark: node.drawsDark)
            for (key, value) in attributes { manager.addRenderingAttribute(key, value: value, for: range) }
            if !attributes.isEmpty { paints.append((range, attributes)) }
            let units = start..<min(end, length)
            if let color = attributes[.foregroundColor] as? PlatformColor { for i in units { ink[i] = color } }
            if let words = rows["text_decoration_line"] as? String {
                for i in units { line[i] = (words.contains("underline"), words.contains("line-through")) }
            }
            painted = true
            shown.append(["name": name, "start": start, "end": end])
        }
        var at = 0
        while at < length {
            guard let here = line[at], here.underline || here.strike else { at += 1; continue }
            var end = at + 1
            while end < length, let next = line[end], next == here, ink[end] == ink[at] { end += 1 }
            lines.append((NSRange(location: at, length: end - at), here.underline, here.strike, ink[at]))
            at = end
        }
        if let record = node.props["decorations"].flatMap({ try? JSONSerialization.jsonObject(with: Data($0.utf8)) as? [String: Any] }) {
            state = ["source": source, "recordValue": record["value"] ?? "", "provisional": reply["provisional"] ?? false, "spans": shown]
        }
    }

    func detach(_ node: NodeView) {
        if let manager = node.textArea?.textLayoutManager, let content = manager.textContentManager {
            manager.renderingAttributesValidator = nil
            if painted { manager.setRenderingAttributes([:], for: content.documentRange) }
        }
        painted = false
        paints = []
        lines = []
        state = [:]
        (node.textArea as? TextArea)?.setNeedsDisplayOfWholeField()
    }

    var drawsLines: Bool { !lines.isEmpty }

    /// The underlines and strikethroughs, in the field's own coordinates,
    /// after it draws its text.
    func drawLines(in field: TextArea) {
        guard !lines.isEmpty, let manager = field.textLayoutManager, let content = manager.textContentManager,
              let context = Self.context, let font = field.font else { return }
        #if os(macOS)
        let origin = field.textContainerOrigin, fallback = field.textColor ?? SystemColor.canvasText
        #else
        let origin = CGPoint(x: field.textContainerInset.left, y: field.textContainerInset.top), fallback = field.textColor ?? SystemColor.canvasText
        #endif
        let ct = font as CTFont, thickness = max(1, CTFontGetUnderlineThickness(ct))
        let below = -CTFontGetUnderlinePosition(ct), middle = CTFontGetXHeight(ct) / 2
        let start = content.documentRange.location
        context.saveGState(); defer { context.restoreGState() }
        for run in lines {
            guard let from = content.location(start, offsetBy: run.range.location),
                  let to = content.location(from, offsetBy: run.range.length), let range = NSTextRange(location: from, end: to) else { continue }
            context.setFillColor((run.ink ?? fallback).cgColor)
            // Visual segments, so a range across mixed directions is lined
            // where its glyphs are; each on its line's baseline.
            manager.enumerateTextSegments(in: range, type: .standard, options: [.rangeNotRequired]) { _, segment, _, _ in
                guard segment.width > 0, let fragment = manager.textLayoutFragment(for: CGPoint(x: segment.midX, y: segment.midY)) else { return true }
                let frame = fragment.layoutFragmentFrame
                guard let line = fragment.textLineFragments.first(where: { $0.typographicBounds.offsetBy(dx: frame.minX, dy: frame.minY).insetBy(dx: -1, dy: 0).contains(CGPoint(x: segment.midX, y: segment.midY)) })
                    ?? fragment.textLineFragments.first else { return true }
                let baseline = origin.y + frame.minY + line.typographicBounds.minY + line.glyphOrigin.y
                let left = origin.x + segment.minX
                if run.underline { context.fill(CGRect(x: left, y: baseline + below - thickness / 2, width: segment.width, height: thickness)) }
                if run.strike { context.fill(CGRect(x: left, y: baseline - middle - thickness / 2, width: segment.width, height: thickness)) }
                return true
            }
        }
    }

    private static var context: CGContext? {
        #if os(macOS)
        NSGraphicsContext.current?.cgContext
        #else
        UIGraphicsGetCurrentContext()
        #endif
    }

    /// The kernel's reconciliation and the declarations' rows, through the core's Rust.
    static func resolve(record: String, current: String, styles: String) -> [String: Any]? {
        guard let request = try? JSONSerialization.data(withJSONObject: ["record": record, "current": current, "styles": styles]) else { return nil }
        var count = 0
        let reply = request.withUnsafeBytes { bytes in exact_field_decorations(bytes.bindMemory(to: UInt8.self).baseAddress, bytes.count, &count) }
        guard let reply else { return nil }
        return try? JSONSerialization.jsonObject(with: Data(bytes: reply, count: count)) as? [String: Any]
    }

    private static func textRange(_ content: NSTextContentManager, _ start: Int, _ end: Int) -> NSTextRange? {
        guard start < end, let from = content.location(content.documentRange.location, offsetBy: start),
              let to = content.location(from, offsetBy: end - start) else { return nil }
        return NSTextRange(location: from, end: to)
    }

    private static func attributes(_ rows: [String: Any], dark: Bool) -> [NSAttributedString.Key: Any] {
        var out: [NSAttributedString.Key: Any] = [:]
        if let color = color(rows["text_color"], dark: dark) { out[.foregroundColor] = color }
        if let color = color(rows["background_color"], dark: dark) { out[.backgroundColor] = color }
        return out
    }

    private static func color(_ row: Any?, dark: Bool) -> PlatformColor? {
        guard let row, let channels = batch(row).channels(dark: dark) else { return nil }
        return TextEngine.color(channels)
    }

    private static func batch(_ value: Any) -> BatchValue {
        switch value {
        case let value as String: return .string(value)
        case let value as NSNumber: return .number(value.doubleValue)
        case let value as [Any]: return .array(value.map(batch))
        case let value as [String: Any]: return .object(value.mapValues(batch))
        default: return .null
        }
    }
}

extension TextArea {
    func setNeedsDisplayOfWholeField() {
        #if os(macOS)
        needsDisplay = true
        #else
        setNeedsDisplay()
        #endif
    }
}

extension NodeView {
    /// Paint, repaint or remove this textarea's decorations.
    func restyleDecorations() {
        guard let field = textArea as? TextArea else { return }
        if props["decorations"] != nil {
            if field.decorations == nil { field.decorations = FieldDecorations() }
            field.decorations?.restyle(self)
        } else if let decorations = field.decorations, !fieldComposing {
            decorations.detach(self)
            field.decorations = nil
        }
    }

    /// Whether the field holds marked text (an input method's composition).
    var fieldComposing: Bool {
        #if os(macOS)
        textArea?.hasMarkedText() ?? (field?.currentEditor() as? NSTextView)?.hasMarkedText() ?? false
        #else
        (textArea?.markedTextRange ?? selectionField?.markedTextRange) != nil
        #endif
    }
}
