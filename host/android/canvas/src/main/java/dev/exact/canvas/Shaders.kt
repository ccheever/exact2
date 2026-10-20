package dev.exact.canvas

import android.graphics.LinearGradient
import android.graphics.Matrix
import android.graphics.RadialGradient
import android.graphics.Shader
import android.graphics.SweepGradient
import java.nio.IntBuffer

/**
 * A `SHADER` op (43, `canvas/gradient.rs`): a box's or an SVG shape's gradient, which the next RRECT, PATH or STROKE
 * paints with in place of its colour. `[43, len, kind, spread, g×6, m×6, n, (offset, ARGB)×n]`.
 */
internal object Shaders {
    private val local = Matrix()
    private val values = FloatArray(9)

    /** The shader the op at [i] describes, or null for a malformed one. */
    fun read(b: IntBuffer, i: Int): Shader? {
        fun f(k: Int) = java.lang.Float.intBitsToFloat(b.get(i + k))
        val n = b.get(i + 16)
        if (n < 1) return null
        val offsets = FloatArray(n) { f(17 + 2 * it) }
        val colors = IntArray(n) { b.get(i + 18 + 2 * it) }
        // Android wants two stops at least; one colour is two at the same place.
        val stops = if (n == 1) floatArrayOf(offsets[0], offsets[0]) else offsets
        val inks = if (n == 1) intArrayOf(colors[0], colors[0]) else colors
        val tile = when (b.get(i + 3)) { 1 -> Shader.TileMode.REPEAT; 2 -> Shader.TileMode.MIRROR; else -> Shader.TileMode.CLAMP }
        val shader = when (b.get(i + 2)) {
            0 -> LinearGradient(f(4), f(5), f(6), f(7), inks, stops, tile)
            1 -> if (android.os.Build.VERSION.SDK_INT >= 31) RadialGradient(f(4), f(5), f(6), f(7), f(8), f(9), inks.toLongs(), stops, tile)
                else Legacy.radial(f(4), f(5), f(6), f(7), f(8), f(9), inks, stops, tile)
            2 -> SweepGradient(f(4), f(5), inks, stops)
            else -> return null
        }
        // a b c d e f, as MATRIX: x' = a x + c y + e, y' = b x + d y + f.
        values[0] = f(10); values[1] = f(12); values[2] = f(14)
        values[3] = f(11); values[4] = f(13); values[5] = f(15)
        values[6] = 0f; values[7] = 0f; values[8] = 1f
        local.setValues(values)
        shader.setLocalMatrix(local)
        return shader
    }

    private fun IntArray.toLongs() = LongArray(size) { android.graphics.Color.pack(this[it]) }
}
