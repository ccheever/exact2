package com.exact.android

import android.content.Context
import android.graphics.Bitmap
import android.graphics.Canvas
import android.graphics.Color
import android.os.Looper
import org.json.JSONArray
import org.json.JSONObject
import java.nio.ByteBuffer
import java.nio.ByteOrder
import kotlin.math.ceil

/** Invoke once on the UI thread in a private test Activity, outside timing cohorts. */
internal object InlineTextPaintTest {
    private fun ByteBuffer.utf8(value: String) {
        val bytes = value.toByteArray(Charsets.UTF_8)
        putInt(bytes.size); put(bytes)
    }
    private fun ByteBuffer.style(size: Float, weight: Int = 400, family: Int = 0, italic: Boolean = false) {
        putFloat(size); putInt(weight); putInt(family); putInt(if (italic) 1 else 0)
        putInt(0); putFloat(0f); putFloat(0f); putInt(0)
    }
    private fun request(view: Int, runs: List<String>, family: Int = 0): ByteBuffer =
        ByteBuffer.allocateDirect(4096).order(ByteOrder.LITTLE_ENDIAN).apply {
            putInt(1); putInt(view); putInt(view); putInt(1); putLong(1)
            putFloat(280f); putFloat(-1f)
            putInt(0); putInt(0); putInt(0); putInt(0); putInt(0); putInt(0); putInt(runs.size); putInt(0)
            style(30f, family = family)
            for ((index, text) in runs.withIndex()) {
                style(30f, if (index == 0) 700 else 400, family, index == 1)
                utf8(text)
            }
            flip()
        }
    private fun rows(first: Int, second: Int, underline: Boolean = true): JSONArray {
        fun channels(color: Int) = JSONArray(listOf(Color.red(color), Color.green(color), Color.blue(color), Color.alpha(color)))
        val rows = JSONArray()
        for ((index, text) in listOf("MMMM ", "MMMM").withIndex()) rows.put(JSONObject()
            .put("id", 11 + index).put("parent", 10).put("paint", true)
            .put("props", JSONObject().put("text", text)).put("handlers", JSONArray())
            .put("style", JSONObject().put("text_color", channels(if (index == 0) first else second))
                .put("background_color", channels(if (index == 0) Color.GREEN else Color.TRANSPARENT))
                .put("text_decoration_line", if (index == 0 && underline) "underline" else "none")))
        return rows
    }
    private fun metrics(engine: TextEngine, request: ByteBuffer): List<Float> {
        val out = engine.measure(request, request.limit())
        return List(3) { out.getFloat(it * 4) }
    }
    private fun count(bitmap: Bitmap, color: Int): Int {
        val pixels = IntArray(bitmap.width * bitmap.height)
        bitmap.getPixels(pixels, 0, bitmap.width, 0, 0, bitmap.width, bitmap.height)
        return pixels.count { it == color }
    }
    fun run(context: Context): String {
        check(Looper.myLooper() == Looper.getMainLooper())
        val engine = TextEngine(context) {}
        val scale = context.resources.displayMetrics.density
        val bitmap = Bitmap.createBitmap(ceil(300 * scale).toInt(), ceil(100 * scale).toInt(), Bitmap.Config.ARGB_8888)
        try {
            val request = request(10, listOf("MMMM ", "MMMM"))
            val before = metrics(engine, request)
            fun draw() {
                bitmap.eraseColor(Color.WHITE)
                engine.draw(10, Canvas(bitmap), ceil(280 * scale).toInt(), Color.BLACK)
            }
            engine.setParagraphPaint(10, rows(Color.RED, Color.BLUE))
            draw()
            check(count(bitmap, Color.RED) > 20 && count(bitmap, Color.BLUE) > 20) { "run foreground spans missing" }
            check(count(bitmap, Color.GREEN) > 100) { "inline background span missing" }
            check(metrics(engine, request) == before) { "paint changed paragraph metrics" }
            check(engine.paragraphText(10) == "MMMM MMMM") { "accessibility text differs from shaped text" }
            val underlined = count(bitmap, Color.RED)
            engine.setParagraphPaint(10, rows(Color.RED, Color.BLUE, underline = false))
            draw()
            check(underlined > count(bitmap, Color.RED) + 5) { "platform underline span missing" }
            engine.setParagraphPaint(10, rows(Color.CYAN, Color.MAGENTA))
            draw()
            check(count(bitmap, Color.RED) == 0 && count(bitmap, Color.BLUE) == 0) { "old run paint retained" }
            check(count(bitmap, Color.CYAN) > 20 && count(bitmap, Color.MAGENTA) > 20) { "new run paint missing" }
            engine.clearParagraphPaint(10)
            draw()
            check(count(bitmap, Color.BLACK) > 20 && count(bitmap, Color.GREEN) == 0) { "empty paragraph retained inline paint" }
            check(metrics(engine, request) == before)
            val stock = metrics(engine, request(20, listOf("iiii"), family = 5))
            val catalog = ByteBuffer.allocateDirect(128).order(ByteOrder.LITTLE_ENDIAN).apply {
                putInt(1); putInt(1); putInt(24); putInt(1); putInt(0)
                utf8("ui-monospace"); utf8(""); flip()
            }
            engine.fonts(catalog)
            val authored = metrics(engine, request(21, listOf("iiii"), family = 24))
            check(authored == stock) { "authored monospace differs from stock platform family" }
            return "InlineTextPaintTest: PASS (platform glyph paint, background, replacement/reset, metric preservation, generic font catalog)"
        } finally { engine.close(); bitmap.recycle() }
    }
}
