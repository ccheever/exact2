// @ref LLP 1042 §9.1. Exact's own floating window for an image's pictures, as
// `requestPictureInPicture(id, presentation=…)` asks for it in place of AVKit's
// panel. Both kinds are non-activating panels at the floating level, on every
// Space and over full-screen apps, locked to the picture's aspect and
// resizable, and show each picture the image names as AVKit's does. Neither
// brings the app forward: a press on the picture, a control or the close
// leaves the app where it is, as Electron's T3 Code v0.1.6 measured.
//
// - `"window"`: a browser's picture-in-picture window, as Chromium draws one on
//   the Mac (`VideoOverlayWindowViews`): square and borderless, at the bottom
//   right, its controls (the origin's title, minimize, back to tab, close and
//   a live timeline) over a shade while the pointer moves over it. The
//   process has one, as a browser has: another image's request moves it to
//   that image, where it stands.
// - `"panel"`: a panel with the platform's title bar, its title and close
//   button, opening where a new window opens, as a page's floating preview
//   window is (Electron's `type: "panel"`), one for each image that asks.
#if os(macOS)
import AppKit
import ImageIO

final class PictureInPictureWindow: NSObject, NSWindowDelegate {
    /// The window's own controls, by the names the agent presses them by.
    enum Control: String, CaseIterable {
        case minimize, back, close
        /// Chromium's labels, which VoiceOver reads.
        var label: String {
            switch self { case .minimize: "Minimize"; case .back: "Back to tab"; case .close: "Close" }
        }
    }

    let panel: NSPanel
    let titled: Bool
    private let content: ContentView
    private var say: (String) -> Void
    private let asked: NSSize?
    private var aspect: NSSize?
    private var opened = false
    /// No picture of the image it shows has been shown yet: the next is its enter.
    private var awaiting = true
    /// Moves on as the window follows another image, so a picture of the one before is dropped.
    private var generation = 0
    private var ended = false
    private var decoding = false
    private var waiting: String?
    private static let decoder = DispatchQueue(label: "exact.picture-in-picture.window", qos: .userInitiated)
    /// The longest side a picture is decoded at, as the video arm's.
    private static let maxPixel = 1920
    /// Where a browser's window was when it last closed: Chromium opens the next one there.
    nonisolated(unsafe) static var lastFrame: NSRect?

    init(panel titled: Bool, title: String, width: Double?, height: Double?, say: @escaping (String) -> Void) {
        self.titled = titled
        self.say = say
        if let width, let height, width > 0, height > 0 { asked = NSSize(width: width, height: height) } else { asked = nil }
        let mask: NSWindow.StyleMask = titled
            ? [.titled, .closable, .miniaturizable, .resizable, .nonactivatingPanel]
            : [.borderless, .resizable, .nonactivatingPanel]
        panel = Panel(contentRect: NSRect(x: 0, y: 0, width: 480, height: 320), styleMask: mask, backing: .buffered, defer: true)
        content = ContentView(frame: NSRect(x: 0, y: 0, width: 480, height: 320), title: title, chrome: !titled)
        super.init()
        // VoiceOver names Chromium's window by its kind; the title is the text in it.
        panel.title = titled ? title : "Picture in Picture"
        panel.isFloatingPanel = true
        panel.isRestorable = false // the author opens it; a relaunch does not
        panel.level = .floating
        panel.collectionBehavior = [.canJoinAllSpaces, .fullScreenAuxiliary]
        panel.hidesOnDeactivate = false
        panel.becomesKeyOnlyIfNeeded = true
        panel.isReleasedWhenClosed = false
        panel.isExcludedFromWindowsMenu = true
        panel.backgroundColor = .black
        panel.delegate = self
        panel.contentView = content
        if titled {
            panel.standardWindowButton(.miniaturizeButton)?.isEnabled = false
            panel.standardWindowButton(.zoomButton)?.isEnabled = false
        } else {
            // Chromium's window is square, with a dark outline and the window's shadow.
            panel.hasShadow = true
            content.layer?.borderColor = NSColor.black.withAlphaComponent(0.6).cgColor
            content.layer?.borderWidth = 1
        }
        content.pressed = { [weak self] control in self?.press(control) }
    }

    /// Open and not closed: another image's request can take it.
    var isOpen: Bool { opened && !ended }

    /// Another image floats in this window, as Chromium's shows the next video that asks: its
    /// title, its pictures from the next, and its own enter and leave.
    func follow(title: String, say: @escaping (String) -> Void) {
        self.say = say
        content.retitle(title)
        generation += 1
        waiting = nil
        awaiting = true
    }

    // MARK: Pictures

    /// The source's next picture: an absolute file path. The first opens the window.
    func frame(_ path: String) {
        guard !ended else { return }
        if decoding { waiting = path; return }
        decoding = true
        let generation = self.generation
        Self.decoder.async { [weak self] in
            let image = Self.decode(path)
            DispatchQueue.main.async {
                guard let self else { return }
                self.decoding = false
                if generation == self.generation {
                    if let image, !self.ended { self.show(image) }
                    // As the video arm: a first picture that cannot be read fails the request, and one
                    // already gone fails it only when no picture shows within a second.
                    else if image == nil, self.awaiting, self.waiting == nil {
                        if FileManager.default.fileExists(atPath: path) { self.say("error the picture could not be read") }
                        else {
                            DispatchQueue.main.asyncAfter(deadline: .now() + 1) { [weak self] in
                                if let self, self.awaiting, self.generation == generation, !self.ended { self.say("error the picture could not be read") }
                            }
                        }
                    }
                }
                if let next = self.waiting { self.waiting = nil; self.frame(next) }
            }
        }
    }

    private func show(_ image: CGImage) {
        CATransaction.begin(); CATransaction.setDisableActions(true)
        content.picture.contents = image
        CATransaction.commit()
        let size = NSSize(width: image.width, height: image.height)
        let changed = aspect.map { abs(($0.width / $0.height) / (size.width / size.height) - 1) >= 0.02 } ?? true
        if changed { aspect = size; panel.contentAspectRatio = size }
        if !opened {
            opened = true
            awaiting = false
            panel.setFrame(openingFrame(size), display: true)
            panel.minSize = minimumSize()
            if !titled { panel.maxSize = NSSize(width: Self.screen.width * 0.75, height: Self.screen.height * 0.75) }
            // Chromium's window fades in.
            panel.alphaValue = titled ? 1 : 0
            panel.orderFrontRegardless()
            if !titled { NSAnimationContext.runAnimationGroup { $0.duration = 0.25; panel.animator().alphaValue = 1 } }
            say("enter")
        } else {
            if changed, titled {
                // A new aspect keeps the content's area and the window's top left.
                let frame = panel.frame, content = panel.contentRect(forFrameRect: frame).size
                var next = panel.frameRect(forContentRect: NSRect(origin: .zero, size: Self.fit(content, aspect: size)))
                next.origin = NSPoint(x: frame.minX, y: frame.maxY - next.height)
                panel.setFrame(next, display: true, animate: false)
            } else if changed {
                // As Chromium's, for the next image's pictures as for its own: the window keeps its
                // top left and width, takes the picture's aspect, and stays on the screen.
                let frame = Self.chromiumFrame(Self.width(panel.frame.width, aspect: size, max: panel.maxSize), topLeft: NSPoint(x: panel.frame.minX, y: panel.frame.maxY), screen: Self.screen)
                panel.minSize = NSSize(width: min(panel.minSize.width, frame.width), height: min(panel.minSize.height, frame.height))
                panel.setFrame(frame, display: true, animate: false)
            }
            if awaiting { awaiting = false; say("enter") }
        }
    }

    private static func decode(_ path: String) -> CGImage? {
        autoreleasepool {
            guard let source = CGImageSourceCreateWithURL(URL(fileURLWithPath: path) as CFURL, [kCGImageSourceShouldCache: false] as CFDictionary) else { return nil }
            return CGImageSourceCreateThumbnailAtIndex(source, 0, [
                kCGImageSourceCreateThumbnailFromImageAlways: true,
                kCGImageSourceCreateThumbnailWithTransform: true,
                kCGImageSourceShouldCacheImmediately: true,
                kCGImageSourceThumbnailMaxPixelSize: maxPixel] as CFDictionary)
        }
    }

    // MARK: Geometry

    private static var screen: NSRect { (NSApp.keyWindow?.screen ?? NSScreen.main)?.visibleFrame ?? NSRect(x: 0, y: 0, width: 1440, height: 900) }

    /// Where the window opens, for a picture of `size` pixels.
    private func openingFrame(_ size: NSSize) -> NSRect {
        let screen = Self.screen
        if titled {
            // As Electron opens a window: centred on the screen at the asked size (480 × 320 where
            // none is), then its content takes the picture's aspect, keeping its area and the
            // window's top left.
            let whole = (NSApp.keyWindow?.screen ?? NSScreen.main)?.frame ?? screen
            var frame = NSRect(origin: .zero, size: asked ?? NSSize(width: 480, height: 320))
            frame.origin = NSPoint(x: (whole.midX - frame.width / 2).rounded(), y: (whole.midY - frame.height / 2).rounded())
            let content = Self.fit(panel.contentRect(forFrameRect: frame).size, aspect: size)
            var fitted = panel.frameRect(forContentRect: NSRect(origin: .zero, size: content))
            fitted.origin = NSPoint(x: frame.minX, y: frame.maxY - fitted.height)
            return fitted
        }
        if asked == nil, let last = Self.lastFrame, screen.intersects(last) {
            return Self.chromiumFrame(Self.width(last.width, aspect: size, max: NSSize(width: screen.width * 0.75, height: screen.height * 0.75)), topLeft: NSPoint(x: last.minX, y: last.maxY), screen: screen)
        }
        return Self.chromiumFrame(asked.map { Self.fit($0, aspect: size) } ?? Self.chromiumSize(size, screen: screen.size), topLeft: nil, screen: screen)
    }

    /// Chromium's window is no smaller than it opened; the panel no smaller than half its asked size.
    private func minimumSize() -> NSSize {
        if titled { return asked.map { NSSize(width: $0.width / 2, height: $0.height / 2) } ?? NSSize(width: 240, height: 160) }
        return panel.frame.size
    }

    /// Where Chromium places its window of `size` (`VideoOverlayWindowViews::CalculateAndUpdateWindowBounds`):
    /// at the bottom right of the visible frame, in from its edges by 2% of the room left beside
    /// and below the window (17 points for 284 × 178 on a 1352 × 849 frame). A window already
    /// placed keeps its top left, moved up and left only as far as that place.
    static func chromiumFrame(_ size: NSSize, topLeft: NSPoint?, screen: NSRect) -> NSRect {
        let roomX = screen.width - size.width, roomY = screen.height - size.height
        let inset = (((roomX + roomY) / 2).rounded(.down) * 0.02).rounded(.down)
        var x = screen.maxX - size.width - inset, top = screen.minY + size.height + inset
        if let topLeft { x = min(topLeft.x, x); top = max(topLeft.y, top) }
        return NSRect(x: x, y: top - size.height, width: size.width, height: size.height)
    }

    /// A window `width` wide at the picture's aspect, within `max`.
    static func width(_ width: CGFloat, aspect picture: NSSize, max: NSSize) -> NSSize {
        let aspect = picture.width / Swift.max(picture.height, 1)
        var size = NSSize(width: width.rounded(), height: (width / aspect).rounded())
        if max.height > 0, size.height > max.height { size = NSSize(width: (max.height * aspect).rounded(), height: max.height.rounded()) }
        return size
    }

    /// Chromium's opening size on the Mac, as measured beside T3 Code v0.1.6 on a 1352-point screen:
    /// 21% of the screen's width (284 points there, for pictures of either orientation), the
    /// height at the picture's aspect, at most half the screen's height.
    static func chromiumSize(_ picture: NSSize, screen: NSSize) -> NSSize {
        let aspect = picture.width / max(picture.height, 1)
        var width = (screen.width * 0.21).rounded(), height = (width / aspect).rounded()
        if height > screen.height / 2 { height = (screen.height / 2).rounded(); width = (height * aspect).rounded() }
        return NSSize(width: width, height: height)
    }

    /// `size`'s area at the picture's aspect.
    static func fit(_ size: NSSize, aspect picture: NSSize) -> NSSize {
        let aspect = picture.width / max(picture.height, 1), width = (size.width * size.height * aspect).squareRoot()
        return NSSize(width: width.rounded(), height: (width / aspect).rounded())
    }

    // MARK: Controls

    /// A control pressed (by a person or the agent): `nil` when it was done. Each of Chromium's
    /// three hides the window and the image hears it leave; none brings the app forward.
    @discardableResult func press(_ name: String) -> String? {
        guard opened, !ended else { return "the window is not open" }
        switch name {
        case "hover": content.showControls(true, animated: false)
        case Control.close.rawValue where titled: panel.performClose(nil)
        case _ where controls.contains(name): leave()
        default: return "the window has no control \(name): \(controls.joined(separator: ", ")) or hover"
        }
        return nil
    }

    /// The controls a person can press, by name.
    var controls: [String] { titled ? [Control.close.rawValue] : Control.allCases.map(\.rawValue) }

    func windowShouldClose(_ sender: NSWindow) -> Bool { leave(); return false }

    /// The person closed it: the image hears it leave, and the app closes it.
    private func leave() {
        guard !ended else { return }
        say("leave")
    }

    /// The app ends the window (`exitPictureInPicture()`, the image gone, another image floated).
    func close() {
        guard !ended else { return }
        ended = true
        if opened, !titled { Self.lastFrame = panel.frame }
        content.showControls(false, animated: false)
        panel.orderOut(nil)
        panel.close()
    }

    // MARK: Agent

    /// What the agent's tree shows of the window: its frame on screen from the top left, in points.
    var report: [String: Any] {
        let top = NSScreen.screens.first?.frame.maxY ?? 0, frame = panel.frame
        return ["presentation": titled ? "panel" : "window", "title": content.title, "open": opened && panel.isVisible,
                "frame": [frame.minX, top - frame.maxY, frame.width, frame.height].map { ($0 * 100).rounded() / 100 },
                "controls": controls, "controlsShown": titled || content.controlsShown]
    }
}

/// A non-activating panel that still takes the keyboard and VoiceOver's focus when asked.
private final class Panel: NSPanel {
    override var canBecomeKey: Bool { true }
}

/// The picture, and over it the controls of a borderless window while the pointer moves over it.
private final class ContentView: NSView {
    let picture = CALayer()
    private(set) var title: String
    private let chrome: ControlsView?
    private var tracking: NSTrackingArea?
    private var still: DispatchWorkItem?
    private(set) var controlsShown = false
    var pressed: ((String) -> Void)? { didSet { chrome?.pressed = pressed } }
    /// Chromium hides the controls once the pointer has rested this long.
    private static let rest: TimeInterval = 2.5

    init(frame: NSRect, title: String, chrome hasChrome: Bool) {
        self.title = title
        chrome = hasChrome ? ControlsView(title: title) : nil
        super.init(frame: frame)
        wantsLayer = true
        layer?.backgroundColor = NSColor.black.cgColor
        picture.contentsGravity = .resizeAspect
        picture.frame = bounds
        picture.autoresizingMask = [.layerWidthSizable, .layerHeightSizable]
        layer?.addSublayer(picture)
        if let chrome {
            chrome.frame = bounds
            chrome.autoresizingMask = [.width, .height]
            chrome.alphaValue = 0
            addSubview(chrome)
        }
    }

    required init?(coder: NSCoder) { fatalError("init(coder:) is not used") }

    func retitle(_ title: String) { self.title = title; chrome?.retitle(title) }

    override func updateTrackingAreas() {
        super.updateTrackingAreas()
        if let tracking { removeTrackingArea(tracking) }
        guard chrome != nil else { return }
        let area = NSTrackingArea(rect: bounds, options: [.mouseEnteredAndExited, .mouseMoved, .activeAlways, .inVisibleRect], owner: self)
        addTrackingArea(area)
        tracking = area
    }

    override func mouseEntered(with event: NSEvent) { moved() }
    override func mouseMoved(with event: NSEvent) { moved() }
    override func mouseExited(with event: NSEvent) { still?.cancel(); showControls(false, animated: true) }

    /// The pointer moved over the window: the controls show, until it rests.
    private func moved() {
        showControls(true, animated: true)
        still?.cancel()
        let hide = DispatchWorkItem { [weak self] in self?.showControls(false, animated: true) }
        still = hide
        DispatchQueue.main.asyncAfter(deadline: .now() + Self.rest, execute: hide)
    }

    func showControls(_ shown: Bool, animated: Bool) {
        guard let chrome, shown != controlsShown else { return }
        controlsShown = shown
        chrome.shown(shown)
        if animated {
            NSAnimationContext.runAnimationGroup { context in
                context.duration = 0.2
                chrome.animator().alphaValue = shown ? 1 : 0
            }
        } else { chrome.alphaValue = shown ? 1 : 0 }
    }

    /// A drag on the picture moves the window; a press does nothing more, as on Chromium's.
    override func mouseDown(with event: NSEvent) { window?.performDrag(with: event) }
    override var mouseDownCanMoveWindow: Bool { false }
    override func acceptsFirstMouse(for event: NSEvent?) -> Bool { true }

    // VoiceOver reads the picture as an image named by the title.
    override func isAccessibilityElement() -> Bool { true }
    override func accessibilityRole() -> NSAccessibility.Role? { .image }
    override func accessibilityLabel() -> String? { title }
    override func accessibilityChildren() -> [Any]? { chrome.map { $0.accessibleChildren } ?? [] }
}

/// Chromium's controls (`VideoOverlayWindowViews`): a shade over the whole picture, the app's
/// icon and the title at the top left, minimize, back to tab and close at the top right, and the
/// live stream's timeline, full, near the bottom. Geometry as measured on Chromium's window.
private final class ControlsView: NSView {
    var pressed: ((String) -> Void)?
    private let shade = CAGradientLayer()
    private let timeline = CALayer()
    private let icon = NSImageView()
    private let label: NSTextField
    private let scrubber = Scrubber()
    private var buttons: [ControlButton] = []

    init(title: String) {
        label = NSTextField(labelWithString: title)
        super.init(frame: .zero)
        wantsLayer = true
        // About 86% black at the top and the bottom and 76% between.
        shade.colors = [0.86, 0.76, 0.76, 0.86].map { NSColor.black.withAlphaComponent($0).cgColor }
        shade.locations = [0, 0.3, 0.7, 1]
        layer?.addSublayer(shade)
        timeline.backgroundColor = NSColor(srgbRed: 0.68, green: 0.78, blue: 0.96, alpha: 1).cgColor
        layer?.addSublayer(timeline)
        icon.image = NSApp.applicationIconImage
        icon.imageScaling = .scaleProportionallyUpOrDown
        addSubview(icon)
        label.font = .systemFont(ofSize: 13)
        label.textColor = .white
        label.lineBreakMode = .byTruncatingTail
        addSubview(label)
        buttons = PictureInPictureWindow.Control.allCases.map { control in
            let button = ControlButton(control, target: self, action: #selector(press(_:)))
            addSubview(button)
            return button
        }
        scrubber.started = Date()
    }

    required init?(coder: NSCoder) { fatalError("init(coder:) is not used") }

    override var isFlipped: Bool { true }

    /// What VoiceOver reaches while the controls show: the title, the buttons and the timeline.
    var accessibleChildren: [Any] { alphaValue > 0 ? [label] + buttons + [scrubber] : [] }

    func shown(_ shown: Bool) { NSAccessibility.post(element: self, notification: .layoutChanged) }

    override func layout() {
        super.layout()
        let bar = NSRect(x: 20, y: bounds.height - 47, width: max(0, bounds.width - 40), height: 2)
        CATransaction.begin(); CATransaction.setDisableActions(true)
        shade.frame = bounds
        timeline.frame = bar
        CATransaction.commit()
        icon.frame = NSRect(x: 12, y: 9, width: 16, height: 16)
        var x = bounds.width - 4
        for button in buttons.reversed() {
            x -= 24
            button.frame = NSRect(x: x, y: 5, width: 24, height: 24)
        }
        label.frame = NSRect(x: 36, y: 8, width: max(0, x - 40), height: 18)
        scrubber.setAccessibilityParent(self)
        scrubber.setAccessibilityFrameInParentSpace(NSRect(x: 12, y: bar.midY - 13, width: max(0, bounds.width - 24), height: 26))
    }

    func retitle(_ title: String) { label.stringValue = title }

    @objc private func press(_ sender: ControlButton) { pressed?(sender.control.rawValue) }

    // The shade, the title and the timeline take no presses: they fall to the picture, which moves the window.
    override func hitTest(_ point: NSPoint) -> NSView? {
        let hit = super.hitTest(point)
        return hit is ControlButton ? hit : nil
    }
}

/// The timeline as VoiceOver reads Chromium's: a slider whose value is the time since the stream began.
private final class Scrubber: NSAccessibilityElement {
    var started = Date()
    override func accessibilityRole() -> NSAccessibility.Role? { .slider }
    override func accessibilityLabel() -> String? { "Time scrubber" }
    override func accessibilityValue() -> Any? {
        let seconds = Int(Date().timeIntervalSince(started))
        return String(format: "%d:%02d", seconds / 60, seconds % 60)
    }
}

/// One of Chromium's buttons: a white glyph, with a round highlight under the pointer.
private final class ControlButton: NSButton {
    let control: PictureInPictureWindow.Control
    private var tracking: NSTrackingArea?
    private var over = false { didSet { needsDisplay = true } }

    init(_ control: PictureInPictureWindow.Control, target: AnyObject, action: Selector) {
        self.control = control
        super.init(frame: .zero)
        self.target = target
        self.action = action
        isBordered = false
        title = ""
        let symbol = switch control { case .minimize: "minus"; case .back: "pip.exit"; case .close: "xmark" }
        let image = NSImage(systemSymbolName: symbol, accessibilityDescription: control.label)
        self.image = image?.withSymbolConfiguration(.init(pointSize: control == .close ? 12 : 13, weight: .regular))
        contentTintColor = .white
        identifier = NSUserInterfaceItemIdentifier(control.rawValue)
        setAccessibilityLabel(control.label)
        toolTip = control.label
    }

    required init?(coder: NSCoder) { fatalError("init(coder:) is not used") }

    override func updateTrackingAreas() {
        super.updateTrackingAreas()
        if let tracking { removeTrackingArea(tracking) }
        let area = NSTrackingArea(rect: bounds, options: [.mouseEnteredAndExited, .activeAlways, .inVisibleRect], owner: self)
        addTrackingArea(area)
        tracking = area
    }

    override func mouseEntered(with event: NSEvent) { over = true }
    override func mouseExited(with event: NSEvent) { over = false }

    override func draw(_ dirtyRect: NSRect) {
        if over {
            NSColor.white.withAlphaComponent(0.1).setFill()
            NSBezierPath(ovalIn: bounds).fill()
        }
        super.draw(dirtyRect)
    }
}
#endif
