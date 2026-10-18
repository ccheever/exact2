// Generic source-backed field styling. The application supplies every range;
// grammar and editing commands stay outside this capability (LLP 1114.007).
#if os(macOS)
import AppKit
#else
import UIKit
#endif
import CExact
import ExactKit

final class DecorationEditor: FieldDecorating {
    private static let weightKey = NSAttributedString.Key("ExactDecorationWeight")
    private static let familyKey = NSAttributedString.Key("ExactDecorationFamily")
    private static let italicKey = NSAttributedString.Key("ExactDecorationItalic")
    private static let numericKey = NSAttributedString.Key("ExactDecorationNumeric")
    struct Item {
        var name: String
        var range: NSRange
        var marker: String?
        var label: String?
    }
    private(set) var styling = false
    private(set) var state: [String: Any] = [:]
    private(set) var projection: FieldTextProjection?
    private(set) var spans: [Item] = [], blocks: [Item] = [], atoms: [Item] = []
    private(set) var hidden: [NSRange] = []
    private var styles: [String: NodeStyle] = [:]
    private var atomFonts: [Int: PlatformFont] = [:]
    private var plain: [NSAttributedString.Key: Any] = [:]
    var font: ((CGFloat, Int, Int, Bool, Int) -> PlatformFont)?
    private var styled = ""
    private var pendingMetrics = false
    private var warned: Set<String> = []
    private weak var node: NodeView?
    var field: TextArea? { node?.textArea as? TextArea }

    private var selection: NSRange {
        #if os(macOS)
        field?.selectedRange() ?? NSRange(location: 0, length: 0)
        #else
        field?.selectedRange ?? NSRange(location: 0, length: 0)
        #endif
    }
    private var storage: NSTextStorage? {
        #if os(macOS)
        field?.textStorage
        #else
        field?.textStorage
        #endif
    }
    private func repaint() {
        #if os(macOS)
        field?.needsDisplay = true
        #else
        field?.setNeedsDisplay()
        #endif
    }

    func detach(_ node: NodeView) {
        guard let storage else { return }
        styling = true; defer { styling = false }
        storage.setAttributes(plain, range: NSRange(location: 0, length: storage.length))
        field?.typingAttributes = plain
        state = [:]; projection = nil; hidden = []; atoms = []; blocks = []; spans = []
        repaint()
    }

    func restyle(_ node: NodeView) {
        guard !styling, !node.fieldComposing, let text = node.text,
              let record = node.props["decorations"], let table = node.props["decorationStyles"] else { return }
        self.node = node
        guard field != nil, let storage else { return }
        let look = MarkupEditor.Look(
            font: { size, weight, family, italic in text.font(size: size, weight: weight, family: family, italic: italic, numeric: Int(node.number("font_variant_numeric"))) },
            size: node.number("font_size", PageFacts.defaultRootFontSize), weight: Int(node.number("font_weight", 400)),
            family: Int(node.number("font_family")), italic: node.style["font_style"]?.string == "italic",
            lineHeight: node.usedLineHeight, ink: node.fieldInk)
        font = {size,weight,family,italic,numeric in text.font(size:size,weight:weight,family:family,italic:italic,numeric:numeric)}
        _ = style(storage,record:record,table:table,look:look,selection:selection,root:PageFacts.rootFontSize,scale:PageFacts.displayScale(node),numeric:Int(node.number("font_variant_numeric")),alignment:node.decorationAlignment)
    }

    @discardableResult func style(_ storage:NSTextStorage,record:String,table:String,look:MarkupEditor.Look,selection:NSRange,root:Double,scale:Double,numeric:Int,alignment:NSTextAlignment = .left) -> Bool {
        guard !styling else { return false }
        let oldHidden = hidden, hadState = state["record"] != nil
        styling = true; defer { styling = false }
        let request: [String: Any] = ["record": record, "current": storage.string, "styles": table,
            "start": selection.location, "end": NSMaxRange(selection), "root": root,
            "font": look.size, "scale": scale]
        guard let bytes = try? JSONSerialization.data(withJSONObject: request) else { return false }
        var pointer: UnsafePointer<UInt8>?, count = 0
        let handle = bytes.withUnsafeBytes { bytes in exact_decorations(bytes.bindMemory(to: UInt8.self).baseAddress, bytes.count, &pointer, &count) }
        defer { exact_markup_free(handle) }
        guard handle != 0, let pointer,
              let reply = try? JSONSerialization.jsonObject(with: Data(bytes: pointer, count: count)) as? [String: Any],
              let resolved = reply["record"] as? [String: Any], let rawStyles = reply["styles"] as? [String: [String: Any]] else {
            if warned.insert("invalid-record").inserted { node?.presenter?.session?.log("decorations refused: invalid record or declaration table") }
            storage.setAttributes(MarkupEditor().baseAttributes(look),range:NSRange(location:0,length:storage.length))
            state = [:]; projection = nil; hidden = []; atoms = []; blocks = []; spans = []
            return false
        }
        styles = rawStyles.mapValues { $0.mapValues(Self.batch) }
        let length = storage.length
        func items(_ key: String) -> [Item] {
            (resolved[key] as? [[String: Any]] ?? []).compactMap { value in
                guard let start = value["start"] as? Int, let end = value["end"] as? Int,
                      start >= 0, end > start, end <= length else { return nil }
                return Item(name: value["name"] as? String ?? "", range: NSRange(location: start, length: end - start),
                            marker: value["marker"] as? String, label: value["label"] as? String)
            }
        }
        spans = items("spans"); blocks = items("blocks"); atoms = items("atoms")
        let concealed = items("hidden"), revealed = reply["revealed"] as? [Bool] ?? []
        #if os(macOS)
        hidden = concealed.enumerated().filter { !revealed.indices.contains($0.offset) || !revealed[$0.offset] }.map { $0.element.range }
        #else
        // The phone caret/selection proof is a separate admission boundary.
        hidden = []
        #endif
        plain = MarkupEditor().baseAttributes(look)
        (plain[.paragraphStyle] as? NSMutableParagraphStyle)?.alignment = alignment
        plain[Self.weightKey] = look.weight; plain[Self.familyKey] = look.family
        plain[Self.italicKey] = look.italic; plain[Self.numericKey] = numeric
        atomFonts = atoms.reduce(into:[:]) { result,atom in
            let rows = styles[atom.name] ?? [:]
            let size = CGFloat(rows["font_size"]?.number ?? Double(look.size))
            let weight = rows["font_weight"]?.number.map(Int.init) ?? look.weight
            let family = rows["font_family"]?.number.map(Int.init) ?? look.family
            let italic = rows["font_style"]?.string.map { $0 == "italic" } ?? look.italic
            let numeric = rows["font_variant_numeric"]?.number.map(Int.init) ?? numeric
            result[atom.range.location] = font?(size,weight,family,italic,numeric) ?? look.font(size,weight,family,italic)
        }
        storage.beginEditing()
        storage.setAttributes(plain, range: NSRange(location: 0, length: length))
        for block in blocks { apply(block, storage: storage, look: look, paragraph: true) }
        for span in spans { apply(span, storage: storage, look: look, paragraph: false) }
        for item in concealed {
            storage.addAttribute(.foregroundColor, value: look.ink.withAlphaComponent(0.3), range: item.range)
        }
        #if os(macOS)
        // A zero horizontal font matrix keeps native source offsets and vertical
        // metrics, while giving the hidden glyphs exactly zero advance.
        let zero = PlatformFont(descriptor: (plain[.font] as! PlatformFont).fontDescriptor.withMatrix(AffineTransform(scaleByX: 0, byY: 1)), size: look.size)!
        for range in hidden { storage.addAttributes([.font: zero, .foregroundColor: PlatformColor.clear, .kern: 0], range: range) }
        for atom in atoms {
            storage.addAttributes([.font: zeroFont(atomFonts[atom.range.location] ?? plain[.font] as! PlatformFont), .foregroundColor: PlatformColor.clear, .kern: 0], range: atom.range)
            storage.addAttribute(.kern, value: atomWidth(atom, look: look), range: NSRange(location: atom.range.location, length: 1))
        }
        #else
        for atom in atoms {
            storage.addAttributes([.font: zeroFont(atomFonts[atom.range.location] ?? plain[.font] as! PlatformFont), .foregroundColor: PlatformColor.clear, .kern: 0], range: atom.range)
            storage.addAttribute(.kern, value: atomWidth(atom, look: look), range: NSRange(location: atom.range.location, length: 1))
        }
        #endif
        storage.endEditing()
        field?.typingAttributes = plain
        styled = storage.string
        projection = makeProjection(styled)
        state = ["record": resolved, "provisional": reply["provisional"] ?? false,
                 "hidden": hidden.map { ["start": $0.location, "end": NSMaxRange($0)] },
                 "source": styled]
        if node != nil, oldHidden != hidden || !hadState && !concealed.isEmpty {
            if !pendingMetrics {
                pendingMetrics = true
                DispatchQueue.main.async { [weak self] in
                    guard let self else { return }
                    pendingMetrics = false
                    node?.remeasureDecorations()
                }
            }
        }
        repaint()
        return true
    }

    private func apply(_ item: Item, storage: NSTextStorage, look: MarkupEditor.Look, paragraph: Bool) {
        guard let rows = styles[item.name] else {
            if warned.insert(item.name).inserted { node?.presenter?.session?.log("unknown decoration \(item.name)") }
            return
        }
        var runs: [([NSAttributedString.Key: Any], NSRange)] = []
        storage.enumerateAttributes(in: item.range) { attributes, range, _ in runs.append((attributes, range)) }
        for (existing, range) in runs {
        let current = existing[.font] as? PlatformFont ?? plain[.font] as! PlatformFont
        let size = CGFloat(rows["font_size"]?.number ?? Double(current.pointSize))
        let weight = rows["font_weight"]?.number.map(Int.init) ?? existing[Self.weightKey] as? Int ?? look.weight
        let family = rows["font_family"]?.number.map(Int.init) ?? existing[Self.familyKey] as? Int ?? look.family
        let italic = rows["font_style"]?.string.map { $0 == "italic" } ?? existing[Self.italicKey] as? Bool ?? look.italic
        let numeric = rows["font_variant_numeric"]?.number.map(Int.init) ?? existing[Self.numericKey] as? Int ?? 0
        var attributes: [NSAttributedString.Key: Any] = [:]
        if rows.keys.contains(where: { $0.hasPrefix("font_") }) {
            attributes[.font] = font?(size,weight,family,italic,numeric) ?? look.font(size, weight, family, italic)
            attributes[Self.weightKey] = weight; attributes[Self.familyKey] = family
            attributes[Self.italicKey] = italic; attributes[Self.numericKey] = numeric
        }
        if let color = color(rows["text_color"]) { attributes[.foregroundColor] = color }
        if !paragraph, let color = color(rows["background_color"]) { attributes[.backgroundColor] = color }
        if let decoration = rows["text_decoration_line"]?.string {
            attributes[.underlineStyle] = decoration.contains("underline") ? NSUnderlineStyle.single.rawValue : 0
            attributes[.strikethroughStyle] = decoration.contains("line-through") ? NSUnderlineStyle.single.rawValue : 0
        }
        if paragraph {
            let style = (existing[.paragraphStyle] as? NSParagraphStyle)?.mutableCopy() as? NSMutableParagraphStyle ?? NSMutableParagraphStyle()
            let marker: CGFloat = item.marker != nil && item.marker != "rule" && rows["padding_left"] == nil ? size * 1.5 : 0
            let indent = CGFloat(rows["padding_left"]?.number ?? 0) + CGFloat(rows["margin_left"]?.number ?? 0) + CGFloat(rows["border_width_left"]?.number ?? 0)
            if rows["padding_left"] != nil || rows["margin_left"] != nil || rows["border_width_left"] != nil || item.marker != nil {
                style.headIndent = indent + marker; style.firstLineHeadIndent = indent + marker
            }
            if let margin = rows["margin_right"]?.number { style.tailIndent = -CGFloat(margin) }
            if let margin = rows["margin_top"]?.number { style.paragraphSpacingBefore = CGFloat(margin) }
            if let margin = rows["margin_bottom"]?.number { style.paragraphSpacing = CGFloat(margin) }
            if let line = rows["line_height"] {
                if let n = line.number { style.minimumLineHeight = CGFloat(n) * size; style.maximumLineHeight = CGFloat(n) * size }
                else if let word = line.string, word.hasSuffix("px"), let n = Double(word.dropLast(2)) {
                    style.minimumLineHeight = CGFloat(n); style.maximumLineHeight = CGFloat(n)
                } else if line.string == "normal" { style.minimumLineHeight = 0; style.maximumLineHeight = 0 }
            }
            attributes[.paragraphStyle] = style
        }
        storage.addAttributes(attributes, range: range)
        }
    }

    private func color(_ value: BatchValue?) -> PlatformColor? {
        guard let node, let channels = value?.channels(dark: node.drawsDark) else { return nil }
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

    func selection(_ range: NSRange, previous: NSRange) -> NSRange {
        guard !styling, node?.fieldText == styled else { return range }
        let atomic = atoms.map(\.range) + hidden
        if range.length == 0, let atom = atomic.first(where: { range.location > $0.location && range.location < NSMaxRange($0) }) {
            return NSRange(location: range.location >= previous.location ? NSMaxRange(atom) : atom.location, length: 0)
        }
        return replacement(range)
    }
    func replacement(_ range: NSRange) -> NSRange {
        guard node?.fieldText == styled, range.length > 0 else { return range }
        var result = range
        for atom in atoms where NSIntersectionRange(result, atom.range).length > 0 { result = NSUnionRange(result, atom.range) }
        return result
    }

    private func makeProjection(_ source: String) -> FieldTextProjection {
        let units = Array(source.utf16)
        var output: [UInt16] = [], boundaries = [0], at = 0
        while at < units.count {
            if let hidden = hidden.first(where: { $0.location == at }) {
                at = NSMaxRange(hidden); boundaries[boundaries.count - 1] = at
            } else if let atom = atoms.first(where: { $0.range.location == at }), let label = atom.label {
                let label = Array(label.utf16)
                output.append(contentsOf: label)
                if label.isEmpty { boundaries[boundaries.count - 1] = NSMaxRange(atom.range) }
                boundaries.append(contentsOf: label.indices.map { $0 == label.count - 1 ? NSMaxRange(atom.range) : atom.range.location })
                at = NSMaxRange(atom.range)
            } else { output.append(units[at]); at += 1; boundaries.append(at) }
        }
        return FieldTextProjection(value: String(decoding: output, as: UTF16.self), boundaries: boundaries)
    }

    private func zeroFont(_ font:PlatformFont) -> PlatformFont {
        #if os(macOS)
        return PlatformFont(descriptor:font.fontDescriptor.withMatrix(AffineTransform(scaleByX:0,byY:1)),size:font.pointSize)!
        #else
        return PlatformFont(descriptor:font.fontDescriptor.withMatrix(CGAffineTransform(scaleX:0,y:1)),size:font.pointSize)
        #endif
    }

    private func atomWidth(_ atom: Item, look: MarkupEditor.Look) -> CGFloat {
        let font = atomFonts[atom.range.location] ?? look.font(look.size, look.weight, look.family, false)
        return ceil(((atom.label ?? "") as NSString).size(withAttributes: [.font: font]).width) + look.size * 0.8
    }

    private func segments(_ range: NSRange) -> [CGRect] {
        guard let field, NSMaxRange(range) <= styled.utf16.count else { return [] }
        #if os(macOS)
        guard let manager = field.textLayoutManager, let content = manager.textContentManager,
              let start = content.location(content.documentRange.location, offsetBy: range.location),
              let end = content.location(start, offsetBy: range.length), let range = NSTextRange(location: start, end: end) else { return [] }
        manager.ensureLayout(for: range)
        var rects: [CGRect] = []
        manager.enumerateTextSegments(in: range, type: .standard, options: [.rangeNotRequired]) { _, rect, _, _ in
            rects.append(rect.offsetBy(dx: field.textContainerOrigin.x, dy: field.textContainerOrigin.y)); return true
        }
        return rects
        #else
        guard let start = field.position(from: field.beginningOfDocument, offset: range.location),
              let end = field.position(from: start, offset: range.length), let range = field.textRange(from: start, to: end) else { return [] }
        return field.selectionRects(for: range).map(\.rect)
        #endif
    }

    private func markerRect(_ block: Item) -> CGRect? {
        guard let first = segments(block.range).first, let font = plain[.font] as? PlatformFont else { return nil }
        let size = font.pointSize
        let gutter = CGFloat(styles[block.name]?["padding_left"]?.number ?? Double(size * 1.5))
        return CGRect(x: first.minX - gutter, y: first.minY + (first.height - size) / 2, width: size, height: size)
    }

    func click(_ point: CGPoint) -> Bool {
        guard let node, !node.disabled, !node.inert, node.props["editable"] != "false", node.fieldText == styled else { return false }
        if let block = blocks.first(where: { ($0.marker == "checkbox:on" || $0.marker == "checkbox:off") && markerRect($0)?.insetBy(dx: -4, dy: -4).contains(point) == true }) {
            node.decorationToggle(checked: block.marker == "checkbox:on", range: block.range)
            return true
        }
        for atom in atoms {
            if segments(atom.range).first?.contains(point) == true {
                #if os(macOS)
                field?.setSelectedRange(atom.range)
                #else
                field?.selectedRange = atom.range
                #endif
                return true
            }
        }
        return false
    }

    func draw(background: Bool) {
        guard let node, let field, !styling, node.fieldText == styled, let font = plain[.font] as? PlatformFont else { return }
        #if os(macOS)
        guard let context = NSGraphicsContext.current?.cgContext else { return }
        #else
        guard let context = UIGraphicsGetCurrentContext() else { return }
        #endif
        let ink = plain[.foregroundColor] as? PlatformColor ?? SystemColor.canvasText
        context.saveGState(); defer { context.restoreGState() }
        for block in blocks {
            guard let rows = styles[block.name] else { continue }
            let fragments = segments(block.range)
            guard let first = fragments.first else { continue }
            let union = fragments.reduce(CGRect.null) { $0.union($1) }
            let indent = CGFloat(rows["padding_left"]?.number ?? 0)
            let width = CGFloat(rows["border_width_left"]?.number ?? 0)
            let marker = block.marker != nil && block.marker != "rule" && rows["padding_left"] == nil ? font.pointSize * 1.5 : 0
            let left = first.minX - indent - width - marker
            if background {
                if let fill = color(rows["background_color"]) {
                    let box = CGRect(x: left, y: union.minY, width: max(0, field.bounds.width - left), height: union.height)
                    let radius = CGFloat(rows["border_radius_top_left"]?.number ?? 0)
                    context.setFillColor(fill.cgColor)
                    context.addPath(CGPath(roundedRect: box, cornerWidth: radius, cornerHeight: radius, transform: nil)); context.fillPath()
                }
                if width > 0, rows["border_style_left"]?.string != "none" {
                    context.setFillColor((color(rows["border_color_left"]) ?? ink).cgColor)
                    context.fill(CGRect(x: left, y: union.minY, width: width, height: union.height))
                }
            } else if let marker = block.marker, let rect = markerRect(block) {
                context.setStrokeColor(ink.cgColor); context.setFillColor(ink.cgColor); context.setLineWidth(1)
                switch marker {
                case "bullet": context.fillEllipse(in: rect.insetBy(dx: rect.width * 0.35, dy: rect.height * 0.35))
                case "checkbox:on", "checkbox:off":
                    let box = rect.insetBy(dx: 1, dy: 1)
                    context.addPath(CGPath(roundedRect: box, cornerWidth: 3, cornerHeight: 3, transform: nil)); context.strokePath()
                    if marker == "checkbox:on" {
                        context.move(to: CGPoint(x: box.minX + 2, y: box.midY)); context.addLine(to: CGPoint(x: box.midX - 1, y: box.maxY - 3)); context.addLine(to: CGPoint(x: box.maxX - 2, y: box.minY + 3)); context.strokePath()
                    }
                case "rule": context.fill(CGRect(x: left, y: union.midY, width: max(0, field.bounds.width - left), height: 1))
                case "source":
                    if let prefix = hidden.first(where: { $0.location == block.range.location }) {
                        let label = (styled as NSString).substring(with: prefix).trimmingCharacters(in: .whitespacesAndNewlines)
                        (label as NSString).draw(at: rect.origin, withAttributes: [.font: font, .foregroundColor: ink])
                    }
                default: break
                }
            }
        }
        guard !background else { return }
        for atom in atoms {
            guard let first = segments(atom.range).first, let label = atom.label else { continue }
            let rows = styles[atom.name] ?? [:]
            let font = atomFonts[atom.range.location] ?? font
            let width = ceil((label as NSString).size(withAttributes:[.font:font]).width) + font.pointSize * 0.8
            let box = CGRect(x: first.minX, y: first.minY, width: width, height: max(first.height, font.pointSize * 1.3))
            context.setFillColor((color(rows["background_color"]) ?? ink.withAlphaComponent(0.08)).cgColor)
            let radius = CGFloat(rows["border_radius_top_left"]?.number ?? 5)
            context.addPath(CGPath(roundedRect: box, cornerWidth: radius, cornerHeight: radius, transform: nil)); context.fillPath()
            (label as NSString).draw(at: CGPoint(x: box.minX + font.pointSize * 0.4, y: box.minY + (box.height - font.pointSize) / 2), withAttributes: [.font: font, .foregroundColor: color(rows["text_color"]) ?? ink])
        }
    }
}
