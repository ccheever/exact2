package com.exact.android

import android.graphics.Bitmap
import android.graphics.Canvas
import android.graphics.Color

/** Pixel checks for general asymmetric CSS solid borders, outside timing cohorts. */
internal object NativeBorderTest {
    fun run(): String {
        val bitmap = Bitmap.createBitmap(100, 80, Bitmap.Config.ARGB_8888)
        val canvas = Canvas(bitmap)
        val border = NativeBorders()
        val radii = FloatArray(8) { 12f }
        try {
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
            return "NativeBorderTest: PASS (rounded one-sided border, unequal widths and separate side colors)"
        } finally { bitmap.recycle() }
    }
}
