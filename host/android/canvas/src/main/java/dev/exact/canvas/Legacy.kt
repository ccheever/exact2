package dev.exact.canvas

import android.app.Activity
import android.graphics.Point
import android.graphics.RadialGradient
import android.graphics.Shader
import android.view.Display
import android.view.HapticFeedbackConstants
import android.view.View
import android.view.Window
import android.view.WindowInsets
import android.view.WindowManager
import android.view.accessibility.AccessibilityEvent
import android.view.accessibility.AccessibilityNodeInfo
import android.view.inputmethod.InputMethodManager

/**
 * The reader on Android 10 and 11 (API 29 and 30) where it otherwise calls a platform API of a later release: each
 * function here is entered only from a `Build.VERSION.SDK_INT` check, so a newer release never runs one. Glyph runs
 * are [LegacyGlyphs]'.
 */
@Suppress("DEPRECATION")
object Legacy {
    /** The display the activity is on: `Context.getDisplay` is API 30. */
    fun display(activity: Activity): Display = activity.windowManager.defaultDisplay

    /** The window's size before `WindowManager.getCurrentWindowMetrics` (API 30): the display's, which the reader's
     * full-screen window has until its root is laid out and says its own. */
    internal fun windowSize(activity: Activity): Pair<Int, Int> {
        val size = Point()
        activity.windowManager.defaultDisplay.getRealSize(size)
        return size.x to size.y
    }

    /** Edge to edge before `Window.setDecorFitsSystemWindows` (API 30): the layout flags that put the window under the
     * status and navigation bars, and the cutout's short edges (`ALWAYS` is API 30), which a portrait window's are. */
    internal fun edgeToEdge(window: Window) {
        val v = window.decorView
        v.systemUiVisibility = v.systemUiVisibility or View.SYSTEM_UI_FLAG_LAYOUT_STABLE or
            View.SYSTEM_UI_FLAG_LAYOUT_FULLSCREEN or View.SYSTEM_UI_FLAG_LAYOUT_HIDE_NAVIGATION
        window.attributes = window.attributes.apply {
            layoutInDisplayCutoutMode = WindowManager.LayoutParams.LAYOUT_IN_DISPLAY_CUTOUT_MODE_SHORT_EDGES
        }
    }

    /** Dark icons in the bars over a light page, before `WindowInsetsController` (API 30). */
    internal fun lightBars(window: Window, light: Boolean) {
        val v = window.decorView
        val bits = View.SYSTEM_UI_FLAG_LIGHT_STATUS_BAR or View.SYSTEM_UI_FLAG_LIGHT_NAVIGATION_BAR
        v.systemUiVisibility = if (light) v.systemUiVisibility or bits else v.systemUiVisibility and bits.inv()
    }

    /** [ExactActivity]'s four insets from what API 29 has (`WindowInsets.Type` is API 30), read from the root's
     * window insets: the decor view consumes the stable insets of those it dispatches. Under `ADJUST_RESIZE` the
     * system window insets include the software keyboard; the stable insets are the bars alone, shown or not. */
    internal fun edges(insets: WindowInsets, out: IntArray) {
        val system = insets.systemWindowInsets; val stable = insets.stableInsets
        val bottom = minOf(system.bottom, stable.bottom)
        out[0] = system.top; out[1] = system.right; out[2] = bottom; out[3] = system.left
        insets.displayCutout?.let { out[4] = it.safeInsetTop; out[5] = it.safeInsetRight; out[6] = it.safeInsetBottom; out[7] = it.safeInsetLeft }
        if (system.bottom > stable.bottom) out[10] = system.bottom
        val g = insets.systemGestureInsets
        out[12] = g.top; out[13] = g.right; out[14] = g.bottom; out[15] = g.left
    }

    /** Ask for the software keyboard for [view] (`WindowInsetsController.show(ime())` is API 30). */
    internal fun showKeyboard(view: View) {
        view.context.getSystemService(InputMethodManager::class.java)?.showSoftInput(view, 0)
    }

    /** Accessibility's node, event and collection and range information before their public constructors (API 30). */
    internal fun node(host: View): AccessibilityNodeInfo = AccessibilityNodeInfo.obtain(host)
    internal fun node(host: View, id: Int): AccessibilityNodeInfo = AccessibilityNodeInfo.obtain(host, id)
    internal fun event(type: Int): AccessibilityEvent = AccessibilityEvent.obtain(type)
    internal fun item(row: Int, column: Int, selected: Boolean): AccessibilityNodeInfo.CollectionItemInfo =
        AccessibilityNodeInfo.CollectionItemInfo.obtain(row, 1, column, 1, false, selected)
    internal fun collection(rows: Int, columns: Int, selection: Int): AccessibilityNodeInfo.CollectionInfo =
        AccessibilityNodeInfo.CollectionInfo.obtain(rows, columns, false, selection)
    internal fun range(min: Float, max: Float, now: Float): AccessibilityNodeInfo.RangeInfo =
        AccessibilityNodeInfo.RangeInfo.obtain(AccessibilityNodeInfo.RangeInfo.RANGE_TYPE_FLOAT, min, max, now)

    /** A node's value before `AccessibilityNodeInfo.stateDescription` (API 30): said after its name, in its
     * description. */
    internal fun state(info: AccessibilityNodeInfo, value: CharSequence) {
        val name = info.contentDescription ?: info.text
        info.contentDescription = if (name.isNullOrEmpty()) value else "$name, $value"
    }

    /** `HapticFeedbackConstants.CONFIRM` (7) and `REJECT` (8, 9), API 30, as AndroidX's
     * `HapticFeedbackConstantsCompat` falls back for them. */
    internal fun haptic(kind: Int): Int = if (kind == 7) HapticFeedbackConstants.VIRTUAL_KEY else HapticFeedbackConstants.LONG_PRESS

    /** Material 3's baseline scheme (no dynamic colour before Android 12), [Palette]'s roles in its order: light, then
     * dark. */
    internal fun palette(): IntArray = longArrayOf(
        0xFF6750A4, 0xFFFFFFFF, 0xFFE8DEF8, 0xFFFEF7FF, 0xFF1D1B20, 0xFF49454F,
        0xFFF7F2FA, 0xFFF3EDF7, 0xFFECE6F0, 0xFFE6E0E9, 0xFFCAC4D0, 0xFFB3261E,
        0xFFD0BCFF, 0xFF381E72, 0xFF4A4458, 0xFF141218, 0xFFE6E0E9, 0xFFCAC4D0,
        0xFF1D1B20, 0xFF211F26, 0xFF2B2930, 0xFF36343B, 0xFF49454F, 0xFFF2B8B5,
    ).let { roles -> IntArray(roles.size) { roles[it].toInt() } }

    /** A two-circle radial gradient before RadialGradient's constructor for one (API 31): the end circle's, with the
     * start circle's radius folded into the stops when the two share a centre (CSS's case). A start circle elsewhere
     * is not drawn as such. */
    internal fun radial(x0: Float, y0: Float, r0: Float, x1: Float, y1: Float, r1: Float, colors: IntArray, stops: FloatArray, tile: Shader.TileMode): Shader {
        val inner = if (x0 == x1 && y0 == y1 && r1 > 0f) (r0 / r1).coerceIn(0f, 1f) else 0f
        return RadialGradient(x1, y1, maxOf(r1, 1e-3f), colors, FloatArray(stops.size) { inner + stops[it] * (1f - inner) }, tile)
    }
}
