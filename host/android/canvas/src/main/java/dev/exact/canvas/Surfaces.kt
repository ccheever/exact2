package dev.exact.canvas

import android.annotation.SuppressLint
import android.app.Dialog
import android.content.Context
import android.graphics.Canvas
import android.graphics.Color
import android.graphics.Outline
import android.graphics.drawable.ColorDrawable
import android.graphics.fonts.Font
import android.os.SystemClock
import android.util.Log
import android.view.Gravity
import android.view.KeyEvent
import android.view.MotionEvent
import android.view.View
import android.view.ViewGroup
import android.view.ViewOutlineProvider
import android.view.Window
import android.widget.PopupWindow
import java.nio.IntBuffer

/**
 * The view a picture of exact2's own is replayed in: a surface of the top layer, as the content of its window, and
 * a bar ([Bars]), over the page. exact2 paints it (Material 3's basic dialog, its menu, its top app bar, its
 * navigation bar) and records it as an op stream of its own, whole, which this replays as [ExactView] replays the
 * page's, with a [Player] and faces of its own. A touch here is told to exact2 at the picture's own pixels
 * ([touch]: 0 down, 1 up, 2 move, 3 cancel); exact2 knows what is where and answers with the picture painted again
 * (a row lit) or gone. [key]: what a hardware keyboard's key in it is told by, for a view that takes the focus (a
 * surface's window); null for one that takes none (a bar).
 */
@SuppressLint("ViewConstructor")
internal class Pane(
    context: Context, private val faces: HashMap<List<Int>, Font>,
    private val touch: (Int, Float, Float) -> Unit, private val key: ((Int) -> Unit)?,
) : View(context) {
    private val fonts = HashMap<Int, Font>()
    private val player = Player(fonts)
    private var ops: IntBuffer? = null
    private val walker = Structure()
    /** The corner radius of the surface's container, in pixels: the shape its window's shadow is cast from. */
    var radius = 0f
    /** Escape in a window that does not take it itself (a popup's): dismiss it. */
    var escape: (() -> Unit)? = null
    /** Called once, when the surface's first frame has been drawn and committed, with when each happened in
     * milliseconds after [since] (`x_log_surfaces=1`). */
    var first: ((Long, Long) -> Unit)? = null
    /** When the touch that opened the surface lifted ([SystemClock.uptimeMillis]). */
    var since = 0L
    private var drawn = 0L
    /** What this pane answers accessibility services with, where it is a surface's window ([Access], LLP 1115.001
     * D13): the surface's own nodes. A bar's pane has none, and says nothing itself: the reader's root answers for
     * what is in it. */
    var access: Access? = null

    override fun getAccessibilityNodeProvider(): android.view.accessibility.AccessibilityNodeProvider? = access?.provider

    override fun dispatchHoverEvent(event: MotionEvent): Boolean = access?.hover(event) == true || super.dispatchHoverEvent(event)

    init {
        // Keys reach the window's focused view: a hardware keyboard's Tab, arrows and Enter go to exact2 from here.
        isFocusable = key != null
        isFocusableInTouchMode = key != null
        defaultFocusHighlightEnabled = false
        outlineProvider = object : ViewOutlineProvider() {
            override fun getOutline(view: View, outline: Outline) = outline.setRoundRect(0, 0, view.width, view.height, radius)
        }
    }

    /** A new picture of the surface: the faces it announces made (each file's face once, however often a stream
     * names it), then drawn. */
    fun show(stream: IntBuffer) {
        fonts.clear()
        var i = 0
        val n = stream.limit()
        while (i < n) {
            val op = stream.get(i)
            val end = walker.end(stream, i)
            if (op == 10 || op == 41) {
                // A face is its file, index, weight and axes: every word of the op but its key.
                val words = List(end - i - 2) { stream.get(i + 2 + it) }
                fonts[stream.get(i + 1)] = faces.getOrPut(words) { fontAt(stream, i) }
            }
            i = end
        }
        ops = stream
        invalidate()
    }

    override fun onAttachedToWindow() {
        super.onAttachedToWindow()
        val told = first ?: return
        first = null
        // Registered before the window's first traversal, so it is the first frame's commit that calls it: the
        // frame is then the compositor's, and shows at the next vsync.
        viewTreeObserver.registerFrameCommitCallback { told(drawn - since, SystemClock.uptimeMillis() - since) }
    }

    override fun onDraw(canvas: Canvas) {
        val b = ops ?: return
        player.width = width; player.height = height
        player.replay(canvas, b, 0, b.limit())
        if (drawn == 0L) drawn = SystemClock.uptimeMillis()
    }

    @SuppressLint("ClickableViewAccessibility")
    override fun onTouchEvent(e: MotionEvent): Boolean {
        val action = when (e.actionMasked) {
            MotionEvent.ACTION_DOWN -> 0; MotionEvent.ACTION_UP -> 1; MotionEvent.ACTION_MOVE -> 2; else -> 3
        }
        if (action == 1) Exact.touched = e.eventTime
        touch(action, e.x, e.y)
        return true
    }

    override fun onKeyDown(keyCode: Int, event: KeyEvent): Boolean {
        val tell = key ?: return super.onKeyDown(keyCode, event)
        val key = when (keyCode) {
            KeyEvent.KEYCODE_ENTER, KeyEvent.KEYCODE_NUMPAD_ENTER, KeyEvent.KEYCODE_DPAD_CENTER -> Surfaces.ENTER
            KeyEvent.KEYCODE_TAB -> if (event.isShiftPressed) Surfaces.TAB_BACK else Surfaces.TAB
            KeyEvent.KEYCODE_DPAD_UP -> Surfaces.UP; KeyEvent.KEYCODE_DPAD_DOWN -> Surfaces.DOWN
            KeyEvent.KEYCODE_DPAD_LEFT -> Surfaces.LEFT; KeyEvent.KEYCODE_DPAD_RIGHT -> Surfaces.RIGHT
            KeyEvent.KEYCODE_SPACE -> Surfaces.SPACE
            KeyEvent.KEYCODE_ESCAPE -> { val leave = escape ?: return super.onKeyDown(keyCode, event); leave(); return true }
            else -> return super.onKeyDown(keyCode, event)
        }
        tell(key)
        return true
    }
}

/**
 * The top layer's windows (LLP 1115.001 D10). A confirmation and a menu are windows of the platform's own, not a
 * layer painted in the page: an alert is a [Dialog] and a menu a [PopupWindow], the framework's classes and no
 * library. What a window gives is the system's: it stacks above the page and every view the page hosts (a text
 * field, a web view) with nothing arranged here; a dialog's scrim is the window manager's dim, which covers the
 * system bars; Back, Escape and a tap outside go to the window by the system's rules; and a window reports itself to
 * accessibility services. What is in the window is exact2's picture of the surface, replayed in a [Pane].
 *
 * exact2 says what the top layer shows as a statement on the one channel out ([Requests], kind 7), told when it
 * changes: a surface's serial, what it is, its flags, its frame in this activity's window and its size in pixels,
 * its container's radius and elevation, and its picture. A serial this holds no window for opens one; the same
 * serial again is that surface painted again (a row lit under a finger, another scheme, another font scale), or
 * moved or sized again; `what` 0 closes it. What happens in the window goes back by one call ([Exact.surface]).
 * `host/linux/src/presenter/top.rs` is exact2's side.
 *
 * Their motion is the platform's. The dialog has what the platform gives its window, with nothing set here: filmed
 * on a Pixel 10 Pro XL (Android 16), it fades and settles into place over about 180 ms as the scrim fades in, and
 * leaves in about 140 ms, its scrim in about 50. The menu is given its motion here, since the platform gives none
 * to a popup it did not place: [PopupWindow]'s default animation is the drop-down's, chosen only for a popup shown
 * from an anchor view, and one placed by [PopupWindow.showAtLocation] has no window animation at all. So the menu's
 * window keeps exact2's placement and is given a window animation of its own ([PopupWindow.setAnimationStyle]),
 * which the window manager runs as it runs any window's: Material's menu, as Compose's `DropdownMenu` moves it, a
 * scale from 80% at the corner by its anchor over 120 ms with a 30 ms fade in, and a 75 ms fade out
 * (`res/anim/exact_menu_*`, `res/values/motion.xml`). The system's animation scale and "remove animations" apply.
 */
internal class Surfaces(private val activity: ExactActivity, private val root: View) {
    /** Called when a surface's window comes over the page (true) and when it goes (false): the page under one is
     * inert, and what the page hosts is told so ([Fields.covered]). */
    var covering: (Boolean) -> Unit = {}
    /** Whether a surface's window is over the page now. */
    val open: Boolean get() = shown != null

    /** One surface as it shows: its window, and whether exact2 already knows it is going. */
    private class Shown(val serial: Int, val pane: Pane, val dialog: Dialog?, val popup: PopupWindow?) {
        var quiet = false
        var frame = IntArray(4)
    }

    private var shown: Shown? = null
    /** exact2's accessibility tree while it is a surface's ([Access.Told], LLP 1115.001 D13): the one that is open,
     * or one whose window is about to be made (the tree comes with the stream, the window's request after it). */
    private var tree: Access.Told? = null

    /** exact2's statement of the accessibility tree: a surface's ([t] names which), or the page's again (null). The
     * open surface's pane answers with it, and an alert's window is titled by its headline, as the platform's own
     * alert is, so TalkBack says it when the window comes. */
    fun access(t: Access.Told?) {
        tree = t
        val s = shown ?: return
        if (t == null || t.surface != s.serial) return
        s.pane.access?.told(t)
        t.headline?.let { s.dialog?.window?.setTitle(it) }
    }

    /** The faces surfaces have drawn with, by what announces each: a surface painted again makes none anew. */
    private val faces = HashMap<List<Int>, Font>()
    /** `x_log_surfaces=1` (exact2's EXACT_LOG_SURFACES): when each surface's window is asked for, made and first
     * drawn, measured from the touch that opened it, to the log. */
    private val log = android.system.Os.getenv("EXACT_LOG_SURFACES") == "1"

    /** exact2's statement of a surface: the words of request kind 7 at [at] ([Requests.perform]). */
    fun told(w: IntArray, at: Int) {
        val serial = w[at]; val what = w[at + 1]; val flags = w[at + 2]
        val frame = intArrayOf(w[at + 3], w[at + 4], w[at + 5], w[at + 6])
        val radius = java.lang.Float.intBitsToFloat(w[at + 7]); val elevation = java.lang.Float.intBitsToFloat(w[at + 8])
        val now = shown
        if (what == CLOSED) { if (now?.serial == serial) close(now); return }
        // One surface at a time: another one's window goes before this one's comes.
        if (now != null && now.serial != serial) close(now)
        val stream = IntBuffer.wrap(w, at + 10, w[at + 9]).slice()
        val same = shown
        try {
            if (same != null) update(same, frame, radius, stream)
            else open(serial, what, flags, frame, radius, elevation, stream)
        } catch (e: RuntimeException) {
            // The window could not be made (no window to attach to: the activity is going): exact2 closes the
            // surface, so the page is not left inert under nothing.
            Log.w(TAG, "surface $serial: $e")
            shown?.let { close(it) }
            Exact.surface(serial, REFUSED, 0f, 0f)
        }
    }

    private fun open(serial: Int, what: Int, flags: Int, frame: IntArray, radius: Float, elevation: Float, stream: IntBuffer) {
        val pane = Pane(activity, faces, { action, x, y -> Exact.surface(serial, action, x, y) }) { key -> Exact.surface(serial, key, 0f, 0f) }
        pane.radius = radius
        if (log) {
            val asked = SystemClock.uptimeMillis() - Exact.touched
            val kind = if (what == ALERT) "alert" else "menu"
            pane.since = Exact.touched
            pane.first = { drawn, committed ->
                Log.i(TAG, "surface $serial ($kind): asked $asked ms after the touch lifted, first drawn at $drawn ms, its frame committed at $committed ms")
            }
        }
        pane.show(stream)
        pane.access = Access(pane)
        val known = tree?.takeIf { it.surface == serial }
        known?.let { pane.access?.told(it) }
        val made = when (what) {
            ALERT -> {
                // The framework's dialog window, with the activity's own dialog theme for its dim and its motion.
                // Nothing of the theme's is drawn: the window is clear, and the picture is the dialog.
                val dialog = Dialog(activity)
                dialog.requestWindowFeature(Window.FEATURE_NO_TITLE)
                dialog.setContentView(pane, ViewGroup.LayoutParams(frame[2], frame[3]))
                dialog.window?.let { window ->
                    window.setBackgroundDrawable(ColorDrawable(Color.TRANSPARENT))
                    window.setElevation(0f)
                    window.setLayout(frame[2], frame[3])
                    // Material's scrim: 32% over everything behind the dialog, the system bars included.
                    window.setDimAmount(SCRIM)
                }
                // An alert has no outside unless its author wrote one (`closedby="any"`, LLP 1115.000 D13.4): exact2
                // says which, as it does for a menu. Back and Escape cancel it where exact2 says they may.
                dialog.setCancelable(flags and BACK_DISMISSES != 0)
                dialog.setCanceledOnTouchOutside(flags and OUTSIDE_DISMISSES != 0)
                dialog.setOnCancelListener { dismissed(serial) }
                known?.headline?.let { dialog.window?.setTitle(it) }
                Shown(serial, pane, dialog, null).also { dialog.show() }
            }
            MENU -> {
                // The framework's popup window in the theme's own style; its background is not used. The window
                // casts the shadow, from the pane's outline. Placed where exact2 says, not dropped from an anchor
                // view.
                val popup = PopupWindow(activity)
                popup.setBackgroundDrawable(null)
                popup.contentView = pane
                popup.width = frame[2]; popup.height = frame[3]
                popup.elevation = elevation
                pane.clipToOutline = true
                // Its motion: the window manager's, from the corner by its anchor (the class's note on motion).
                popup.animationStyle = when ((flags shr 2) and 3) {
                    0 -> R.style.ExactMenu_TopStart
                    1 -> R.style.ExactMenu_TopEnd
                    2 -> R.style.ExactMenu_BottomStart
                    else -> R.style.ExactMenu_BottomEnd
                }
                // Focusable: Back goes to it and the page under it takes no touch; a tap outside dismisses it.
                popup.isFocusable = true
                popup.isOutsideTouchable = flags and OUTSIDE_DISMISSES != 0
                pane.escape = { popup.dismiss() }
                val s = Shown(serial, pane, null, popup)
                popup.setOnDismissListener { if (!s.quiet) dismissed(serial) }
                popup.showAtLocation(root, Gravity.NO_GRAVITY, frame[0], frame[1])
                s
            }
            else -> throw IllegalArgumentException("a surface of kind $what")
        }
        made.frame = frame
        shown = made
        covering(true)
        pane.requestFocus()
    }

    /** The same surface again: painted again, and its window moved or sized where its frame is another. */
    private fun update(s: Shown, frame: IntArray, radius: Float, stream: IntBuffer) {
        if (s.pane.radius != radius) { s.pane.radius = radius; s.pane.invalidateOutline() }
        s.pane.show(stream)
        if (frame.contentEquals(s.frame)) return
        s.frame = frame
        s.dialog?.let { d ->
            s.pane.layoutParams = s.pane.layoutParams.apply { width = frame[2]; height = frame[3] }
            d.window?.setLayout(frame[2], frame[3])
        }
        s.popup?.update(frame[0], frame[1], frame[2], frame[3])
    }

    /** exact2 says the surface closed (a row was chosen, an action closed it, it no longer shows what is there):
     * its window goes, and exact2 is told nothing of that. */
    private fun close(s: Shown) {
        s.quiet = true
        if (shown === s) { shown = null; covering(false) }
        try { s.dialog?.dismiss(); s.popup?.dismiss() } catch (e: RuntimeException) { Log.w(TAG, "surface ${s.serial}: $e") }
    }

    /** The system dismissed the window by its own rules (Back, Escape, a tap outside a menu): exact2 closes the
     * surface, as its cancel closes it. */
    private fun dismissed(serial: Int) {
        if (shown?.serial == serial) { shown = null; covering(false) }
        Exact.surface(serial, DISMISSED, 0f, 0f)
    }

    /** The activity goes: its windows go with it, and exact2 is told nothing. A surface still open there is told
     * again to the reader that comes next, which makes its window anew. */
    fun destroy() {
        shown?.let { close(it) }
    }

    companion object {
        private const val TAG = "exact"
        /** What a surface is (`presenter::top::SurfaceKind`), and 0 for one that closed. */
        const val CLOSED = 0
        const val ALERT = 1
        const val MENU = 2
        /** The flags: Back and Escape dismiss it; a tap outside dismisses it. Bits 2 and 3 are the corner a menu
         * grows from: its right side, its bottom. */
        const val BACK_DISMISSES = 1
        const val OUTSIDE_DISMISSES = 2
        /** What is told back beside a touch's 0 to 3 (`presenter::top::event`). */
        const val DISMISSED = 8
        const val REFUSED = 9
        const val ENTER = 16
        const val TAB = 17
        const val TAB_BACK = 18
        const val UP = 19
        const val DOWN = 20
        const val LEFT = 21
        const val RIGHT = 22
        const val SPACE = 23
        /** Material 3's scrim under a dialog: its `scrim` role at 32%. */
        const val SCRIM = 0.32f
    }
}
