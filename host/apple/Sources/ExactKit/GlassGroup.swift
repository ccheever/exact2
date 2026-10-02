// Liquid Glass groups (LLP 1053.000.000): `glassGroup` makes a node's
// mounted subtree one platform glass container (`UIGlassContainerEffect`,
// `NSGlassEffectContainerView`) whose glass merges `spacing` points apart.
//
// The group view is the innermost of a node's own views: the first subview
// of what `container` named before it (`baseContainer`), holding the node's
// children (D2). A material, a scroll or a canvas on the node wins over it
// (D6). Inside a group the platform ignores the opacity, masks and clipping
// of every view between the group and a glass, so a glass whose path is not
// clear is isolated in a container of its own, merging with nothing (D4):
// on iOS a slot around its material whose effect switches between nil and
// a container in place, on macOS a container it is moved into and out of.
// Paint order inside a group is the platform's (D5).
#if os(iOS)
import UIKit
typealias GlassPlatformView = UIView
#else
import AppKit
typealias GlassPlatformView = NSView
#endif

/// Every group and glass a presenter shows, weakly (a leaving view stays
/// while it draws), and the pass that isolates or joins each glass at the
/// end of a batch (D4).
final class GlassGroups {
    let groups = NSHashTable<NodeView>.weakObjects()
    let glass = NSHashTable<NodeView>.weakObjects()
    private var active = false
    private static var noted = Set<String>()

    /// Each glass, joined to its group or isolated by its path. A glass
    /// inside another glass answers to that one's isolation, so the pass
    /// repeats while anything changed (a few rounds at most). Nothing to do
    /// in a presenter that has never had a group.
    func reconcile() {
        let live = groups.count > 0
        guard live || active else { return }
        active = live
        for _ in 0..<3 {
            var changed = false
            for node in glass.allObjects {
                if node.reconcileGlass() { changed = true }
            }
            if !changed { return }
        }
    }

    /// Says once why a `glassGroup` draws no group (D6).
    static func note(_ reason: String, _ session: ExactSession?) {
        guard noted.insert(reason).inserted else { return }
        let why = ["spacing": "its spacing is not a finite number", "material": "the node has its own material",
                   "scroll": "the node scrolls", "canvas": "the node is a canvas"][reason] ?? reason
        session?.log("glassGroup draws no group where \(why) (LLP 1053.000.000 D6)")
    }

    /// Moves views inside `root` and gives focus back if a move took it
    /// (D2): the first responder (on macOS the field a field editor edits)
    /// and its selection.
    static func moving(in root: GlassPlatformView, _ move: () -> Void) {
        #if os(iOS)
        func focused(_ v: UIView) -> UIView? {
            if v.isFirstResponder { return v }
            for s in v.subviews { if let f = focused(s) { return f } }
            return nil
        }
        let held = root.window == nil ? nil : focused(root)
        let range = (held as? UITextInput)?.selectedTextRange
        move()
        if let held, !held.isFirstResponder, held.window != nil, held.becomeFirstResponder(), let range {
            (held as? UITextInput)?.selectedTextRange = range
        }
        #else
        guard let window = root.window else { return move() }
        var held = window.firstResponder as? NSView
        var range: NSRange?
        if let editor = held as? NSTextView {
            range = editor.selectedRange()
            if editor.isFieldEditor, let field = editor.delegate as? NSView { held = field }
        }
        guard let held, held.isDescendant(of: root) else { return move() }
        move()
        let editing = (window.firstResponder as? NSTextView).map { $0 === held || ($0.isFieldEditor && $0.delegate as? NSView === held) } ?? false
        if !editing, window.firstResponder !== held, held.window === window, window.makeFirstResponder(held), let range {
            ((held as? NSTextField)?.currentEditor() as? NSTextView ?? held as? NSTextView)?.setSelectedRange(range)
        }
        #endif
    }
}

extension NodeView {
    /// `glassGroup`'s spacing (D1): points, clamped to 0…10,000; nil when
    /// absent or not finite.
    var glassGroupSpacing: CGFloat? {
        guard let raw = props["glassGroup"], let value = Double(raw), value.isFinite else { return nil }
        return CGFloat(min(max(value, 0), 10_000))
    }

    /// Why this node's `glassGroup` draws no group, if it draws none (D6):
    /// a spacing that is not a number, its own material, a scroll, a canvas,
    /// an OS before Liquid Glass.
    var glassGroupConflict: String? {
        guard props["glassGroup"] != nil else { return nil }
        if glassGroupSpacing == nil { return "spacing" }
        if materialRequest != nil { return "material" }
        if scroll != nil || style["overflow_x"]?.string == "scroll" || style["overflow_y"]?.string == "scroll" { return "scroll" }
        if overlay != nil { return "canvas" }
        #if os(iOS)
        if #unavailable(iOS 26.0) { return "os" }
        #else
        if #unavailable(macOS 26.0) { return "os" }
        #endif
        return nil
    }

    /// The nearest glass container above a glass node — its group, or the
    /// isolation of a glass it is inside — and what on its path there the
    /// platform would not honour (D4): opacity below 1 or a mask on any view
    /// from the node up, clipping on any view above it (its own bounds hold
    /// its glass), and a canvas's overlay, whose alpha the capture changes
    /// between batches. Hidden views and transforms are honoured.
    func glassPath() -> (container: NodeView?, reasons: [String]) {
        var reasons: [String] = []
        func note(_ r: String) { if !reasons.contains(r) { reasons.append(r) } }
        var view: GlassPlatformView? = self
        while let v = view {
            #if os(iOS)
            if let group = v.superview as? GlassGroupView { return (group.owner, reasons) }
            if let slot = v.superview as? GlassSlot, slot.effect != nil { return (slot.superview as? NodeView, reasons) }
            if v.alpha < 1 { note("opacity") }
            if v.layer.mask != nil { note("mask") }
            if v !== self, v.clipsToBounds || v.layer.masksToBounds { note("clip") }
            #else
            if #available(macOS 26.0, *), let group = v.superview as? GlassGroupView { return (group.owner, reasons) }
            if v.alphaValue < 1 { note("opacity") }
            if v.layer?.mask != nil { note("mask") }
            if v !== self, v is NSClipView || v.clipsToBounds || v.layer?.masksToBounds == true { note("clip") }
            #endif
            if v !== self, (v.superview as? NodeView)?.overlay === v { note("canvas") }
            view = v.superview
        }
        return (nil, [])
    }

    /// The group a glass is in, past any isolation (D7).
    func nearestGlassGroup() -> NodeView? {
        var view = superview
        while let v = view {
            #if os(iOS)
            if let group = v as? GlassGroupView { return group.owner }
            #else
            if #available(macOS 26.0, *), let group = v as? GlassGroupView, !(group is GlassIsolationView) { return group.owner }
            #endif
            view = v.superview
        }
        return nil
    }

    /// D7: what `layout <node>` says of a group and of a grouped glass.
    func glassAgentFields(_ native: inout [String: Any]) {
        if props["glassGroup"] != nil {
            var group: [String: Any] = ["spacing": Double(glassGroupSpacing ?? 0)]
            if glassGroupView != nil {
                #if os(iOS)
                group["drawn"] = "UIGlassContainerEffect"
                #else
                group["drawn"] = "NSGlassEffectContainerView"
                #endif
            } else {
                group["drawn"] = "none"
                group["reason"] = glassGroupConflict ?? "os"
            }
            native["glassGroup"] = group
        }
        guard Materials.glass(props["backgroundMaterial"]), let group = nearestGlassGroup() else { return }
        native["glassGroupOf"] = "#\(group.id)"
        let (_, reasons) = glassPath()
        if !reasons.isEmpty { native["isolated"] = reasons }
    }
}

#if os(iOS)
/// The platform's glass container as a node's innermost view (D2). Never a
/// hit target: its children are tested without its bounds (one in visible
/// overflow is hittable), and empty group space, the bridge between merged
/// shapes included, falls through to the group's node.
final class GlassGroupView: UIVisualEffectView {
    weak var owner: NodeView?
    var spacing: CGFloat = -1
    override func hitTest(_ point: CGPoint, with event: UIEvent?) -> UIView? {
        guard !isHidden, isUserInteractionEnabled else { return nil }
        for child in contentView.subviews.reversed() {
            if let hit = child.hitTest(convert(point, to: child), with: event) { return hit }
        }
        return nil
    }
}

/// A grouped glass's slot (D4): its material inside, the effect nil while
/// joined to the group and a container while isolated, switched in place
/// (LLP 1053.000.000 §2 row 15).
final class GlassSlot: UIVisualEffectView {}

extension NodeView {
    /// What `container` names without a group: the scroll, the canvas
    /// overlay, a glass's content view, the clip box, or the node.
    var baseContainer: UIView { scroll ?? overlay ?? (Materials.glass(materialKind) ? materialView?.contentView : nil) ?? clipBox ?? self }

    /// The group view's place and spacing, or its absence (D2, D6).
    func syncGlassGroup() {
        let conflict = glassGroupConflict
        if let conflict, conflict != "os" { GlassGroups.note(conflict, presenter?.session) }
        guard props["glassGroup"] != nil, conflict == nil, let spacing = glassGroupSpacing, #available(iOS 26.0, *) else {
            if let group = glassGroupView { removeGlassGroup(group) }
            return
        }
        let base = baseContainer
        let group = glassGroupView ?? {
            let made = GlassGroupView(effect: nil)
            made.owner = self
            made.frame = base.bounds
            made.autoresizingMask = [.flexibleWidth, .flexibleHeight]
            let children = base.subviews.filter { $0 is NodeView }
            GlassGroups.moving(in: self) {
                base.insertSubview(made, at: 0)
                for child in children { made.contentView.addSubview(child) }
            }
            glassGroupView = made
            presenter?.glassGroups.groups.add(self)
            presenter?.flats.containerChanged(id)
            return made
        }()
        if group.superview !== base {
            GlassGroups.moving(in: self) { base.insertSubview(group, at: 0) }
            group.frame = base.bounds
        }
        if group.spacing != spacing {
            let effect = UIGlassContainerEffect()
            effect.spacing = spacing
            group.effect = effect
            group.spacing = spacing
        }
    }

    private func removeGlassGroup(_ group: GlassGroupView) {
        glassGroupView = nil
        let base = baseContainer
        let children = group.contentView.subviews.filter { $0 is NodeView }
        GlassGroups.moving(in: self) {
            for (i, child) in children.enumerated() { base.insertSubview(child, at: i) }
            group.removeFromSuperview()
        }
        presenter?.glassGroups.groups.remove(self)
        presenter?.flats.containerChanged(id)
    }

    /// A glass material is registered for the pass; its slot goes when it
    /// stops being glass, a material that is not glass back on the node.
    func syncGlassSlot() {
        let glass = Materials.glass(materialKind) && materialView != nil
        if glass { presenter?.glassGroups.glass.add(self) } else { presenter?.glassGroups.glass.remove(self) }
        guard !glass, let slot = glassSlot else { return }
        glassSlot = nil
        GlassGroups.moving(in: self) {
            if let material = materialView { insertSubview(material, at: 0) }
            slot.removeFromSuperview()
        }
    }

    /// Joined to its group or isolated from it by its path (D4). The slot is
    /// made when the glass is first found in a group, normally in the batch
    /// that mounts it, and then only its effect changes.
    @discardableResult func reconcileGlass() -> Bool {
        guard Materials.glass(materialKind), let material = materialView, #available(iOS 26.0, *) else { return false }
        let (container, reasons) = glassPath()
        guard container != nil else {
            guard let slot = glassSlot, slot.effect != nil else { return false }
            slot.effect = nil
            return true
        }
        let slot = glassSlot ?? {
            let made = GlassSlot(effect: nil)
            made.frame = bounds
            made.autoresizingMask = [.flexibleWidth, .flexibleHeight]
            GlassGroups.moving(in: self) {
                insertSubview(made, at: 0)
                made.contentView.addSubview(material)
            }
            glassSlot = made
            return made
        }()
        let isolated = !reasons.isEmpty
        guard (slot.effect != nil) != isolated else { return false }
        slot.effect = isolated ? UIGlassContainerEffect() : nil
        return true
    }
}
#else
/// The node's group view on macOS (D2): AppKit's container, innermost,
/// never a hit target.
@available(macOS 26.0, *)
class GlassGroupView: NSGlassEffectContainerView {
    weak var owner: NodeView?
    var appliedSpacing: CGFloat = -1
    override func hitTest(_ point: NSPoint) -> NSView? {
        guard !isHidden, let content = contentView else { return nil }
        let inContent = content.convert(point, from: superview)
        for child in content.subviews.reversed() {
            if let hit = child.hitTest(inContent) { return hit }
        }
        return nil
    }
}

/// An isolated glass's own container (D4): a group of one, owned by the
/// glass, which a glass inside it answers to; never a node's group.
@available(macOS 26.0, *)
final class GlassIsolationView: GlassGroupView {}

/// A group's or an isolation's content view: flipped like the node, and
/// never a hit target itself.
private final class GlassContent: NSView {
    override var isFlipped: Bool { true }
    override func hitTest(_ point: NSPoint) -> NSView? {
        let hit = super.hitTest(point)
        return hit === self ? nil : hit
    }
}

extension NodeView {
    /// What `container` names without a group: the scroll's document, the
    /// canvas overlay, a material's content, the clip box, or the node.
    var baseContainer: NSView { scroll?.documentView ?? overlay ?? materialContent ?? clipBox ?? self }

    /// The group view's content, where the children are while it exists.
    var glassGroupContent: NSView? {
        guard #available(macOS 26.0, *), let group = glassGroupView as? GlassGroupView else { return nil }
        return group.contentView
    }

    /// The group view's place and spacing, or its absence (D2, D6).
    func syncGlassGroup() {
        let conflict = glassGroupConflict
        if let conflict, conflict != "os" { GlassGroups.note(conflict, presenter?.session) }
        guard props["glassGroup"] != nil, conflict == nil, let spacing = glassGroupSpacing, #available(macOS 26.0, *) else {
            if let group = glassGroupView { removeGlassGroup(group) }
            return
        }
        let base = baseContainer
        let group = (glassGroupView as? GlassGroupView) ?? {
            let made = GlassGroupView(frame: base.bounds)
            made.owner = self
            made.autoresizingMask = [.width, .height]
            made.setAccessibilityElement(false)
            let content = GlassContent(frame: made.bounds)
            content.autoresizingMask = [.width, .height]
            content.setAccessibilityElement(false)
            made.contentView = content
            let children = base.subviews.filter { $0 is NodeView }
            GlassGroups.moving(in: self) {
                base.addSubview(made, positioned: .below, relativeTo: base.subviews.first)
                for child in children { content.addSubview(child) }
            }
            glassGroupView = made
            presenter?.glassGroups.groups.add(self)
            return made
        }()
        if group.superview !== base {
            GlassGroups.moving(in: self) { base.addSubview(group, positioned: .below, relativeTo: base.subviews.first) }
            group.frame = base.bounds
        }
        if group.appliedSpacing != spacing {
            group.spacing = spacing
            group.appliedSpacing = spacing
        }
    }

    private func removeGlassGroup(_ group: NSView) {
        glassGroupView = nil
        let base = baseContainer
        let content = group.subviews.first { $0 is GlassContent } ?? group
        let children = content.subviews.filter { $0 is NodeView }
        GlassGroups.moving(in: self) {
            for child in children.reversed() { base.addSubview(child, positioned: .below, relativeTo: base.subviews.first) }
            group.removeFromSuperview()
        }
        presenter?.glassGroups.groups.remove(self)
    }

    /// A glass material is registered for the pass; an isolation goes when
    /// it stops being glass.
    func syncGlassSlot() {
        let glass = Materials.glass(props["backgroundMaterial"]) && materialView != nil
        if glass { presenter?.glassGroups.glass.add(self) } else { presenter?.glassGroups.glass.remove(self) }
        if !glass { releaseGlassIsolation() }
    }

    /// The glass back on the node from its isolation container, if it is in one.
    func releaseGlassIsolation() {
        guard let isolation = glassIsolation else { return }
        glassIsolation = nil
        GlassGroups.moving(in: self) {
            if let material = materialView, material.superview !== self {
                addSubview(material, positioned: .below, relativeTo: subviews.first)
            }
            isolation.removeFromSuperview()
        }
    }

    /// Joined to its group or isolated from it by its path (D4): AppKit has
    /// no effect-less container, so the glass moves into one and back.
    @discardableResult func reconcileGlass() -> Bool {
        guard Materials.glass(props["backgroundMaterial"]), let material = materialView, #available(macOS 26.0, *) else { return false }
        let (container, reasons) = glassPath()
        let isolated = container != nil && !reasons.isEmpty
        guard isolated != (glassIsolation != nil) else { return false }
        guard isolated else { releaseGlassIsolation(); return true }
        let made = GlassIsolationView(frame: bounds)
        made.owner = self
        made.spacing = 0
        made.autoresizingMask = [.width, .height]
        made.setAccessibilityElement(false)
        let content = GlassContent(frame: made.bounds)
        content.autoresizingMask = [.width, .height]
        made.contentView = content
        GlassGroups.moving(in: self) {
            addSubview(made, positioned: .below, relativeTo: material)
            content.addSubview(material)
        }
        glassIsolation = made
        return true
    }
}
#endif
