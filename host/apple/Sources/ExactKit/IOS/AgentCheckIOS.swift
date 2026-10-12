// `check` on UIKit (LLP 1116.003 D8): what the presented screen shows, as
// observations the driver judges (scripts/agent-check.mjs) — each view's box
// in the window, a paragraph's text, ink and type and whether it fits its box
// or a clip cuts it, a control and who sized it (UIKit's own controls at
// their own size are the platform's), what a finger at its middle reaches at
// rest (a bar, the keyboard, a sheet, another node) and whether a scroller
// could move it clear, a filled box's fill and edge, and how far a view runs
// past the viewport's sides. UIKit's own labels (a native button's title, a
// segment's, a list cell's) are text too. Read from the view tree, never a
// second model; the background under a text is the nearest fill painted
// under it (LLP 1116.006 D5), else the driver's, from a capture of the
// window, taken at rest (`settleMotion`, AgentMotionIOS.swift).
#if os(iOS) || os(tvOS)
import UIKit

extension Agent {
    func check(_ req: [String: Any]) -> [String: Any] {
        _ = settleForPicture()
        settleMotion()
        let vp = presenter.viewport
        guard let win = vp.window else { return ["error": "check: the app has no window yet"] }
        let port = vp.convert(vp.bounds, to: nil).intersection(win.bounds)
        // While a sheet is up, it is what is presented; the screen under it is not.
        let scope: UIView = presenter.modals.coordinateView ?? win
        let keyboard = keyboardTopForCheck()
        let r2 = { (v: CGFloat) in Agent.r2(v) }
        // The bars drawn over the content: what lies under one, scrolled there, is covered, not presented (LLP 1116.007 D3.8).
        var bars: [(view: UIView, box: CGRect, top: Bool)] = []
        func findBars(_ v: UIView) {
            guard !v.isHidden, v.alpha > 0.01 else { return }
            if Agent.isBar(v) { let b = v.convert(v.bounds, to: nil); if b.intersects(port) { bars.append((v, b, b.midY < port.midY)) }; return }
            for s in v.subviews { findBars(s) }
        }
        findBars(scope)
        var items: [[String: Any]] = []

        func rgba(_ c: [Double]?) -> [Double]? {
            guard let c, c.count >= 4, c[3] > 0 else { return nil }
            return [c[0].rounded(), c[1].rounded(), c[2].rounded(), Double(r2(CGFloat(c[3] / 255)))]
        }
        func rgba(_ color: UIColor, traits: UITraitCollection) -> [Double]? {
            var r: CGFloat = 0, g: CGFloat = 0, b: CGFloat = 0, a: CGFloat = 0
            guard color.resolvedColor(with: traits).getRed(&r, green: &g, blue: &b, alpha: &a), a > 0 else { return nil }
            return [Double(min(max(r, 0), 1) * 255).rounded(), Double(min(max(g, 0), 1) * 255).rounded(), Double(min(max(b, 0), 1) * 255).rounded(), Double(r2(a))]
        }
        func rect(_ b: CGRect) -> [String: Any] { ["x": r2(b.minX), "y": r2(b.minY), "w": r2(b.width), "h": r2(b.height)] }
        /// A node above `v` that sticks or stays put (`position: sticky|fixed`), below `upTo`.
        func pinned(_ v: UIView, upTo: UIView) -> Bool {
            var at: UIView? = v
            while let cur = at, cur !== upTo {
                if let n = cur as? NodeView, ["sticky", "fixed"].contains(n.style["position"]?.string ?? "") { return true }
                at = cur.superview
            }
            return false
        }
        /// Whether `hit` moves with `sv`'s content: inside it, in a node or a cell, and not stuck in place. A
        /// scroller's own views (its edge effect, its indicators) stay where the scroller is.
        func movesWith(_ hit: UIView, _ sv: UIScrollView) -> Bool {
            guard hit.isDescendant(of: sv), !pinned(hit, upTo: sv) else { return false }
            var at: UIView? = hit
            while let cur = at, cur !== sv {
                if cur is NodeView || cur is UICollectionReusableView { return true }
                at = cur.superview
            }
            return false
        }
        /// Whether a scroller holding `v` could move it `by` points away from the side `above` names: one `hit`
        /// does not move with, with that much room to scroll.
        func reachable(_ v: UIView, hit: UIView?, above: Bool, by: CGFloat = 0) -> Bool {
            var at = v.superview
            while let cur = at {
                if let sv = cur as? UIScrollView, sv.isScrollEnabled, !sv.isHidden, !(hit.map { movesWith($0, sv) } ?? false) {
                    let i = sv.adjustedContentInset, minY = -i.top, maxY = max(minY, sv.contentSize.height + i.bottom - sv.bounds.height)
                    let room = max(0.5, by)
                    if above ? sv.contentOffset.y > minY + room : sv.contentOffset.y < maxY - room { return true }
                }
                at = cur.superview
            }
            return false
        }
        /// The part of the screen `v` can show in: each scroller holding it cuts its content at its edges.
        func shown(_ v: UIView) -> CGRect {
            var r = port, at = v.superview
            while let cur = at {
                if let sv = cur as? UIScrollView, sv.clipsToBounds { r = r.intersection(sv.convert(sv.bounds, to: nil)) }
                at = cur.superview
            }
            return r
        }
        /// The part of box `b` (`v`'s, in the window) that is presented: each clipping view above it (a scroller, a list,
        /// a box that hides its overflow) cuts it at its edges (LLP 1116.006 D5), and a bar over the screen's top or
        /// bottom cuts what lies past its inner edge (LLP 1116.007 D3.8).
        func visible(_ v: UIView, _ b: CGRect) -> CGRect {
            var r = b, at = v.superview
            while let cur = at, !r.isNull {
                if cur.clipsToBounds || cur.layer.masksToBounds { r = r.intersection(cur.convert(cur.bounds, to: nil)) }
                at = cur.superview
            }
            for bar in bars where !r.isNull && !v.isDescendant(of: bar.view) {
                let cut = bar.top ? bar.box.maxY - r.minY : r.maxY - bar.box.minY
                guard cut > 0 else { continue }
                if bar.top { r.origin.y += min(cut, r.height) }
                r.size.height = max(0, r.height - cut)
            }
            return r
        }
        /// `v`'s alpha times its ancestors'.
        func opacity(_ v: UIView) -> Double {
            var o = 1.0, at: UIView? = v
            while let cur = at { o *= Double(cur.alpha) * Double(cur.layer.opacity); at = cur.superview }
            return o
        }
        /// Whether `s` or a view in it paints at `p` something a style does not say for certain: a fill, a picture, a
        /// material, a native control's own background.
        func paints(_ s: UIView, at p: CGPoint) -> Bool {
            guard !s.isHidden, s.alpha > 0.01, s.convert(s.bounds, to: nil).contains(p) else { return false }
            if let n = s as? NodeView {
                if n.kind == "image" || n.symbolView != nil || n.style["background_image"] != nil || (n.channels("background_color")?[3] ?? 0) > 0 { return true }
            } else if s.layer.contents != nil || s is UIVisualEffectView || s is UIImageView || (s.backgroundColor.flatMap { rgba($0, traits: s.traitCollection) } != nil) {
                return true
            }
            return s.subviews.contains { paints($0, at: p) }
        }
        /// What a text (`v`, box `b` in the window) stands on (LLP 1116.006 D5): the nearest fill painted under its
        /// middle — its own box's, a bubble's, a card's, the route's or the canvas's — composited over what lies below
        /// where it is translucent, and whose it is (nil: the page's). Nil where something the tree does not say for
        /// certain paints there — a picture, a material, a native control's or a list cell's own background, a view laid
        /// between that is not its ancestor — or nothing opaque is reached: the capture's pixels answer then.
        func backing(_ v: UIView, _ b: CGRect) -> (bg: [Double], by: Int?)? {
            let p = CGPoint(x: b.midX, y: b.midY)
            var layers: [[Double]] = [], by: Int?, first = true, child: UIView?, at: UIView? = v
            while let cur = at {
                // A view earlier among its parent's children paints under the one the walk came from.
                if let child, let i = cur.subviews.firstIndex(of: child), cur.subviews[..<i].contains(where: { paints($0, at: p) }) { return nil }
                child = cur
                at = cur.superview
                var fill: [Double]?
                if let n = cur as? NodeView {
                    if n.kind == "image" || n.style["background_image"] != nil || n.vibrancyView != nil { return nil }
                    if cur !== v, !n.convert(n.bounds, to: nil).contains(p) { continue }
                    if n.backgroundClip != "text", let c = n.channels("background_color"), c.count >= 4, c[3] > 0 { fill = [c[0], c[1], c[2], c[3] / 255] }
                } else {
                    #if os(iOS)
                    if cur is UIToolbar { return nil }
                    #endif
                    if cur is UIControl || cur is UIVisualEffectView || cur is UICollectionReusableView || cur is UITableViewCell
                        || cur is UIImageView || cur is UINavigationBar || cur is UITabBar { return nil }
                    fill = cur.backgroundColor.flatMap { rgba($0, traits: cur.traitCollection) }
                }
                guard var c = fill else { continue }
                c[3] *= opacity(cur)
                if first { by = (cur as? NodeView).map { Int($0.id) }; first = false }
                layers.append(c)
                if c[3] >= 0.999 {
                    var bg = Array(layers.removeLast().prefix(3))
                    while let top = layers.popLast() { bg = (0..<3).map { top[$0] * top[3] + bg[$0] * (1 - top[3]) } }
                    return (bg.map { $0.rounded() }, by)
                }
            }
            return nil
        }
        /// What lies over the middle of `v` (box `b` in the window) at rest, if anything else does.
        func cover(_ v: UIView, _ b: CGRect) -> [String: Any]? {
            // Past the screen's top or bottom, or its scroller's, where nothing scrolls it in: a column taller than
            // the screen, a row past a scroller's end. Whatever is over that point covers nothing the scroller cut
            // away: a row past a tab's scroller, under the tab bar the agent paints there, was one (LLP 1116.003).
            let seen = shown(v)
            if seen.isNull || b.midY >= seen.maxY || b.midY < seen.minY {
                let up = seen.isNull ? b.midY < port.midY : b.midY < seen.minY
                let screen = seen.isNull || b.midY >= port.maxY || b.midY < port.minY
                return reachable(v, hit: nil, above: up) ? nil : ["kind": "offscreen", "name": "the \(up ? "top" : "bottom") of \(screen ? "the screen" : "its scroller")"]
            }
            let p = CGPoint(x: min(max(b.midX, port.minX + 1), port.maxX - 1), y: min(max(b.midY, port.minY + 1), port.maxY - 1))
            if let top = keyboard, p.y >= top {
                return reachable(v, hit: nil, above: false) ? nil : ["kind": "keyboard", "name": "the software keyboard"]
            }
            guard let hit = win.hitTest(p, with: nil), hit !== v, !hit.isDescendant(of: v), !v.isDescendant(of: hit) else { return nil }
            var owner: UIView? = hit, bar: UIView? = nil
            while let cur = owner, !(cur is NodeView) {
                if Agent.isBar(cur) { bar = cur }
                owner = cur.superview
            }
            // The cover whole — a bar, the node the hit is part of, or a scroller's own view (its edge effect) —
            // is over the shown part's top half or its bottom half, and a scroller clears it by carrying the middle
            // past its far edge. The hit alone misjudged a navigation bar, whose title lies below the middle of a
            // row under the bar, as covering the row from below.
            let own = (owner as? NodeView)?.scroll.map { hit.isDescendant(of: $0) } ?? false
            let whole: UIView = bar ?? (own ? hit : owner ?? hit)
            let cb = whole.convert(whole.bounds, to: nil), above = cb.midY < seen.midY
            if reachable(v, hit: hit, above: above, by: above ? cb.maxY - p.y : p.y - cb.minY) { return nil }
            let sheet = presenter.modals.coordinateView.map { hit.isDescendant(of: $0) && !v.isDescendant(of: $0) } ?? false
            let barName = bar.map { String(describing: Swift.type(of: $0)) }
            var c: [String: Any] = ["kind": barName != nil ? "bar" : sheet ? "sheet" : "node", "name": barName ?? String(describing: Swift.type(of: hit))]
            if bar == nil, let n = owner as? NodeView { c["by"] = Int(n.id) }
            return c
        }

        // Preorder over what is shown, so a view past the viewport's side is named and not each of its children.
        func walk(_ v: UIView, alpha: CGFloat, held: Bool) {
            guard !v.isHidden, v.alpha > 0.01, v.layer.opacity > 0.01 else { return }
            let a = alpha * v.alpha * CGFloat(v.layer.opacity)
            var heldBelow = held
            if let n = v as? NodeView {
                guard !n.inert, !n.cssVisibilityHidden, !presenter.navigation.isInactiveRoute(containing: n) else { return }
                let b = n.convert(n.bounds, to: nil)
                if b.width > 0, b.height > 0, b.intersects(port) || b.intersects(win.bounds) {
                    var item: [String: Any] = rect(b).merging(["id": Int(n.id)]) { x, _ in x }
                    let past = max(b.maxX - port.maxX, port.minX - b.minX)
                    if !held, past > 1, !pinned(n, upTo: scope) { item["outX"] = r2(past); heldBelow = true }
                    if n.isParagraph { paragraph(n, b, alpha: a, into: &item) }
                    control(n, into: &item)
                    if item["control"] != nil || item["text"] != nil, let c = cover(n, b), item["control"] != nil || c["kind"] as? String != "offscreen" { item["cover"] = c }
                    if item["text"] == nil, item["control"] == nil { fill(n, b, alpha: a, into: &item) } else { presented(n, b, into: &item) }
                    if item.keys.contains(where: { ["text", "control", "fill", "outX"].contains($0) }) { items.append(item) }
                } else if b.width > 0, b.height > 0, b.maxX > port.minX, b.minX < port.maxX {
                    // Off the screen: only a control out of reach (past its top or bottom, and nothing scrolls to it) is anything.
                    var item: [String: Any] = rect(b).merging(["id": Int(n.id)]) { x, _ in x }
                    control(n, into: &item)
                    if item["control"] != nil, let c = cover(n, b), c["kind"] as? String == "offscreen" { item["cover"] = c; items.append(item) }
                }
                // What scrolls sideways or clips keeps its children's overflow its own.
                if n.scroll?.scrollsX == true || n.clipsToBounds || n.clipBox != nil { heldBelow = true }
            } else if let label = v as? UILabel {
                nativeLabel(label, alpha: a, into: &items)
                return
            } else if v is UITextField || v is UITextView { return }
            for s in v.subviews { walk(s, alpha: a, held: heldBelow) }
        }

        func paragraph(_ n: NodeView, _ b: CGRect, alpha: CGFloat, into item: inout [String: Any]) {
            let spec = n.paragraphSpec()
            let text = spec.runs.filter { !$0.hidden }.map(\.text).joined().split(whereSeparator: \.isWhitespace).joined(separator: " ")
            guard !text.isEmpty else { return }
            item["text"] = text.count > 40 ? String(text.prefix(39)) + "…" : text
            if var fg = rgba(spec.color) { fg[3] = Double(r2(CGFloat(fg[3]) * alpha)); item["fg"] = fg }
            if let under = backing(n, b) { item["bg"] = under.bg; item["on"] = under.by == Int(n.id) ? "own" : under.by.map { $0 as Any } ?? "page" }
            item["size"] = Double(n.number("font_size", PageFacts.defaultRootFontSize)); item["weight"] = Double(n.number("font_weight", 400))
            if let role = n.systemColor("text_color") { item["fgRole"] = role }
            if spec.lineClamp > 0 || spec.ellipsis { item["clamp"] = true }
            // A control's own label on the control's own fill (D3's ink rule): the nearest pressable node within three.
            var up = n.superview, k = 0
            while let cur = up, k < 3 {
                if let c = cur as? NodeView {
                    k += 1
                    if c.takesPress || c.actsAsButton { item["of"] = Int(c.id); if rgba(c.channels("background_color")) != nil { item["label"] = true }; break }
                }
                up = cur.superview
            }
            // Its text against its own box, and the first ancestor that clips part of it away (never one that scrolls).
            if let p = n.paragraphLayout() {
                let content = n.contentBox()
                let wide = p.lines.map { CGFloat(CTLineGetTypographicBounds($0, nil, nil, nil) - CTLineGetTrailingWhitespaceWidth($0)) }.max() ?? 0
                let dx = wide - content.width, dy = p.height - content.height
                if dx > 1 || dy > 1 { item["over"] = [Double(r2(max(0, dx))), Double(r2(max(0, dy)))]; if n.clipsToBounds || n.clipBox != nil { item["clips"] = true } }
            }
            var above = n.superview
            while let cur = above {
                if cur is UIScrollView { break }
                if let c = cur as? NodeView, c.scroll == nil, c.clipsToBounds || c.clipBox != nil {
                    let clip = c.convert(c.bounds, to: nil)
                    if !clip.intersects(b) { break }
                    let dx = max(0, clip.minX - b.minX) + max(0, b.maxX - clip.maxX), dy = max(0, clip.minY - b.minY) + max(0, b.maxY - clip.maxY)
                    if dx >= 2 || dy >= 2 { item["cut"] = ["by": Int(c.id), "dx": Double(r2(dx)), "dy": Double(r2(dy))]; break }
                }
                above = cur.superview
            }
            // The screen's own end cutting it where nothing scrolls it in.
            if item["cut"] == nil, b.intersects(port) {
                if b.maxY - port.maxY >= 2, !reachable(n, hit: nil, above: false) { item["cut"] = ["edge": "bottom", "dx": 0.0, "dy": Double(r2(b.maxY - port.maxY))] }
                else if port.minY - b.minY >= 2, !reachable(n, hit: nil, above: true) { item["cut"] = ["edge": "top", "dx": 0.0, "dy": Double(r2(port.minY - b.minY))] }
            }
        }

        func control(_ n: NodeView, into item: inout [String: Any]) {
            let native = n.subviews.first { $0 is UIControl && !($0 is UITextField) }
            guard n.takesPress || n.actsAsButton || n.field != nil || n.textArea != nil || native != nil else { return }
            item["control"] = n.field != nil || n.textArea != nil ? "field" : native.map { String(describing: Swift.type(of: $0)) } ?? (n.kind == "button" ? "button" : "press")
            // UIKit's control at its own size, or a native field, is the platform's; one squeezed under its own size is the author's.
            var platform = n.isNativeTextControl || (native != nil && !(native is NativeButtonIOS))
            if let button = native as? NativeButtonIOS {
                let natural = button.naturalSize
                platform = button.bounds.width >= natural.width - 1 && button.bounds.height >= natural.height - 1
            }
            item["sized"] = platform ? "platform" : "author"
            if n.disabled { item["disabled"] = true }
        }

        /// What is presented of a text's or a control's box, where a clipping view above cuts it (the driver judges overlap on it).
        func presented(_ v: UIView, _ b: CGRect, into item: inout [String: Any]) {
            let s = visible(v, b)
            let vis = s.isNull || s.width <= 0 || s.height <= 0 ? CGRect(x: b.minX, y: b.minY, width: 0, height: 0) : s
            if abs(vis.minX - b.minX) > 0.5 || abs(vis.minY - b.minY) > 0.5 || abs(vis.width - b.width) > 0.5 || abs(vis.height - b.height) > 0.5 { item["vis"] = rect(vis) }
        }

        func fill(_ n: NodeView, _ b: CGRect, alpha: CGFloat, into item: inout [String: Any]) {
            guard n.kind != "image", n.symbolView == nil, n.style["background_image"] == nil, b.width >= 16, b.height >= 16,
                  var fill = rgba(n.channels("background_color")) else { return }
            let radius = n.cornerRadii(in: n.bounds).max() ?? 0
            guard radius > 0 || (b.minX >= port.minX + 2 && b.maxX <= port.maxX - 2) else { return }
            fill[3] = Double(r2(CGFloat(fill[3]) * alpha))
            item["fill"] = fill; item["radius"] = Double(r2(radius))
            if let role = n.systemColor("background_color") { item["fillRole"] = role }
            let sides = ["top", "right", "bottom", "left"]
            let border = sides.contains { n.number("border_width_" + $0, n.number("border_width")) > 0 && rgba(n.channels("border_color_" + $0)) != nil }
            if border || !n.boxShadows.isEmpty { item["edge"] = true }
        }

        /// UIKit's own text: a native button's title, a segment's, a list cell's (a bar's is its own). Named by the node it stands for.
        func nativeLabel(_ label: UILabel, alpha: CGFloat, into items: inout [[String: Any]]) {
            guard let raw = label.text ?? label.attributedText?.string, !raw.isEmpty, label.window != nil else { return }
            let b = label.convert(label.bounds, to: nil)
            guard b.width > 0, b.height > 0, b.intersects(port) else { return }
            let text = raw.split(whereSeparator: \.isWhitespace).joined(separator: " ")
            var item: [String: Any] = rect(b).merging(["text": text.count > 40 ? String(text.prefix(39)) + "…" : text, "clamp": true, "name": "UILabel"]) { x, _ in x }
            if var fg = rgba(label.textColor ?? .label, traits: label.traitCollection) { fg[3] = Double(r2(CGFloat(fg[3]) * alpha)); item["fg"] = fg }
            if let under = backing(label, b) { item["bg"] = under.bg; item["on"] = under.by.map { $0 as Any } ?? "page" }
            presented(label, b, into: &item)
            item["size"] = Double(label.font.pointSize)
            let weight = (label.font.fontDescriptor.object(forKey: .traits) as? [UIFontDescriptor.TraitKey: Any])?[.weight] as? CGFloat ?? 0
            item["weight"] = weight >= 0.4 ? 700.0 : weight >= 0.3 ? 600.0 : weight >= 0.23 ? 500.0 : 400.0
            var up: UIView? = label.superview
            while let cur = up {
                if let button = cur as? NativeButtonIOS { if let o = button.owner { item["of"] = Int(o.id) }; item["label"] = true; break }
                if cur is UISegmentedControl { item["label"] = true }
                if Agent.isBar(cur) { return } // UIKit's bars draw their own ink, glass and vibrancy: the platform's chrome, not judged
                if let row = GroupedListsLink.part?(cur)?["row"] as? Int { item["of"] = row; break }
                if let n = cur as? NodeView { item["of"] = Int(n.id); break }
                up = cur.superview
            }
            items.append(item)
        }

        walk(scope, alpha: 1, held: false)
        let dark = vp.traitCollection.userInterfaceStyle == .dark
        var reply: [String: Any] = ["viewport": rect(port), "capture": ["window": true, "w": r2(win.bounds.width), "h": r2(win.bounds.height)],
                                    "scheme": dark ? "dark" : "light", "target": 44, "items": items]
        if let keyboard { reply["keyboard"] = r2(keyboard) }
        return reply
    }

    /// UIKit's bars: what a finger at the top or the bottom of a screen meets.
    static func isBar(_ v: UIView) -> Bool {
        #if os(iOS)
        if v is UIToolbar { return true }
        #endif
        return v is UINavigationBar || v is UITabBar
    }

    /// The software keyboard's top in the window, or nil while it is down.
    func keyboardTopForCheck() -> CGFloat? {
        if let top = presenter.keyboardTop { return top }
        guard let container = presenter.keyboardContainer, let top = presenter.keyboardGuideTop(in: container) else { return nil }
        return container.convert(CGPoint(x: 0, y: top), to: nil).y
    }
}
#endif
