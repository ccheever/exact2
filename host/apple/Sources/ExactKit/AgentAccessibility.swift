// @ref LLP 1080.002 D2–D7 — `tree --ax` on Apple: what UIKit and AppKit
// expose for assistive technology, read back from the platform's own
// accessibility properties after every projection the host made (swipe
// cells, text elements, native controls, modal layers). UIKit's walk is
// containment order, never a reading-order claim; AppKit's is its
// navigation order. Each element is joined to the Exact view it belongs to:
// itself, its declared owner (`AgentOwned`), or the nearest view above it.
// The driver adds the views' intent and the findings (`scripts/agent-ax.mjs`).
#if os(macOS)
import AppKit
#else
import UIKit
#endif

/// An accessibility object standing for one Exact view it is not itself:
/// a swipe cell's authored control, an inline run's element (D5).
protocol AgentOwned { var agentViewId: UInt32? { get } }

extension Agent {
    /// `{"op":"tree","ax":true}`: the walk over this session's own surfaces (D3).
    func accessibilityElementsTree(_ req: [String: Any]) -> [String: Any] {
        let limit = req["limit"] as? Int ?? 500
        guard (1...2000).contains(limit) else { return ["error": "tree --ax: limit is an integer from 1 to 2000"] }
        guard let view = session.view, view.window != nil else { return tagged(["ax": ["unavailable": true, "reason": "unmounted"]]) }
        let session = self.session
        return tagged(["ax": presenter.axElements(roots: axRoots(view), limit: limit, excluded: req["excluded"] as? Bool == true,
                                                  foreign: { ($0 as? ExactView).map { $0.session !== session } ?? false })])
    }

    /// The session's own surfaces (D3): its view, a presentation holding its
    /// viewport, and on macOS a sheet attached to its window.
    private func axRoots(_ view: ExactView) -> [AnyObject] {
        var roots: [AnyObject] = [view]
        #if os(iOS)
        // A modal layer moves the viewport into a presented controller (ModalIOS).
        var v: UIView? = presenter.viewport
        while let s = v?.superview, !(s is UIWindow) { v = s }
        if let top = v, top !== view, !view.isDescendant(of: top), !top.isDescendant(of: view) { roots.append(top) }
        #else
        if let sheet = view.window?.attachedSheet, let content = sheet.contentView { roots.append(content) }
        #endif
        return roots
    }
}

extension Presenter {
    struct AxWalk {
        var elements: [[String: Any]] = []
        var objects: [AnyObject] = []
        var seen = Set<ObjectIdentifier>()
        var visited = 0, more = 0, fields = 0, cycles = 0
        var stopped = false
        let limit: Int, budget: Int, excluded: Bool
        let foreign: (AnyObject) -> Bool
        var modal: [String: Any] = ["present": false]
        var modalView: AnyObject?
    }

    /// The elements under `roots`, the platform's facts each (D2, D7);
    /// `foreign` names another session's surface, where the walk stops.
    func axElements(roots: [AnyObject], limit: Int = 500, excluded: Bool = false, foreign: @escaping (AnyObject) -> Bool = { _ in false }) -> [String: Any] {
        var w = AxWalk(limit: limit, budget: 5 * limit + 64, excluded: excluded, foreign: foreign) // a scroll view's indicators and other unexposed views cost visits too
        for root in roots { visit(root, parent: nil, depth: 0, exclusion: [], into: &w) }
        #if os(iOS)
        markModalLeaks(&w)
        let platform = "iOS \(UIDevice.current.systemVersion)", source = "uikit", order = "containment"
        #else
        let platform = "macOS \(ProcessInfo.processInfo.operatingSystemVersionString)", source = "appkit", order = "navigation"
        #endif
        var coverage: [String: Any] = ["roots": roots.map { String(describing: type(of: $0)) }, "complete": !w.stopped && w.more == 0, "visited": w.visited,
                                       "limits": order == "containment" ? ["alpha", "reading-order"] : [String]()]
        #if os(iOS)
        if !Self.axRuntimeLoaded {
            coverage["complete"] = false
            coverage["excluded"] = [["root": "process", "reason": "UIKit's accessibility runtime is not loaded (no assistive technology or automation is on), so it derives no labels, frames or field elements; on a simulator: xcrun simctl spawn <udid> defaults write com.apple.Accessibility ApplicationAccessibilityEnabled -bool true, then relaunch"]]
        }
        #endif
        var ax: [String: Any] = [
            "source": source, "platform": platform, "order": order, "coverage": coverage,
            "modal": w.modal, "elements": w.elements,
        ]
        if w.stopped || w.more > 0 || w.fields > 0 {
            ax["truncated"] = ["elements": w.stopped ? "unknown" as Any : w.more as Any, "fields": w.fields]
        }
        if w.cycles > 0 { ax["cycles"] = w.cycles }
        return ax
    }

    private func visit(_ obj: AnyObject, parent: Int?, depth: Int, exclusion: [String], into w: inout AxWalk) {
        guard !w.stopped else { return }
        let key = ObjectIdentifier(obj)
        if w.seen.contains(key) { w.cycles += 1; return }
        w.seen.insert(key)
        w.visited += 1
        if w.visited > w.budget || depth > 64 { w.stopped = true; return }
        #if os(iOS)
        if w.foreign(obj) { return } // another session's surface
        var exclusion = exclusion
        if let v = obj as? UIView {
            if v.isHidden { exclusion.append("hidden") }
            if v.accessibilityElementsHidden { exclusion.append("elementsHidden") }
        }
        if !exclusion.isEmpty && !w.excluded { return }
        let o = obj as! NSObject
        if o.accessibilityViewIsModal, exclusion.isEmpty, (obj as? UIView)?.window != nil, w.modalView == nil {
            w.modalView = obj
            let i = emit(obj, role: "group", parent: parent, exclusion: exclusion, into: &w)
            w.modal = ["present": true, "by": "accessibilityViewIsModal", "element": i as Any, "id": i.flatMap { w.elements[$0]["id"] } ?? NSNull()]
            for child in children(of: o) { visit(child, parent: i ?? parent, depth: depth + 1, exclusion: exclusion, into: &w) }
            return
        }
        if o.isAccessibilityElement { _ = emit(obj, role: nil, parent: parent, exclusion: exclusion, into: &w); return }
        var here = parent
        if o.accessibilityContainerType != .none { here = emit(obj, role: containerRole(o.accessibilityContainerType), parent: parent, exclusion: exclusion, into: &w) ?? parent }
        for child in children(of: o) { visit(child, parent: here, depth: depth + 1, exclusion: exclusion, into: &w) }
        #else
        if w.foreign(obj) { return }
        let element = Self.axFacts(obj)?.element == true
        var here = parent
        if element, depth > 0 || !(obj is ExactView) { here = emit(obj, role: nil, parent: parent, exclusion: exclusion, into: &w) ?? parent }
        for child in children(of: obj) { visit(child, parent: here, depth: depth + 1, exclusion: exclusion, into: &w) }
        #endif
    }

    #if os(iOS)
    /// UIKit loads the code that answers accessibility (a button's derived
    /// label, a view's frame, a field as an element) only for an assistive
    /// technology or automation; a title-only button tells which this is.
    static var axRuntimeLoaded: Bool {
        let probe = UIButton(type: .system)
        probe.setTitle("probe", for: .normal)
        return probe.accessibilityLabel == "probe"
    }
    private func children(of o: NSObject) -> [AnyObject] {
        if let declared = o.accessibilityElements, !declared.isEmpty { return declared.map { $0 as AnyObject } }
        let count = o.accessibilityElementCount()
        if count != NSNotFound, count > 0 { return (0..<count).compactMap { o.accessibilityElement(at: $0) as AnyObject? } }
        return (o as? UIView)?.subviews ?? []
    }
    private func containerRole(_ t: UIAccessibilityContainerType) -> String {
        switch t { case .list: "list"; case .landmark: "landmark"; case .dataTable: "table"; case .semanticGroup: "group"; default: "group" }
    }
    private static let traitNames: [(UIAccessibilityTraits, String)] = [
        (.button, "button"), (.link, "link"), (.header, "header"), (.searchField, "searchField"), (.image, "image"),
        (.selected, "selected"), (.playsSound, "playsSound"), (.keyboardKey, "keyboardKey"), (.staticText, "staticText"),
        (.summaryElement, "summaryElement"), (.notEnabled, "notEnabled"), (.updatesFrequently, "updatesFrequently"),
        (.startsMediaSession, "startsMediaSession"), (.adjustable, "adjustable"), (.allowsDirectInteraction, "allowsDirectInteraction"),
        (.causesPageTurn, "causesPageTurn"), (.tabBar, "tabBar"), (.toggleButton, "toggleButton"), (.supportsZoom, "supportsZoom"),
    ]
    #else
    /// AppKit's answers for the two kinds of object its tree holds: a view
    /// and an element (the inline runs). Anything else has none to give.
    /// `enabled` is the attribute an AX client is sent: a view that is not a
    /// control has none, and its accessor's default `false` is not a state.
    struct AxFacts {
        var role: String?, subrole: String?, label: String?, title: String?, help: String?, value: Any?
        var enabled = true, focused = false, selected = false, element = false
        var frame: NSRect = .zero, identifier: String?, children: [Any] = []
    }
    static func axFacts(_ obj: AnyObject) -> AxFacts? {
        if let v = obj as? NSView {
            return AxFacts(role: v.accessibilityRole()?.rawValue, subrole: v.accessibilitySubrole()?.rawValue, label: v.accessibilityLabel(), title: v.accessibilityTitle(),
                           help: v.accessibilityHelp(), value: v.accessibilityValue(), enabled: v.accessibilityAttributeValue(.enabled) as? Bool ?? true, focused: v.isAccessibilityFocused(),
                           selected: v.isAccessibilitySelected(), element: v.isAccessibilityElement(), frame: v.accessibilityFrame(), identifier: v.accessibilityIdentifier(),
                           children: v.accessibilityChildrenInNavigationOrder() ?? v.accessibilityChildren() ?? [])
        }
        if let e = obj as? NSAccessibilityElement {
            return AxFacts(role: e.accessibilityRole()?.rawValue, subrole: e.accessibilitySubrole()?.rawValue, label: e.accessibilityLabel(), title: e.accessibilityTitle(),
                           help: e.accessibilityHelp(), value: e.accessibilityValue(), enabled: e.accessibilityAttributeValue(.enabled) as? Bool ?? true, focused: e.isAccessibilityFocused(),
                           selected: e.isAccessibilitySelected(), element: e.isAccessibilityElement(), frame: e.accessibilityFrame(), identifier: e.accessibilityIdentifier(),
                           children: e.accessibilityChildrenInNavigationOrder() ?? e.accessibilityChildren() ?? [])
        }
        return nil
    }
    private func children(of obj: AnyObject) -> [AnyObject] {
        NSAccessibility.unignoredChildren(from: Self.axFacts(obj)?.children ?? []).map { $0 as AnyObject }
    }
    #endif

    /// One element (D3, D5, D7): the platform's facts, cut to their bounds.
    private func emit(_ obj: AnyObject, role forced: String?, parent: Int?, exclusion: [String], into w: inout AxWalk) -> Int? {
        if w.elements.count >= w.limit { w.more += 1; return nil }
        var fields = 0
        func cut(_ s: String?) -> String? {
            guard let s else { return nil }
            if s.count <= 200 { return s }
            fields += 1
            return String(s.prefix(199)) + "…"
        }
        let i = w.elements.count
        var e: [String: Any] = ["i": i, "parent": parent ?? NSNull()]
        let (id, via) = axOwner(obj)
        e["id"] = id ?? NSNull()
        e["via"] = via
        var states: [String: Any] = [:]
        var native: [String: Any] = ["class": String(describing: type(of: obj))]
        #if os(iOS)
        let o = obj as! NSObject
        let traits = o.accessibilityTraits
        let names = Self.traitNames.filter { traits.contains($0.0) }.map(\.1)
        let secure = (obj as? UITextField)?.isSecureTextEntry == true
        let label = o.accessibilityLabel ?? "", value = o.accessibilityValue
        e["name"] = cut(label) ?? ""
        if let value, !value.isEmpty, !secure { e["value"] = cut(value) }
        if let hint = o.accessibilityHint, !hint.isEmpty { e["description"] = cut(hint) }
        if secure { states["protected"] = true }
        if traits.contains(.notEnabled) { states["disabled"] = true }
        if traits.contains(.selected) { states["selected"] = true }
        let editable = obj is UITextField || obj is UITextView
        e["role"] = forced ?? (editable ? "textbox" : names.contains("button") && (value == "checked" || value == "unchecked") ? "checkbox"
            : names.contains("link") ? "link" : names.contains("header") ? "heading" : names.contains("searchField") ? "searchbox"
            : names.contains("button") ? "button" : names.contains("image") ? "image" : names.contains("adjustable") ? "adjustable"
            : names.contains("tabBar") ? "tablist" : names.contains("staticText") ? "text" : "unknown")
        if e["role"] as? String == "checkbox" { states["checked"] = value == "checked" }
        e["interactive"] = forced == nil && (o.accessibilityRespondsToUserInteraction || editable || names.contains("button") || names.contains("link") || names.contains("adjustable"))
        if let actions = o.accessibilityCustomActions, !actions.isEmpty { e["actions"] = actions.prefix(16).compactMap { cut($0.name) } }
        native["role"] = names
        if let ident = (o as? UIAccessibilityIdentification)?.accessibilityIdentifier, ident != axTestId(id) { native["identifier"] = cut(ident) }
        let frame = o.accessibilityFrame
        if !frame.isEmpty {
            let vp = viewport
            if let window = vp.window {
                let r = vp.convert(frame, from: window.screen.coordinateSpace)
                e["frame"] = ["x": round2(r.minX - vp.contentOffset.x), "y": round2(r.minY - vp.contentOffset.y), "w": round2(r.width), "h": round2(r.height), "source": "element"]
            }
        }
        #else
        let f = Self.axFacts(obj) ?? AxFacts()
        // AppKit's secure field answers its subrole through its cell; the view's class is the same fact.
        let secure = f.subrole == NSAccessibility.Subrole.secureTextField.rawValue || obj is NSSecureTextField
        let name = (f.label?.isEmpty == false ? f.label : f.title) ?? ""
        e["name"] = cut(name) ?? ""
        if let v = f.value, !secure { let s = "\(v)"; if !s.isEmpty { e["value"] = cut(s) } }
        if let help = f.help, !help.isEmpty { e["description"] = cut(help) }
        if secure { states["protected"] = true }
        if !f.enabled { states["disabled"] = true }
        if f.focused { states["focused"] = true }
        if f.selected { states["selected"] = true }
        let r = f.role ?? "AXUnknown"
        let mapped = ["AXButton": "button", "AXLink": "link", "AXHeading": "heading", "AXTextField": "textbox", "AXTextArea": "textbox",
                      "AXCheckBox": "checkbox", "AXStaticText": "text", "AXGroup": "group", "AXImage": "image", "AXList": "list"][r]
        e["role"] = forced ?? mapped ?? r
        if r == "AXHeading", let level = f.value as? Int { states["level"] = level }
        if r == "AXCheckBox", let on = f.value as? Int { states["checked"] = on == 1 }
        e["interactive"] = ["AXButton", "AXLink", "AXTextField", "AXTextArea", "AXCheckBox", "AXRadioButton", "AXSlider", "AXPopUpButton", "AXMenuItem", "AXComboBox"].contains(r)
        native["role"] = r
        if let subrole = f.subrole { native["subrole"] = subrole }
        if let ident = f.identifier, !ident.isEmpty, ident != axTestId(id) { native["identifier"] = cut(ident) }
        if !f.frame.isEmpty, let window = viewport.window {
            // Screen points, y up, into the viewport's: the clip view's, less its scroll.
            let clip = viewport.contentView
            let r = clip.convert(window.convertFromScreen(f.frame), from: nil)
            let y = clip.isFlipped ? r.minY - clip.bounds.origin.y : clip.bounds.maxY - r.maxY
            e["frame"] = ["x": round2(r.minX - clip.bounds.origin.x), "y": round2(y), "w": round2(r.width), "h": round2(r.height), "source": "element"]
        }
        #endif
        if !exclusion.isEmpty { e["excluded"] = exclusion }
        e["states"] = states
        e["native"] = native
        if let t = axTestId(id), via != "ancestor" { e["testId"] = t }
        w.fields += fields
        w.elements.append(e)
        w.objects.append(obj)
        return i
    }

    /// D5: the view an element is, the view that declared it, or the view above it.
    private func axOwner(_ obj: AnyObject) -> (UInt32?, String) {
        if let n = obj as? NodeView { return (n.id, "self") }
        if let owned = obj as? AgentOwned, let id = owned.agentViewId { return (id, "owner") }
        #if os(iOS)
        if let e = obj as? UIAccessibilityElement, let n = e.accessibilityContainer as? NodeView { return (n.id, "owner") }
        var v = (obj as? UIView)?.superview
        let control = obj is UIControl
        while let s = v { if let n = s as? NodeView { return (n.id, control && n.isAccessibilityElement == false ? "owner" : "ancestor") }; v = s.superview }
        #else
        if let e = obj as? NSAccessibilityElement, let n = e.accessibilityParent() as? NodeView { return (n.id, "owner") }
        var v = (obj as? NSView)?.superview
        let control = obj is NSControl
        while let s = v { if let n = s as? NodeView { return (n.id, control ? "owner" : "ancestor") }; v = s.superview }
        #endif
        return (nil, "none")
    }

    private func axTestId(_ id: UInt32?) -> String? { id.flatMap { views[$0]?.props["testId"] ?? inlineText($0)?.props["testId"] } }
    private func round2(_ x: CGFloat) -> Double { (Double(x) * 100).rounded() / 100 }

    #if os(iOS)
    /// UIKit's documented rule (UIAccessibility.h): a visible modal view hides
    /// its siblings from VoiceOver. An element neither inside it nor inside
    /// a sibling of it is reachable around it: `outsideModal`.
    private func markModalLeaks(_ w: inout AxWalk) {
        guard let modal = w.modalView as? UIView, let parent = modal.superview else { return }
        let siblings = parent.subviews.filter { $0 !== modal }
        for (k, obj) in w.objects.enumerated() {
            var view = obj as? UIView
            if view == nil, let e = obj as? UIAccessibilityElement { view = e.accessibilityContainer as? UIView }
            guard let view else { continue }
            if view === modal || view.isDescendant(of: modal) { continue }
            let hidden = siblings.contains { view === $0 || view.isDescendant(of: $0) }
            w.elements[k]["outsideModal"] = !hidden
        }
    }
    #endif
}
