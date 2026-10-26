package dev.exact.canvas

import android.app.Activity
import android.content.pm.ActivityInfo
import android.os.Build
import android.view.Surface
import org.json.JSONObject

/**
 * The session's orientation lock (LLP 1133.029 D6): the activity's `requestedOrientation`. exact2 tells the lock
 * when it changes and anew to a reader made again (request kind 13, `host/linux/src/presenter/requests.rs`): none,
 * or the orientations its types allow as bits in the Screen Orientation API's order (portrait-primary 1,
 * portrait-secondary 2, landscape-primary 4, landscape-secondary 8), whether a holder says `natural` or `current`,
 * and each holder and its word. A set is the manifest's own mapping (`build.gradle.kts`) for that type; `current`
 * is `SCREEN_ORIENTATION_LOCKED`, which "locks the orientation to its current rotation"; none restores the
 * activity's value from the manifest.
 *
 * A lock narrows the manifest and never widens it. On a display 600 dp wide or wider an app targeting Android 16
 * has every type ignored ("orientation, resizability, and aspect ratio restrictions no longer apply"), so there no
 * type is asked and `current` alone holds, as on an iPad. What was asked, and why not, is the agent's
 * `state.orientationlock.applied`.
 */
internal class Orientation(private val activity: Activity, private val string: (IntArray, Int) -> Pair<String, Int>) {
    /** The activity's value from the manifest, as it stood at the first lock (`natural` already resolved to the
     * panel's axis by the app's activity). */
    private var manifest: Int? = null
    private var said: String? = null

    fun told(w: IntArray, at: Int) {
        val from = manifest ?: activity.requestedOrientation.also { manifest = it }
        if (w[at] == 0) {
            activity.requestedOrientation = from
            said = null
            report(JSONObject().put("asked", JSONObject.NULL).put("held", false), false, null)
            return
        }
        val types = w[at + 1]; val natural = w[at + 2] != 0; val current = w[at + 3] != 0
        val holders = ArrayList<String>()
        var i = at + 5
        repeat(w[at + 4]) {
            val view = w[i]
            val (word, next) = string(w, i + 1)
            holders.add("view $view \"$word\"")
            i = next
        }
        val now = rotationBit()
        val set = types and (if (natural) naturalBit() else ALL) and (if (current) now else ALL)
        val wide = activity.resources.configuration.smallestScreenWidthDp >= 600 && Build.VERSION.SDK_INT >= 36
        val allowed = if (wide) ALL else bits(from)
        val reply = JSONObject().put("rotation", ORIENTATIONS.getOrNull(Integer.numberOfTrailingZeros(now)) ?: "unknown")
        var line: String? = null
        var conflict = false
        val value: Int? = when {
            set == 0 -> { conflict = true; line = "orientationlock: conflict: ${holders.joinToString(", ")}; none held"; reply.put("why", "conflict"); null }
            current -> ActivityInfo.SCREEN_ORIENTATION_LOCKED
            wide -> { reply.put("why", "a display 600 dp wide applies no type"); null }
            set and allowed == 0 -> { line = "orientationlock: ${holders.joinToString(", ")}: this device does not show it"; reply.put("why", "this device does not show it"); null }
            else -> value(set and allowed, from)
        }
        activity.requestedOrientation = value ?: from
        reply.put("asked", value?.let { NAMES[it] ?: it.toString() } ?: JSONObject.NULL).put("held", value != null)
        report(reply, conflict, line.takeIf { it != said })
        said = line
    }

    private fun report(reply: JSONObject, conflict: Boolean, line: String?) = Exact.orientationApplied(reply.toString(), conflict, line)

    /** The activity's value for a set: the manifest's own for every orientation it allows. */
    private fun value(set: Int, from: Int): Int = when (set) {
        bits(from) -> from
        0b0011 -> ActivityInfo.SCREEN_ORIENTATION_SENSOR_PORTRAIT
        0b1100 -> ActivityInfo.SCREEN_ORIENTATION_SENSOR_LANDSCAPE
        0b0001 -> ActivityInfo.SCREEN_ORIENTATION_PORTRAIT
        0b0010 -> ActivityInfo.SCREEN_ORIENTATION_REVERSE_PORTRAIT
        0b0100 -> ActivityInfo.SCREEN_ORIENTATION_LANDSCAPE
        0b1000 -> ActivityInfo.SCREEN_ORIENTATION_REVERSE_LANDSCAPE
        else -> ActivityInfo.SCREEN_ORIENTATION_FULL_USER
    }

    /** The orientations an activity value allows. */
    private fun bits(value: Int): Int = when (value) {
        ActivityInfo.SCREEN_ORIENTATION_PORTRAIT -> 0b0001
        ActivityInfo.SCREEN_ORIENTATION_REVERSE_PORTRAIT -> 0b0010
        ActivityInfo.SCREEN_ORIENTATION_SENSOR_PORTRAIT, ActivityInfo.SCREEN_ORIENTATION_USER_PORTRAIT -> 0b0011
        ActivityInfo.SCREEN_ORIENTATION_LANDSCAPE -> 0b0100
        ActivityInfo.SCREEN_ORIENTATION_REVERSE_LANDSCAPE -> 0b1000
        ActivityInfo.SCREEN_ORIENTATION_SENSOR_LANDSCAPE, ActivityInfo.SCREEN_ORIENTATION_USER_LANDSCAPE -> 0b1100
        ActivityInfo.SCREEN_ORIENTATION_NOSENSOR -> naturalBit()
        else -> ALL
    }

    private fun display() = if (Build.VERSION.SDK_INT >= 30) activity.display else Legacy.display(activity)

    /** Whether the display's own panel stands portrait (its mode's physical size). */
    private fun portraitPanel(): Boolean {
        val mode = display()?.mode
        return mode == null || mode.physicalHeight >= mode.physicalWidth
    }

    private fun naturalBit() = if (portraitPanel()) 0b0001 else 0b0100

    /** The display's orientation now, as Chrome on Android names it from the rotation and the panel's axis. */
    private fun rotationBit(): Int {
        val rotation = display()?.rotation ?: Surface.ROTATION_0
        val order = if (portraitPanel()) intArrayOf(0b0001, 0b0100, 0b0010, 0b1000) else intArrayOf(0b0100, 0b0001, 0b1000, 0b0010)
        return order[rotation and 3]
    }

    private companion object {
        const val ALL = 0b1111
        val ORIENTATIONS = arrayOf("portrait-primary", "portrait-secondary", "landscape-primary", "landscape-secondary")
        val NAMES = mapOf(
            ActivityInfo.SCREEN_ORIENTATION_LOCKED to "locked", ActivityInfo.SCREEN_ORIENTATION_SENSOR_PORTRAIT to "sensorPortrait",
            ActivityInfo.SCREEN_ORIENTATION_SENSOR_LANDSCAPE to "sensorLandscape", ActivityInfo.SCREEN_ORIENTATION_PORTRAIT to "portrait",
            ActivityInfo.SCREEN_ORIENTATION_REVERSE_PORTRAIT to "reversePortrait", ActivityInfo.SCREEN_ORIENTATION_LANDSCAPE to "landscape",
            ActivityInfo.SCREEN_ORIENTATION_REVERSE_LANDSCAPE to "reverseLandscape", ActivityInfo.SCREEN_ORIENTATION_FULL_USER to "fullUser",
        )
    }
}
