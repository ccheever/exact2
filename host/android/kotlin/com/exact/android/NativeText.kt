/*
 * Copyright 2019 The Android Open Source Project
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at http://www.apache.org/licenses/LICENSE-2.0
 * Unless required by applicable law or agreed to in writing, software distributed
 * under the License is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR
 * CONDITIONS OF ANY KIND, either express or implied. See the License for the
 * specific language governing permissions and limitations under the License.
 * Intrinsic-width candidate selection and proportional trimmed line metrics are
 * adapted from AndroidX Compose ui-text 1.12.1 (LayoutIntrinsics/LineHeightStyleSpan).
 */
package com.exact.android

import android.graphics.Canvas
import android.graphics.Paint
import android.os.Build
import android.text.BoringLayout
import android.text.Layout
import android.text.SpannableString
import android.text.StaticLayout
import android.text.TextDirectionHeuristics
import android.text.TextPaint
import android.text.TextUtils
import android.text.style.BackgroundColorSpan
import android.text.style.CharacterStyle
import android.text.style.ForegroundColorSpan
import android.text.style.LineHeightSpan
import android.text.style.StrikethroughSpan
import android.text.style.UnderlineSpan
import java.text.BreakIterator
import java.util.PriorityQueue
import kotlin.math.abs
import kotlin.math.ceil

/** Metric spans and one mutable paint projection; no paint-specific layout tree. */
internal class NativeText(val text: SpannableString, val paint: TextPaint,
    val direction: Int, val align: Int, lineHeight: Float?, private val spacing: Boolean,
    private val stale: () -> Boolean) {
    val hasStaleResolvedFonts get() = stale()
    val textDirection get() = if (direction == 1) TextDirectionHeuristics.RTL else TextDirectionHeuristics.LTR
    val alignment get() = when (align) {
        1 -> Layout.Alignment.ALIGN_CENTER
        2 -> Layout.Alignment.ALIGN_OPPOSITE
        else -> Layout.Alignment.ALIGN_NORMAL
    }
    private var paintRanges: List<InlinePaintRange> = emptyList()
    private val unstyled: String?
    private var uniformColor: Int? = null
    val drawableText: CharSequence get() = if (paintSpans.isEmpty()) unstyled ?: text else text
    fun drawColor(inherited: Int) = uniformColor ?: inherited
    private data class PaintSpan(val value: CharacterStyle, val start: Int, val end: Int)
    private val paintSpans = ArrayList<PaintSpan>()
    init {
        lineHeight?.let { text.setSpan(TrimmedHeight(it, text.length), 0, text.length, 33) }
        unstyled = if (text.getSpans(0, text.length, Any::class.java).isEmpty()) text.toString() else null
    }
    fun setPaint(ranges: List<InlinePaintRange>) {
        if (paintRanges === ranges) return
        for (span in paintSpans) text.removeSpan(span.value)
        paintSpans.clear(); paintRanges = ranges; uniformColor = null
        if (ranges.isNotEmpty() && ranges.first().start == 0 && ranges.last().end == text.length &&
            ranges.zipWithNext().all { it.first.end == it.second.start } &&
            ranges.all { it.color == ranges[0].color && it.background == null && it.decoration == 0 }) {
            uniformColor = ranges[0].color
            return
        }
        fun add(value: CharacterStyle, range: InlinePaintRange) {
            text.setSpan(value, range.start, range.end, 33)
            paintSpans.add(PaintSpan(value, range.start, range.end))
        }
        for (range in ranges) {
            range.color?.let { add(ForegroundColorSpan(it), range) }
            range.background?.let { add(BackgroundColorSpan(it), range) }
            if (range.decoration and 1 != 0) add(UnderlineSpan(), range)
            if (range.decoration and 2 != 0) add(StrikethroughSpan(), range)
        }
    }
    fun <T> pure(block: () -> T): T {
        if (paintSpans.isEmpty()) return block()
        for (span in paintSpans) text.removeSpan(span.value)
        try { return block() }
        finally { for (span in paintSpans) text.setSpan(span.value, span.start, span.end, 33) }
    }
    val boring: BoringLayout.Metrics? by lazy {
        pure {
            if (Build.VERSION.SDK_INT >= 33) BoringLayout.isBoring(text, paint, textDirection, true, null)
            else if (!textDirection.isRtl(text, 0, text.length)) BoringLayout.isBoring(text, paint, null)
            else null
        }
    }
    val maxIntrinsicWidth: Float by lazy {
        pure { (boring?.width?.toFloat() ?: ceil(Layout.getDesiredWidth(text, paint))) +
            if (spacing && text.isNotEmpty()) .5f else 0f }
    }
    val minIntrinsicWidth: Float by lazy {
        pure {
            val breaks = BreakIterator.getLineInstance(paint.textLocale).apply { setText(this@NativeText.text.toString()) }
            val candidates = PriorityQueue<IntRange>(10, compareBy { it.last - it.first })
            var start = 0; var end = breaks.next()
            while (end != BreakIterator.DONE) {
                if (candidates.size < 10) candidates.add(start..end)
                else if (end - start > checkNotNull(candidates.peek()).let { it.last - it.first }) {
                    candidates.poll(); candidates.add(start..end)
                }
                start = end; end = breaks.next()
            }
            candidates.maxOfOrNull { Layout.getDesiredWidth(text, it.first, it.last, paint) } ?: 0f
        }
    }
    private class TrimmedHeight(private val height: Float, private val length: Int) : LineHeightSpan {
        private var originalAscent = Int.MIN_VALUE
        private var originalDescent = 0
        private var ascent = 0
        private var descent = 0
        override fun chooseHeight(text: CharSequence, start: Int, end: Int, top: Int, y: Int,
            metrics: Paint.FontMetricsInt) {
            val oldHeight = metrics.descent - metrics.ascent
            if (oldHeight <= 0 || start == 0 && end == length) return
            if (originalAscent == Int.MIN_VALUE) {
                originalAscent = metrics.ascent; originalDescent = metrics.descent
                val target = ceil(height).toInt(); val diff = target - oldHeight
                val ratio = abs(metrics.ascent.toFloat()) / oldHeight
                descent = metrics.descent + ceil(diff * if (diff <= 0) ratio else 1f - ratio).toInt()
                ascent = descent - target
            }
            metrics.ascent = if (start == 0) originalAscent else ascent
            metrics.descent = if (end == length) originalDescent else descent
        }
    }
}

internal class NativeParagraph(private val source: NativeText, width: Int, maxLines: Int, private val ellipsis: Boolean) {
    val width = width.toFloat()
    private val layout: Layout = source.boring?.takeIf { source.maxIntrinsicWidth <= width }?.let { metrics ->
        if (Build.VERSION.SDK_INT >= 33) BoringLayout(source.text, source.paint, width, source.alignment,
            1f, 0f, metrics, false, if (ellipsis) TextUtils.TruncateAt.END else null, width, true)
        else BoringLayout(source.text, source.paint, width, source.alignment, 1f, 0f, metrics, false,
            if (ellipsis) TextUtils.TruncateAt.END else null, width)
    } ?: StaticLayout.Builder.obtain(source.text, 0, source.text.length, source.paint, width)
        .setAlignment(source.alignment).setTextDirection(source.textDirection)
        .setIncludePad(false).setUseLineSpacingFromFallbacks(true)
        .setBreakStrategy(Layout.BREAK_STRATEGY_SIMPLE).setHyphenationFrequency(Layout.HYPHENATION_FREQUENCY_NONE)
        .setJustificationMode(if (source.align == 3) Layout.JUSTIFICATION_MODE_INTER_WORD else Layout.JUSTIFICATION_MODE_NONE)
        .setMaxLines(maxLines).setEllipsize(if (ellipsis) TextUtils.TruncateAt.END else null)
        .setEllipsizedWidth(width).build()
    val lineCount = minOf(layout.lineCount, maxLines)
    val height = layout.getLineBottom(lineCount - 1).toFloat()
    val firstBaseline = layout.getLineBaseline(0).toFloat()
    fun getLineWidth(line: Int) = layout.getLineWidth(line)
    private var drawableText: CharSequence = source.text
    fun paint(canvas: Canvas, color: Int) {
        source.paint.color = source.drawColor(color)
        val desired = source.drawableText
        if (layout is BoringLayout && drawableText !== desired) {
            // TextView's public reuse path preserves the measured geometry and
            // lets an unstyled String use BoringLayout's direct drawing path.
            val metrics = checkNotNull(source.boring)
            val end = if (ellipsis) TextUtils.TruncateAt.END else null
            if (Build.VERSION.SDK_INT >= 33) layout.replaceOrMake(desired, source.paint,
                width.toInt(), source.alignment, metrics, false, end, width.toInt(), true)
            else layout.replaceOrMake(desired, source.paint, width.toInt(), source.alignment,
                1f, 0f, metrics, false, end, width.toInt())
            drawableText = desired
        }
        val checkpoint = canvas.save()
        if (layout.lineCount > lineCount || layout.getEllipsisCount(lineCount - 1) > 0 ||
            layout.getLineEnd(lineCount - 1) < source.text.length) canvas.clipRect(0f, 0f, width, height)
        try { layout.draw(canvas) } finally { canvas.restoreToCount(checkpoint) }
    }
}
