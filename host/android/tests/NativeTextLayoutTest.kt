package com.exact.android

import android.graphics.Bitmap
import android.graphics.Canvas
import android.graphics.Color
import android.graphics.Paint
import android.os.Build
import android.text.SpannableString
import android.text.StaticLayout
import android.text.TextPaint
import android.text.style.LineHeightSpan

/** Single-line optimization keeps the SDK's fractional metrics and wrapped line height. */
internal object NativeTextLayoutTest {
    fun run(): String {
        val before = Bitmap.createBitmap(300, 240, Bitmap.Config.ARGB_8888)
        val after = Bitmap.createBitmap(300, 240, Bitmap.Config.ARGB_8888)
        try {
            for (size in listOf(0f, 33.75f, 39.375f)) {
                for (value in listOf("office affinity", "one two three four", "one\ntwo")) {
                    for (width in listOf(60, 300)) {
                        val text = SpannableString(value)
                        val paint = TextPaint(Paint.ANTI_ALIAS_FLAG).apply { textSize = size; color = Color.BLACK }
                        val source = NativeText(text, paint, 0, 0, 48f, false) { false }
                        val span = text.getSpans(0, text.length, LineHeightSpan::class.java).single()
                        source.maxIntrinsicWidth
                        check(text.getSpans(0, text.length, LineHeightSpan::class.java).single() === span)
                        if (Build.VERSION.SDK_INT >= 33 && '\n' !in value) check(source.boring != null)
                        val actual = NativeParagraph(source, width, Int.MAX_VALUE, false)
                        val expected = StaticLayout.Builder.obtain(text, 0, text.length, paint, width)
                            .setIncludePad(false).setUseLineSpacingFromFallbacks(true)
                            .setBreakStrategy(StaticLayout.BREAK_STRATEGY_SIMPLE)
                            .setHyphenationFrequency(StaticLayout.HYPHENATION_FREQUENCY_NONE).build()
                        check(actual.lineCount == expected.lineCount)
                        check(actual.height == expected.height.toFloat())
                        check(actual.firstBaseline == expected.getLineBaseline(0).toFloat())
                        for (line in 0 until actual.lineCount) {
                            check(actual.getLineWidth(line).toRawBits() == expected.getLineWidth(line).toRawBits())
                        }
                        before.eraseColor(Color.WHITE); after.eraseColor(Color.WHITE)
                        expected.draw(Canvas(before)); actual.paint(Canvas(after), Color.BLACK)
                        check(before.sameAs(after)) { "SDK text pixels changed: size=$size width=$width text=$value" }
                    }
                }
            }
            return "NativeTextLayoutTest: PASS (SDK metrics/pixels, fractional sizes, restored line-height span)"
        } finally { before.recycle(); after.recycle() }
    }
}
