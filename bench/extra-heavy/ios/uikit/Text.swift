// Text-heavy kinds: markdown, code, thread (show more + comment box), and the web view.
import UIKit
import WebKit

// MARK: 8 markdown — the SwiftUI app's block split; inline syntax by Foundation's Markdown parser

enum MDBlock {
    case heading(NSAttributedString), para(NSAttributedString), bullets([NSAttributedString]), quote(NSAttributedString)
}

func inlineMarkdown(_ s: String, base: UIFont, color: UIColor) -> NSAttributedString {
    let a = (try? AttributedString(markdown: s, options: .init(interpretedSyntax: .inlineOnlyPreservingWhitespace)))
        ?? AttributedString(s)
    let out = NSMutableAttributedString()
    for run in a.runs {
        let text = String(a[run.range].characters)
        var font = base
        var attrs: [NSAttributedString.Key: Any] = [.foregroundColor: color]
        if let intent = run.inlinePresentationIntent {
            var traits = base.fontDescriptor.symbolicTraits
            if intent.contains(.stronglyEmphasized) { traits.insert(.traitBold) }
            if intent.contains(.emphasized) { traits.insert(.traitItalic) }
            if intent.contains(.stronglyEmphasized) && !intent.contains(.emphasized) && !traits.contains(.traitItalic) {
                font = .systemFont(ofSize: base.pointSize, weight: .bold)
            } else if let d = base.fontDescriptor.withSymbolicTraits(traits) {
                font = UIFont(descriptor: d, size: base.pointSize)
            }
            if intent.contains(.code) {
                font = spaceMono(14)
                attrs[.backgroundColor] = UIColor.xfill
            }
        }
        if run.link != nil { attrs[.foregroundColor] = UIColor.xblue }
        attrs[.font] = font
        out.append(NSAttributedString(string: text, attributes: attrs))
    }
    return out
}

/// Parsed + attributed blocks per row id (parsing is per row, not per frame; built once and reused).
let mdCache = NSCache<NSString, MDBox>()
final class MDBox { let blocks: [MDBlock]; init(_ b: [MDBlock]) { blocks = b } }

func mdBlocks(_ r: Row) -> [MDBlock] {
    if let hit = mdCache.object(forKey: r.id as NSString) { return hit.blocks }
    let md = r.md ?? ""
    let blocks: [MDBlock] = md.components(separatedBy: "\n\n").map { block in
        if block.hasPrefix("## ") {
            return .heading(inlineMarkdown(String(block.dropFirst(3)), base: sys(20, .semibold), color: .black))
        } else if block.hasPrefix("- ") {
            return .bullets(block.components(separatedBy: "\n").map {
                inlineMarkdown(String($0.dropFirst(2)), base: sys(16), color: .black)
            })
        } else if block.hasPrefix("> ") {
            return .quote(inlineMarkdown(String(block.dropFirst(2)), base: italic(sys(16)), color: .label2))
        }
        return .para(inlineMarkdown(block, base: sys(16), color: .black))
    }
    mdCache.setObject(MDBox(blocks), forKey: r.id as NSString)
    return blocks
}

final class MarkdownCell: FeedCell {
    private var labels: [UILabel] = []
    private var dots: [UILabel] = []
    private var bars: [UIView] = []
    private var used = (labels: 0, dots: 0, bars: 0)

    private func nextLabel() -> UILabel {
        if used.labels == labels.count { let l = label(sys(16), lines: 0); labels.append(l); contentView.addSubview(l) }
        used.labels += 1
        let l = labels[used.labels - 1]
        l.isHidden = false
        return l
    }
    private func nextDot() -> UILabel {
        if used.dots == dots.count { let l = label(sys(16)); l.text = "•"; dots.append(l); contentView.addSubview(l) }
        used.dots += 1
        dots[used.dots - 1].isHidden = false
        return dots[used.dots - 1]
    }
    private func nextBar() -> UIView {
        if used.bars == bars.count { let v = UIView(); v.backgroundColor = .gray3; bars.append(v); contentView.addSubview(v) }
        used.bars += 1
        bars[used.bars - 1].isHidden = false
        return bars[used.bars - 1]
    }

    override func configureBody(_ r: Row) {}

    override func layoutBody(x: CGFloat, y: CGFloat, C: CGFloat) -> CGFloat {
        used = (0, 0, 0)
        var yy = y
        for b in mdBlocks(row) {
            switch b {
            case .heading(let s), .para(let s):
                let l = nextLabel()
                l.attributedText = s
                let h = fit(l, C)
                l.frame = CGRect(x: x, y: yy, width: C, height: h)
                yy += h
            case .quote(let s):
                let l = nextLabel()
                l.attributedText = s
                let h = fit(l, C - 10)
                l.frame = CGRect(x: x + 10, y: yy, width: C - 10, height: h)
                nextBar().frame = CGRect(x: x, y: yy, width: 3, height: h)
                yy += h
            case .bullets(let items):
                for (i, s) in items.enumerated() {
                    if i > 0 { yy += 4 }
                    let d = nextDot()
                    let dw = ceil(d.sizeThatFits(.zero).width)
                    let l = nextLabel()
                    l.attributedText = s
                    let h = fit(l, C - dw - 8)
                    d.frame = CGRect(x: x, y: yy, width: dw, height: ceil(d.font.lineHeight))
                    l.frame = CGRect(x: x + dw + 8, y: yy, width: C - dw - 8, height: h)
                    yy += h
                }
            }
            yy += 8
        }
        for l in labels[used.labels...] { l.isHidden = true }
        for l in dots[used.dots...] { l.isHidden = true }
        for v in bars[used.bars...] { v.isHidden = true }
        return max(0, yy - 8 - y)
    }
}

// MARK: 9 code

let tokenColors: [String: UIColor] = [
    "keyword": UIColor(hex: 0xFF7B72), "string": UIColor(hex: 0xA5D6FF), "number": UIColor(hex: 0x79C0FF),
    "comment": UIColor(hex: 0x8B949E), "function": UIColor(hex: 0xD2A8FF), "type": UIColor(hex: 0xFFA657),
    "plain": UIColor(hex: 0xE6EDF3),
]

final class CodeCell: FeedCell {
    let card = UIView()
    let file = label(spaceMono(12), UIColor(hex: 0x8B949E))
    let lang = label(sys(11, .semibold), UIColor(hex: 0x8B949E))
    var nums: [UILabel] = []
    var codes: [UILabel] = []
    override func setup() {
        card.backgroundColor = UIColor(hex: 0x0D1117)
        rounded(card, 12)
        card.addSubview(file)
        card.addSubview(lang)
        contentView.addSubview(card)
    }
    override func configureBody(_ r: Row) {
        file.text = r.file
        lang.text = ["ts": "TS", "rs": "RUST", "py": "PYTHON"][r.lang ?? ""] ?? ""
        let lines = r.lines!
        while nums.count < lines.count {
            let n = label(spaceMono(13), UIColor(hex: 0x6E7681))
            n.textAlignment = .right
            let c = label(spaceMono(13))
            c.lineBreakMode = .byClipping
            nums.append(n); codes.append(c)
            card.addSubview(n); card.addSubview(c)
        }
        let mono = spaceMono(13)
        for i in 0..<nums.count {
            guard i < lines.count else { nums[i].isHidden = true; codes[i].isHidden = true; continue }
            nums[i].isHidden = false; codes[i].isHidden = false
            nums[i].text = "\(i + 1)"
            let a = NSMutableAttributedString()
            for t in lines[i] {
                a.append(NSAttributedString(string: t.t, attributes: [.font: mono, .foregroundColor: tokenColors[t.k] ?? tokenColors["plain"]!]))
            }
            codes[i].attributedText = a
        }
    }
    override func layoutBody(x: CGFloat, y: CGFloat, C: CGFloat) -> CGFloat {
        let n = row.lines!.count
        let hh = ceil(file.font.lineHeight)
        let lw = ceil(lang.sizeThatFits(.zero).width)
        lang.frame = CGRect(x: C - 12 - lw, y: 12 + (hh - ceil(lang.font.lineHeight)) / 2, width: lw, height: ceil(lang.font.lineHeight))
        file.frame = CGRect(x: 12, y: 12, width: C - 24 - lw - 8, height: hh)
        let y0 = 12 + hh + 8
        for i in 0..<n {
            nums[i].frame = CGRect(x: 12, y: y0 + CGFloat(i) * 20, width: 24, height: 20)
            // No wrapping: the line is clipped (not truncated) at the card's 12 pt right padding.
            let w = min(ceil(codes[i].sizeThatFits(.zero).width), C - 12 - 48)
            codes[i].frame = CGRect(x: 12 + 24 + 12, y: y0 + CGFloat(i) * 20, width: w, height: 20)
        }
        let h = y0 + CGFloat(n) * 20 + 12
        card.frame = CGRect(x: x, y: y, width: C, height: h)
        return h
    }
}

// MARK: 16 thread (show more / less, animated height; comment box with a per-row draft)

final class ThreadCell: FeedCell, UITextFieldDelegate {
    let text = label(sys(15), lines: 3)
    let more = UIButton(type: .system)
    let field = UITextField()
    let send = label(sys(15, .semibold), .gray3)
    weak var cv: UICollectionView?

    override func setup() {
        text.contentMode = .top
        more.titleLabel?.font = sys(15, .semibold)
        more.setTitleColor(.xblue, for: .normal)
        more.contentHorizontalAlignment = .left
        more.addTarget(self, action: #selector(toggle), for: .touchUpInside)
        field.font = sys(15)
        field.backgroundColor = .xfill
        rounded(field, 20)
        field.leftView = UIView(frame: CGRect(x: 0, y: 0, width: 14, height: 40))
        field.leftViewMode = .always
        field.rightView = UIView(frame: CGRect(x: 0, y: 0, width: 14, height: 40))
        field.rightViewMode = .always
        field.attributedPlaceholder = NSAttributedString(string: "Add a comment…", attributes: [.foregroundColor: UIColor.secondaryGray, .font: sys(15)])
        field.addTarget(self, action: #selector(edited), for: .editingChanged)
        send.text = "Send"
        clipsToBounds = true
        for v in [text, more, field, send] as [UIView] { contentView.addSubview(v) }
    }

    private var expanded: Bool { AppState.expanded[row.id] ?? false }
    private func flip(_ s: Int) -> Bool { (s / 3 + row.index) % 2 == 1 }

    override func configureBody(_ r: Row) {
        if liveMode { AppState.expanded[r.id] = (AppState.seconds / 3 + r.index) % 2 == 1 }
        text.text = r.text
        applyExpanded()
        field.text = AppState.drafts[r.id]
        send.textColor = (AppState.drafts[r.id] ?? "").isEmpty ? .gray3 : .xblue
    }
    private func applyExpanded() {
        text.numberOfLines = expanded ? 0 : 3
        UIView.performWithoutAnimation { more.setTitle(expanded ? "Show less" : "Show more", for: .normal); more.layoutIfNeeded() }
    }
    @objc private func edited() {
        AppState.drafts[row.id] = field.text ?? ""
        send.textColor = (field.text ?? "").isEmpty ? .gray3 : .xblue
    }
    @objc private func toggle() { setExpanded(!expanded) }

    private func setExpanded(_ e: Bool) {
        guard e != expanded else { return }
        AppState.expanded[row.id] = e
        applyExpanded()
        relayout()
        let cv = sequence(first: superview, next: { $0?.superview }).compactMap { $0 as? UICollectionView }.first
        UIView.animate(withDuration: 0.25, delay: 0, options: [.curveEaseInOut, .beginFromCurrentState]) {
            self.invalidateIntrinsicContentSize()
            cv?.collectionViewLayout.invalidateLayout()
            cv?.layoutIfNeeded()
        }
    }

    override func tickBody(in cv: UICollectionView) {
        if liveMode { setExpanded(flip(AppState.seconds)) }
    }

    override func layoutBody(x: CGFloat, y: CGFloat, C: CGFloat) -> CGFloat {
        let th = fit(text, C)
        text.frame = CGRect(x: x, y: y, width: C, height: th)
        let mh = ceil(sys(15, .semibold).lineHeight)
        more.frame = CGRect(x: x, y: y + th + 4, width: 120, height: mh)
        let fy = y + th + 4 + mh + 12
        let sw = ceil(send.sizeThatFits(.zero).width)
        field.frame = CGRect(x: x, y: fy, width: C - 8 - sw, height: 40)
        send.frame = CGRect(x: x + C - sw, y: fy + (40 - ceil(send.font.lineHeight)) / 2, width: sw, height: ceil(send.font.lineHeight))
        return fy + 40 - y
    }
}

// MARK: 17 webview (WKWebView, the page as an HTML string, no base URL, scrolling off)

final class WebCell: FeedCell {
    let caption = label(sys(15), lines: 0)
    let box = UIView()
    let web: WKWebView = {
        let v = WKWebView(frame: .zero, configuration: WKWebViewConfiguration())
        v.scrollView.isScrollEnabled = false
        v.isOpaque = false
        v.backgroundColor = .clear
        return v
    }()
    private var html = ""
    override func setup() {
        rounded(box, 12)
        box.layer.borderWidth = 0.5
        box.layer.borderColor = UIColor.border.cgColor
        box.addSubview(web)
        contentView.addSubview(caption)
        contentView.addSubview(box)
    }
    override func configureBody(_ r: Row) {
        caption.text = r.caption
        let h = embedHTML(r)
        if h != html { html = h; web.loadHTMLString(h, baseURL: nil) }
    }
    override func layoutBody(x: CGFloat, y: CGFloat, C: CGFloat) -> CGFloat {
        let y1 = layoutCaption(caption, x: x, y: y, C: C)
        box.frame = CGRect(x: x, y: y1, width: C, height: 220)
        web.frame = box.bounds
        return y1 + 220 - y
    }
}
