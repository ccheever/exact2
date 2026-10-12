package dev.exact.canvas

import android.graphics.fonts.Font
import android.view.Gravity
import android.view.View
import android.widget.FrameLayout
import java.nio.IntBuffer

/**
 * The app's bars (LLP 1115.001 D11): a route's `header` as Material 3's top app bar and the navigation root's
 * `tablist` as its navigation bar. What counts as either is exact2's to say (the kernel's rule, which the iOS host
 * reads too), and so is the picture: each bar is painted by exact2 from Material's own metrics and replayed here in a
 * [Pane], a view of its own over the page, as a surface of the top layer is in a window of its own ([Surfaces]).
 * No library, and nothing of the framework's widgets.
 *
 * Over the page, not in it: the top bar is at the window's top and fills the status bar's area with its own colour;
 * the navigation bar is at the window's bottom and fills the gesture bar's. The page is where exact2 puts it
 * ([Native.chrome]), inset by the room the bars take. A keyboard ends the page above it and leaves the navigation
 * bar where it is, under the keyboard.
 *
 * exact2 says what the bars are with each stream ([Native.bars], [Owner.Pub.bars]), when that is not what it last
 * said, so a bar and the page under it change in one frame: which bar, its frame in this window in pixels, and its
 * picture. A bar it leaves out is gone. A touch in a bar goes back by one call ([Exact.bar]) at the bar's own
 * pixels; exact2 knows what is there. `host/linux/src/presenter/bars.rs` is exact2's side.
 *
 * A statement often arrives while the page's view is being drawn, and a bar it shows or moves must be in that same
 * frame, or the page is shown for a frame without its bar. So both views are made with the reader, in a layer of
 * their own over the page that is always there (the window's size, drawing nothing itself, taking no touch), and
 * each is laid out by hand when exact2 says where it is. A layout asked of the parent would come a frame late; and
 * the layer, which the frame's drawing reaches after the page's view, records its bars again in that same pass,
 * where a view shown straight in the reader's root would wait for the root to be recorded again.
 */
internal class Bars(activity: ExactActivity, root: FrameLayout) {
    /** The faces the bars have drawn with, by what announces each: a bar painted again makes none anew. */
    private val faces = HashMap<List<Int>, Font>()
    private val layer = FrameLayout(activity).also {
        root.addView(it, FrameLayout.LayoutParams(FrameLayout.LayoutParams.MATCH_PARENT, FrameLayout.LayoutParams.MATCH_PARENT))
    }
    private val panes = arrayOf(pane(activity, TOP), pane(activity, BOTTOM))

    private fun pane(activity: ExactActivity, which: Int): Pane {
        val pane = Pane(activity, faces, { action, x, y -> Exact.bar(which, action, x, y) }, null)
        pane.visibility = View.INVISIBLE
        layer.addView(pane, FrameLayout.LayoutParams(0, 0).apply { gravity = Gravity.TOP or Gravity.LEFT })
        return pane
    }

    /** exact2's statement of its bars ([Native.bars]). */
    fun told(w: IntArray) {
        val shown = BooleanArray(panes.size)
        var at = 1
        repeat(w[0]) {
            val which = w[at]; val n = w[at + 5]
            val pane = panes.getOrNull(which - 1)
            if (pane != null) {
                shown[which - 1] = true
                place(pane, w[at + 1], w[at + 2], w[at + 3], w[at + 4])
                pane.show(IntBuffer.wrap(w, at + 6, n).slice())
                if (pane.visibility != View.VISIBLE) pane.visibility = View.VISIBLE
            }
            at += 6 + n
        }
        for ((i, pane) in panes.withIndex()) if (!shown[i] && pane.visibility == View.VISIBLE) pane.visibility = View.INVISIBLE
    }

    /** Put [pane] at its frame now, and leave its layout parameters saying the same for the parent's next pass. */
    private fun place(pane: Pane, x: Int, y: Int, width: Int, height: Int) {
        if (pane.left == x && pane.top == y && pane.width == width && pane.height == height) return
        (pane.layoutParams as FrameLayout.LayoutParams).let { lp ->
            lp.width = width; lp.height = height; lp.leftMargin = x; lp.topMargin = y
        }
        pane.measure(
            View.MeasureSpec.makeMeasureSpec(width, View.MeasureSpec.EXACTLY),
            View.MeasureSpec.makeMeasureSpec(height, View.MeasureSpec.EXACTLY),
        )
        pane.layout(x, y, x + width, y + height)
    }

    companion object {
        /** Which bar (`presenter::bars`): the top app bar, the navigation bar. */
        const val TOP = 1
        const val BOTTOM = 2
    }
}
