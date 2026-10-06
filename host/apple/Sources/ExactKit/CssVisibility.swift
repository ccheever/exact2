// CSS `visibility` (inherited). `hidden` keeps the box and turns off that
// element's own painting, hit-testing and accessibility. A descendant that
// computes `visible` still paints and is hit. `display: none` is `isHidden`
// and removes the subtree; this does not.
#if os(macOS)
import AppKit
#else
import UIKit
#endif

extension NodeView {
    /// Computed `visibility: hidden`. Absent is the initial `visible`.
    var cssVisibilityHidden: Bool { style["visibility"]?.string == "hidden" }

    /// A hidden paragraph that still has an inline run computing `visible`.
    /// A paragraph that is not hidden answers false without walking its runs,
    /// so a visible paragraph's raster decision does not scan them.
    var paintsVisibleInlineRun: Bool {
        guard cssVisibilityHidden else { return false }
        return inlineText.contains { $0.paints && !$0.run(dark: drawsDark).hidden }
    }

    /// In the accessibility tree when this element paints, or when a run inside
    /// a hidden paragraph still computes `visible` (the web keeps that span).
    var accessibilityExposed: Bool { !cssVisibilityHidden || paintsVisibleInlineRun }

    /// This node's own platform content. Child node views keep their own
    /// computed visibility, and so does a scroll or clip that holds them.
    func applyCssVisibility() {
        let hidden = cssVisibilityHidden
        field?.isHidden = hidden
        textArea?.isHidden = hidden
        symbolView?.isHidden = hidden
        web?.isHidden = hidden
        metal?.isHidden = hidden
        video?.applyCssHidden(hidden)
        presenter?.controls.controls[id]?.isHidden = hidden
        #if os(macOS)
        textAreaScroll?.isHidden = hidden
        symbolClip?.isHidden = hidden
        #endif
        if hidden { giveUpFocus() }
        guard isParagraph else { return }
        // The bitmap `invalidateText` left up. A fully hidden paragraph may
        // raster blank afterwards; a visible run draws through TextEngine.
        if hidden { retireHiddenText() }
        if hidden || inlineText.contains(where: { $0.paints && $0.run(dark: drawsDark).hidden }) { updateTextAccessibility() }
    }

    /// A box hidden now no longer holds the focus it had: no ring, no keys
    /// (its own, or its field's or text area's editor). On the next turn:
    /// ending an edit sends its `change` at once, which must not apply a batch
    /// inside the one applying this style.
    func giveUpFocus() {
        DispatchQueue.main.async { [weak self] in
            guard let self, self.cssVisibilityHidden else { return }
            #if os(macOS)
            guard let window = self.window, window.firstResponder === self || window.firstResponder === self.textArea
                || self.field?.currentEditor() != nil else { return }
            window.makeFirstResponder(nil)
            #else
            for responder in [self, self.field, self.textArea].compactMap({ $0 }) where responder.isFirstResponder {
                _ = responder.resignFirstResponder()
            }
            #if os(tvOS)
            if self.isFocused { self.setNeedsFocusUpdate(); self.updateFocusIfNeeded() } // the remote's focus moves on
            #endif
            #endif
        }
    }

    /// Take down the text bitmap this paragraph was already showing.
    /// `dropTextRaster` clears the layer only while the paragraph still
    /// rasters. A visible run makes it stop, and the old surface would stay.
    func retireHiddenText() {
        dropTextRaster()
        #if os(macOS)
        layer?.contents = nil
        needsDisplay = true
        #else
        setNeedsDisplay()
        #endif
    }

    /// The glyph under `point` (this view's coordinates) belongs to an inline
    /// run whose computed visibility is `visible`. The host sends that
    /// computed value on the run (`hidden` when it is hidden).
    func inlineRunShows(at point: CGPoint) -> Bool {
        guard isParagraph, let run = inlineTarget(at: point) else { return false }
        return !run.run(dark: drawsDark).hidden
    }

    #if os(macOS)
    /// `pointer-events: none`, or this box's own hit while it is
    /// `visibility: hidden`. A descendant node, and a visible inline run,
    /// are not refused.
    func refusesOwnHit(_ hit: NSView, local: CGPoint) -> Bool {
        let events = style["pointer_events"]?.string == "none"
        guard events || cssVisibilityHidden else { return false }
        var owner: NSView? = hit
        while let view = owner, !(view is NodeView) { owner = view.superview }
        guard owner === self else { return false }
        return events || !inlineRunShows(at: local)
    }
    #endif
}
