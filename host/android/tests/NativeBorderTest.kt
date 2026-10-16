package com.exact.android

import android.graphics.Bitmap
import android.graphics.Canvas
import android.graphics.Color
import android.graphics.Paint
import android.graphics.Path

/** Pixel checks for general asymmetric CSS solid borders, outside timing cohorts. */
internal object NativeBorderTest {
    fun run(): String {
        val bitmap = Bitmap.createBitmap(100, 80, Bitmap.Config.ARGB_8888)
        var strokes = 0
        val canvas = object : Canvas(bitmap) {
            override fun drawRoundRect(left: Float, top: Float, right: Float, bottom: Float, rx: Float, ry: Float, paint: Paint) {
                check(left == 3f && top == 3f && right == 97f && bottom == 77f && rx == 9f && ry == 9f)
                check(paint.style == Paint.Style.STROKE && paint.strokeWidth == 6f && paint.isAntiAlias)
                strokes++; super.drawRoundRect(left, top, right, bottom, rx, ry, paint)
            }
            override fun drawPath(path: Path, paint: Paint) {
                check(paint.style == Paint.Style.FILL); super.drawPath(path, paint)
            }
        }
        val border = NativeBorders()
        val radii = FloatArray(8) { 12f }
        try {
            val midpoints = arrayOf(50 to 2, 97 to 40, 50 to 77, 2 to 40)
            for (side in 0..3) {
                val widths = FloatArray(4).apply { this[side] = 6f }
                border.invalidate(); bitmap.eraseColor(Color.WHITE)
                border.draw(canvas, 100f, 80f, widths, IntArray(4) { Color.BLUE }, FloatArray(8))
                for (other in 0..3) {
                    val (x, y) = midpoints[other]
                    check(bitmap.getPixel(x, y) == if (other == side) Color.BLUE else Color.WHITE)
                }
                check(bitmap.getPixel(50, 40) == Color.WHITE) { "separator covered content" }
            }
            border.invalidate(); bitmap.eraseColor(Color.WHITE)
            border.draw(canvas, 100f, 80f, floatArrayOf(120f, 0f, 0f, 0f), IntArray(4) { Color.BLUE }, FloatArray(8))
            check(bitmap.getPixel(50, 79) == Color.BLUE) { "oversized separator was not clamped" }
            val uniform = FloatArray(4) { 6f }
            border.invalidate(); bitmap.eraseColor(Color.WHITE)
            border.draw(canvas, 100f, 80f, uniform, IntArray(4) { Color.BLUE }, radii)
            val actual = IntArray(8000).also { bitmap.getPixels(it, 0, 100, 0, 0, 100, 80) }
            bitmap.eraseColor(Color.WHITE)
            val stroke = Paint(Paint.ANTI_ALIAS_FLAG).apply { color = Color.BLUE; style = Paint.Style.STROKE; strokeWidth = 6f }
            Canvas(bitmap).drawRoundRect(3f, 3f, 97f, 77f, 9f, 9f, stroke)
            check(actual.contentEquals(IntArray(8000).also { bitmap.getPixels(it, 0, 100, 0, 0, 100, 80) }))
            bitmap.eraseColor(Color.WHITE)
            val colors = intArrayOf(Color.RED, Color.BLUE, Color.GREEN, Color.MAGENTA)
            border.draw(canvas, 100f, 80f, uniform, colors, radii)
            for (side in 0..3) { val (x, y) = midpoints[side]; check(bitmap.getPixel(x, y) == colors[side]) }
            border.invalidate(); bitmap.eraseColor(Color.WHITE)
            border.draw(canvas, 100f, 80f, uniform, IntArray(4) { Color.BLUE }, FloatArray(8) { 100f })
            border.invalidate(); border.draw(canvas, 100f, 80f, FloatArray(4) { 20f }, IntArray(4) { Color.BLUE }, radii)
            check(strokes == 1) { "uniform primitive was skipped or normalizing radius used it" }
            border.invalidate()
            bitmap.eraseColor(Color.WHITE)
            border.draw(canvas, 100f, 80f, floatArrayOf(0f, 0f, 0f, 6f), IntArray(4) { Color.RED }, radii)
            check(bitmap.getPixel(2, 40) == Color.RED) { "left border missing" }
            check(bitmap.getPixel(50, 1) == Color.WHITE && bitmap.getPixel(98, 40) == Color.WHITE) { "one-sided border leaked" }
            check(bitmap.getPixel(0, 0) == Color.WHITE) { "rounded border corner is square" }
            border.invalidate(); bitmap.eraseColor(Color.WHITE)
            border.draw(canvas, 100f, 80f, floatArrayOf(4f, 8f, 10f, 6f), intArrayOf(Color.RED, Color.BLUE, Color.GREEN, Color.MAGENTA), radii)
            check(bitmap.getPixel(50, 1) == Color.RED && bitmap.getPixel(98, 40) == Color.BLUE)
            check(bitmap.getPixel(50, 78) == Color.GREEN && bitmap.getPixel(2, 40) == Color.MAGENTA)
            check(bitmap.getPixel(50, 40) == Color.WHITE) { "border covered content" }
            return "NativeBorderTest: PASS (uniform native stroke, radius fallback, square separators, oversized widths, rounded border and separate side colors)"
        } finally { bitmap.recycle() }
    }
}
