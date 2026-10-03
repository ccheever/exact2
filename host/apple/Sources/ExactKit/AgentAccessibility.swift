// @ref LLP 1080.002 D2–D7 — `tree --ax` on Apple: what UIKit and AppKit
// expose for assistive technology, read back from the platform's own
// accessibility properties after every projection the host made (swipe
// cells, text elements, native controls, modal layers). UIKit's walk is
// containment order, never a reading-order claim; AppKit's is its
// navigation order. Each element is joined to the Exact view it belongs to:
// itself, its declared owner (`AgentOwned`), or the nearest view above it.
// What the walk cannot cover it says, in `coverage`, rather than guess.
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
    /// `{"op":"tree","ax":true}` (D1, D7): validated here, at the wire, and
    /// scoped to `target` when one is named.
    func accessibilityElementsTree(_ req: [String: Any]) -> [String: Any] {
        if req["shallow"] != nil { return ["error": "tree --ax: shallow is refused with ax"] }
        var limit = 500
        if let raw = req["limit"] {
            guard let n = raw as? NSNumber, CFGetTypeID(n) == CFNumberGetTypeID(), let i = Int(exactly: n.doubleValue), (1...2000).contains(i) else {
                return ["error": "tree --ax: limit is an integer from 1 to 2000"]
            }
            limit = i
        }
        if let raw = req["excluded"], !(raw is Bool) || CFGetTypeID(raw as CFTypeRef) != CFBooleanGetTypeID() { return ["error": "tree --ax: excluded is a boolean"] }
        // A target is resolved through the runner's logical tree (inline runs
        // included), and the reply keeps the elements joined to its subtree's
        // ids, wherever the host projected them.
        var scope: Set<UInt32>?
        if let target = req["target"] {
            guard target is NSNumber || target is String,
                  let data = try? JSONSerialization.data(withJSONObject: ["op": "tree", "target": target]),
                  let tree = try? JSONSerialization.jsonObject(with: Data(session.agent(String(decoding: data, as: UTF8.self)).utf8)) as? [String: Any] else {
                return ["error": "no view matches \(target)"]
            }
            if let error = tree["error"] { return ["error": error] }
            scope = Set((tree["nodes"] as? [[String: Any]] ?? []).compactMap { ($0["id"] as? NSNumber).map { $0.uint32Value } })
        }
        guard let view = session.view, view.window != nil else { return tagged(["ax": ["unavailable": true, "reason": "unmounted"]]) }
        let session = self.session
        var limits: [[String: Any]] = []
        var modalRoot: AnyObject?
        #if os(macOS)
        switch presenter.axSheet(view.window?.attachedSheet) {
        case .owned(let content): modalRoot = content
        case .foreign: limits.append(["root": "sheet", "reason": "a sheet this session cannot be shown to own blocks its window; it is not walked"])
        case .none: break
        }
        if presenter.toolbar.projected, view.window?.toolbar != nil {
            limits.append(["root": "toolbar", "reason": "the window toolbar this session projected is not walked; its commands' authored views are hidden"])
        }
        #endif
        return tagged(["ax": presenter.axElements(roots: axRoots(view), limit: limit, excluded: req["excluded"] as? Bool == true, scope: scope, modalRoot: modalRoot,
                                                  limits: limits, foreign: { ($0 as? ExactView).map { $0.session !== session } ?? false })])
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
        if case .owned(let content) = presenter.axSheet(view.window?.attachedSheet) { roots.append(content) }
        #endif
        return roots
    }
}

extension Presenter {
    struct AxWalk {
        var elements: [[String: Any]] = []
        var objects: [AnyObject] = []
        var seen = Set<ObjectIdentifier>()
        var visited = 0, more = 0, fields = 0, cycles = 0, bytes = 0
        var stopped = false, segments = false
        let limit: Int, budget: Int, excluded: Bool
        let foreign: (AnyObject) -> Bool
        let modalRoot: AnyObject?
        var modal: [String: Any] = ["present": false]
        var modalView: AnyObject?
        var limits: [[String: Any]] = []
        var remaining: Int { budget - visited }
    }
    /// The reply's own budget (D7): elements stop once their JSON passes it.
    static let axBytes = 240 * 1024

    /// The elements under `roots`, the platform's facts each (D2, D7);
    /// `foreign` names another session's surface, where the walk stops;
    /// `modalRoot` is a presentation the platform makes modal (an AppKit sheet);
    /// `scope` keeps the elements joined to those ids, with the chain above.
    func axElements(roots: [AnyObject], limit: Int = 500, excluded: Bool = false, scope: Set<UInt32>? = nil, modalRoot: AnyObject? = nil,
                    limits: [[String: Any]] = [], foreign: @escaping (AnyObject) -> Bool = { _ in false }) -> [String: Any] {
        // A scroll view's indicators and other unexposed views cost visits too.
        var w = AxWalk(limit: limit, budget: 5 * limit + 64, excluded: excluded, foreign: foreign, modalRoot: modalRoot, limits: limits)
        for root in roots where !w.stopped { visit(root, parent: nil, depth: 0, exclusion: [], into: &w) }
        #if os(iOS)
        markModalLeaks(&w)
        let platform = "iOS \(UIDevice.current.systemVersion)", source = "uikit", order = "containment"
        var known = ["alpha", "reading-order"]
        #else
        let platform = "macOS \(ProcessInfo.processInfo.operatingSystemVersionString)", source = "appkit", order = "navigation"
        var known: [String] = []
        #endif
        // A projected segment or tab bar item joins its tablist, not its tab.
        if w.segments { known.append("segments") }
        var coverage: [String: Any] = ["roots": roots.map { String(describing: type(of: $0)) }, "complete": !w.stopped && w.more == 0 && w.limits.isEmpty,
                                       "visited": w.visited, "limits": known]
        #if os(iOS)
        if !Self.axRuntimeLoaded {
            coverage["complete"] = false
            w.limits.append(["root": "process", "reason": "UIKit's accessibility runtime is not loaded (no assistive technology or automation is on), so it derives no labels, frames or field elements; on a simulator: xcrun simctl spawn <udid> defaults write com.apple.Accessibility ApplicationAccessibilityEnabled -bool true, then relaunch"])
        }
        #endif
        if !w.limits.isEmpty { coverage["excluded"] = w.limits }
        var elements = w.elements
        var ancestors: [[String: Any]] = []
        if let scope {
            let kept = w.elements.indices.filter { k in (w.elements[k]["id"] as? UInt32).map(scope.contains) ?? false }
            if let first = kept.first {
                var p = w.elements[first]["parent"] as? Int
                while let k = p { ancestors.insert(["i": k, "id": w.elements[k]["id"] ?? NSNull(), "role": w.elements[k]["role"] ?? "", "name": w.elements[k]["name"] ?? ""], at: 0); p = w.elements[k]["parent"] as? Int }
            }
            elements = kept.map { w.elements[$0] }
        }
        var ax: [String: Any] = ["source": source, "platform": platform, "order": order, "coverage": coverage, "modal": w.modal, "elements": elements]
        if scope != nil { ax["ancestors"] = ancestors }
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
        if w.foreign(obj) { return } // another session's surface
        #if os(iOS)
        var exclusion = exclusion
        if let v = obj as? UIView {
            if v.isHidden { exclusion.append("hidden") }
            if v.accessibilityElementsHidden { exclusion.append("elementsHidden") }
        }
        if !exclusion.isEmpty && !w.excluded { return }
        let o = obj as! NSObject
        var here = parent
        if Self.isModal(obj), exclusion.isEmpty, w.modalView == nil {
            w.modalView = obj
            let i = emit(obj, role: "group", parent: parent, exclusion: exclusion, into: &w)
            let owned = i.map { !(w.elements[$0]["id"] is NSNull) } ?? false
            w.modal = ["present": true, "by": "accessibilityViewIsModal", "element": i as Any, "id": i.flatMap { w.elements[$0]["id"] } ?? NSNull()]
            // UIKit's own modal views (a presentation's dimming view) are not
            // reproduced by the sibling rule: say so instead.
            if !owned { w.limits.append(["root": String(describing: type(of: obj)), "reason": "a modal view UIKit owns: which elements VoiceOver skips beside it is not reproduced"]) }
            here = i ?? parent
        } else if o.isAccessibilityElement {
            _ = emit(obj, role: nil, parent: parent, exclusion: exclusion, into: &w); return
        } else if o.accessibilityContainerType != .none {
            here = emit(obj, role: containerRole(o.accessibilityContainerType), parent: parent, exclusion: exclusion, into: &w) ?? parent
        }
        let kids = children(of: o, into: &w)
        // UIKit's documented rule: a visible modal view hides its siblings —
        // applied where the modal view is one of the session's own.
        let modal = kids.first { Self.isModal($0) && axOwner($0).0 != nil }
        for child in kids {
            if w.stopped { break }
            let hidden = modal != nil && child !== modal
            if hidden && !w.excluded { continue }
            visit(child, parent: here, depth: depth + 1, exclusion: hidden ? exclusion + ["modalSibling"] : exclusion, into: &w)
        }
        #else
        let f = Self.axFacts(obj)
        var here = parent
        if let root = w.modalRoot, root === obj {
            here = emit(obj, role: "group", parent: parent, exclusion: exclusion, into: &w) ?? parent
            w.modal = ["present": true, "by": "sheet", "element": here as Any, "id": here.flatMap { w.elements[$0]["id"] } ?? NSNull()]
        } else if f?.element == true, depth > 0 || !(obj is ExactView) {
            here = emit(obj, role: nil, parent: parent, exclusion: exclusion, into: &w) ?? parent
        }
        for child in children(of: obj, into: &w) {
            if w.stopped { break }
            visit(child, parent: here, depth: depth + 1, exclusion: exclusion, into: &w)
        }
        #endif
    }

    #if os(iOS)
    static func isModal(_ obj: AnyObject) -> Bool { (obj as? NSObject)?.accessibilityViewIsModal == true && ((obj as? UIView).map { $0.window != nil && !$0.isHidden } ?? true) }
    /// UIKit loads the code that answers accessibility (a button's derived
    /// label, a view's frame, a field as an element) only for an assistive
    /// technology or automation; a title-only button tells which this is.
    static var axRuntimeLoaded: Bool {
        let probe = UIButton(type: .system)
        probe.setTitle("probe", for: .normal)
        return probe.accessibilityLabel == "probe"
    }
    /// A container's children, no more than the visits left (D7): a count
    /// past them stops the walk, its remainder unknown.
    private func children(of o: NSObject, into w: inout AxWalk) -> [AnyObject] {
        let room = max(0, w.remaining)
        if let declared = o.accessibilityElements, !declared.isEmpty {
            if declared.count > room { w.stopped = true }
            return declared.prefix(room).map { $0 as AnyObject }
        }
        let count = o.accessibilityElementCount()
        if count != NSNotFound, count > 0 {
            if count > room { w.stopped = true }
            return (0..<min(count, room)).compactMap { o.accessibilityElement(at: $0) as AnyObject? }
        }
        let subviews = (o as? UIView)?.subviews ?? []
        if subviews.count > room { w.stopped = true }
        return Array(subviews.prefix(room))
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
    /// AppKit's answers. A view and an element answer through their typed
    /// accessors; any other participant (a control's cell, AppKit's own
    /// parts) through the attribute protocol every AX client is served by.
    /// `enabled` is the attribute a client is sent: a view that is not a
    /// control has none, and its accessor's default `false` is not a state.
    struct AxFacts {
        var role: String?, subrole: String?, label: String?, title: String?, help: String?, value: Any?
        var enabled = true, focused = false, selected = false, element = false, expanded = false
        var frame: NSRect = .zero, identifier: String?, children: [Any] = []
    }
    static func axFacts(_ obj: AnyObject) -> AxFacts? {
        if let v = obj as? NSView {
            return AxFacts(role: v.accessibilityRole()?.rawValue, subrole: v.accessibilitySubrole()?.rawValue, label: v.accessibilityLabel(), title: v.accessibilityTitle(),
                           help: v.accessibilityHelp(), value: v.accessibilityValue(), enabled: v.accessibilityAttributeValue(.enabled) as? Bool ?? true, focused: v.isAccessibilityFocused(),
                           selected: v.isAccessibilitySelected(), element: v.isAccessibilityElement(), expanded: v.isAccessibilityExpanded(), frame: v.accessibilityFrame(), identifier: v.accessibilityIdentifier(),
                           children: v.accessibilityChildrenInNavigationOrder() ?? v.accessibilityChildren() ?? [])
        }
        if let e = obj as? NSAccessibilityElement {
            return AxFacts(role: e.accessibilityRole()?.rawValue, subrole: e.accessibilitySubrole()?.rawValue, label: e.accessibilityLabel(), title: e.accessibilityTitle(),
                           help: e.accessibilityHelp(), value: e.accessibilityValue(), enabled: e.accessibilityAttributeValue(.enabled) as? Bool ?? true, focused: e.isAccessibilityFocused(),
                           selected: e.isAccessibilitySelected(), element: e.isAccessibilityElement(), expanded: e.isAccessibilityExpanded(), frame: e.accessibilityFrame(), identifier: e.accessibilityIdentifier(),
                           children: e.accessibilityChildrenInNavigationOrder() ?? e.accessibilityChildren() ?? [])
        }
        guard let o = obj as? NSObject else { return nil }
        func a(_ name: String) -> Any? { o.accessibilityAttributeValue(NSAccessibility.Attribute(rawValue: name)) }
        // AXPosition is the top-left corner in global display points, y down from the primary screen's top.
        // A cell fills its control: the control's frame is the cell's.
        var frame = (obj as? NSCell)?.controlView?.accessibilityFrame() ?? .zero
        if frame.isEmpty, let p = (a("AXPosition") as? NSValue)?.pointValue, let s = (a("AXSize") as? NSValue)?.sizeValue {
            let top = NSScreen.screens.first?.frame.maxY ?? 0
            frame = NSRect(x: p.x, y: top - p.y - s.height, width: s.width, height: s.height)
        }
        return AxFacts(role: a("AXRole") as? String, subrole: a("AXSubrole") as? String, label: a("AXDescription") as? String, title: a("AXTitle") as? String,
                       help: a("AXHelp") as? String, value: a("AXValue"), enabled: a("AXEnabled") as? Bool ?? true, focused: a("AXFocused") as? Bool ?? false,
                       selected: a("AXSelected") as? Bool ?? false, element: !o.accessibilityIsIgnored(), expanded: a("AXExpanded") as? Bool ?? false, frame: frame, identifier: a("AXIdentifier") as? String,
                       children: a("AXChildren") as? [Any] ?? [])
    }
    enum AxSheet { case none, owned(NSView), foreign }
    /// A sheet attached to the window is this session's only when its
    /// content holds this presenter's own views (D3); any other sheet blocks
    /// the window and is someone else's.
    func axSheet(_ sheet: NSWindow?) -> AxSheet {
        guard let sheet else { return .none }
        guard let content = sheet.contentView else { return .foreign }
        let mine = viewport.window === sheet || views.values.contains { $0.window === sheet }
        return mine ? .owned(content) : .foreign
    }
    private func children(of obj: AnyObject, into w: inout AxWalk) -> [AnyObject] {
        let all = NSAccessibility.unignoredChildren(from: Self.axFacts(obj)?.children ?? [])
        let room = max(0, w.remaining)
        if all.count > room { w.stopped = true }
        return all.prefix(room).map { $0 as AnyObject }
    }
    #endif

    /// One element (D3, D5, D7): the platform's facts, cut to their bounds.
    private func emit(_ obj: AnyObject, role forced: String?, parent: Int?, exclusion: [String], into w: inout AxWalk) -> Int? {
        if w.elements.count >= w.limit { w.more += 1; return nil }
        if w.bytes > Self.axBytes { w.stopped = true; return nil }
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
        if Self.underSegments(obj) { w.segments = true }
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
        if #available(iOS 18, *) {
            switch o.accessibilityExpandedStatus { case .expanded: states["expanded"] = true; case .collapsed: states["expanded"] = false; default: break }
        }
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
        // A secure field answers its subrole through its cell; the classes are the same fact.
        let secure = f.subrole == NSAccessibility.Subrole.secureTextField.rawValue || obj is NSSecureTextField || obj is NSSecureTextFieldCell
        // A text field with neither description nor title is spoken by its placeholder.
        let placeholder = (obj as? NSObject)?.accessibilityAttributeValue(.placeholderValue) as? String
        let name = [f.label, f.title, placeholder].compactMap { $0 }.first { !$0.isEmpty } ?? ""
        e["name"] = cut(name) ?? ""
        if let v = f.value, !secure { let s = "\(v)"; if !s.isEmpty { e["value"] = cut(s) } }
        if let help = f.help, !help.isEmpty { e["description"] = cut(help) }
        if secure { states["protected"] = true }
        if !f.enabled { states["disabled"] = true }
        if f.focused { states["focused"] = true }
        if f.selected { states["selected"] = true }
        // AppKit's accessor answers false for "not expanded" and "no such state" alike: only true is a fact.
        if f.expanded { states["expanded"] = true }
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
        w.bytes += (try? JSONSerialization.data(withJSONObject: e).count) ?? 0
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
        // A control's cell is the control's (its accessibility is the cell's).
        let host = (obj as? NSCell)?.controlView ?? (obj as? NSView)
        if let n = host as? NodeView, obj is NSCell { return (n.id, "owner") }
        var v = obj is NSCell ? host : host?.superview
        let control = host is NSControl
        while let s = v { if let n = s as? NodeView { return (n.id, control ? "owner" : "ancestor") }; v = s.superview }
        #endif
        return (nil, "none")
    }

    /// A segment or tab bar item the host projected from a tablist (D5's
    /// segment join is not built: such an element joins its tablist).
    private static func underSegments(_ obj: AnyObject) -> Bool {
        #if os(iOS)
        var v: UIView? = (obj as? UIView) ?? ((obj as? UIAccessibilityElement)?.accessibilityContainer as? UIView)
        while let s = v { if s is UISegmentedControl || s is UITabBar { return true }; v = s.superview }
        #else
        var v: NSView? = (obj as? NSView) ?? (obj as? NSCell)?.controlView ?? ((obj as? NSAccessibilityElement)?.accessibilityParent() as? NSView)
        while let s = v { if s is NSSegmentedControl { return true }; v = s.superview }
        #endif
        return false
    }

    private func axTestId(_ id: UInt32?) -> String? { id.flatMap { views[$0]?.props["testId"] ?? inlineText($0)?.props["testId"] } }
    private func round2(_ x: CGFloat) -> Double { (Double(x) * 100).rounded() / 100 }

    #if os(iOS)
    /// An element outside a session-owned modal view, and outside the
    /// siblings UIKit's rule hides, is reachable around it: `outsideModal`.
    private func markModalLeaks(_ w: inout AxWalk) {
        guard let modal = w.modalView as? UIView, axOwner(modal).0 != nil else { return }
        for (k, obj) in w.objects.enumerated() where w.elements[k]["excluded"] == nil {
            var view = obj as? UIView
            if view == nil, let e = obj as? UIAccessibilityElement { view = e.accessibilityContainer as? UIView }
            // A container holding the modal view is around it, not beside it.
            guard let view, !modal.isDescendant(of: view) else { continue }
            w.elements[k]["outsideModal"] = !(view === modal || view.isDescendant(of: modal))
        }
    }
    #endif
}
