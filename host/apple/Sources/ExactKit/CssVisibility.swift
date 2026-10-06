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
        // The bitmap `invalidateText` left up. A fully hidden paragraph may
        // raster blank afterwards; a visible run draws through TextEngine.
        if hidden, isParagraph { retireHiddenText() }
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
