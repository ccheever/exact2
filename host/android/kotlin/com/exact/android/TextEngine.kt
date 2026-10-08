package com.exact.android

import android.content.Context
import android.graphics.Canvas as AndroidCanvas
import android.graphics.RenderNode
import android.graphics.Typeface
import android.os.Handler
import android.os.Looper
import android.util.SparseArray
import androidx.compose.ui.graphics.CanvasHolder
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.AnnotatedString
import androidx.compose.ui.text.Paragraph
import androidx.compose.ui.text.ParagraphIntrinsics
import androidx.compose.ui.text.PlatformTextStyle
import androidx.compose.ui.text.SpanStyle
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.Font
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.font.createFontFamilyResolver
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextDirection
import androidx.compose.ui.text.style.TextDecoration
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.Constraints
import androidx.compose.ui.unit.Density
import androidx.compose.ui.unit.TextUnit
import androidx.compose.ui.unit.sp
import org.json.JSONArray
import java.nio.ByteBuffer
import java.nio.ByteOrder
import java.lang.ref.WeakReference
import kotlin.math.ceil

/** The public, imperative Compose paragraph API; no composition or second layout tree. */
internal class TextEngine(private val context: Context, private val onWake: () -> Unit) {
    private val handler = Handler(Looper.getMainLooper())
    private val scale = context.resources.displayMetrics.density
    // CSS lengths are logical pixels. User font scaling belongs in the kernel's root font size.
    private val density = Density(scale, fontScale = 1f)
    private val resolver = createFontFamilyResolver(context)
    private val metrics = ByteBuffer.allocateDirect(12).order(ByteOrder.LITTLE_ENDIAN)
    private val canvasHolder = CanvasHolder()
    private val families = HashMap<Int, FontFamily>()
    // Drawing visits every retained text node. Primitive keys avoid a boxed
    // Integer allocation for each lookup outside Integer.valueOf's cache.
    private val sources = SparseArray<Source>()
    private val paragraphs = object : LinkedHashMap<Key, Paragraph>(128, .75f, true) {
        override fun removeEldestEntry(eldest: MutableMap.MutableEntry<Key, Paragraph>): Boolean {
            if (size <= 512) return false
            eldest.key.source.forgetParagraphKey(eldest.key)
            return true
        }
    }
    // Exact content identity is separate from the view/incarnation owner. Weak
    // leases share only still-live shaping work; the index cannot keep native
    // Paragraph objects alive after source/offer retirement.
    private val sharedTexts = object : LinkedHashMap<WeakTextKey, WeakReference<SharedText>>(128, .75f, true) {
        override fun removeEldestEntry(eldest: MutableMap.MutableEntry<WeakTextKey, WeakReference<SharedText>>) = size > 512
    }
    private var closed = false
    private var configurationVersion = 0L
    private val inlinePaint = SparseArray<InlinePaintModel>()
    private val decorations = SparseArray<Int>()
    private var paintDark = false

    private data class Run(val text: String, val style: Style)
    private data class Style(
        val size: Float, val weight: Int, val family: Int, val italic: Boolean,
        val lineHeight: Float?, val spacing: Float, val numeric: Int
    )
    private data class TextKey(
        val strut: Style, val runs: List<Run>, val align: Int, val clamp: Int,
        val wrap: Int, val whiteSpace: Int, val direction: Int,
        val density: Float, val fontScale: Float, val configuration: Long
    )
    private class WeakTextKey(key: TextKey) : WeakReference<TextKey>(key) {
        private val hash = key.hashCode()
        override fun hashCode() = hash
        override fun equals(other: Any?): Boolean {
            if (this === other) return true
            val value = get() ?: return false
            return other is WeakTextKey && value == other.get()
        }
    }
    private class Metrics(val width: Int, val ellipsis: Boolean,
        val inkWidth: Float, val height: Float, val baseline: Float)
    private class SharedText(val key: TextKey, val intrinsic: ParagraphIntrinsics) {
        // Answers remain valid for this exact public shaping lease even when a
        // transient Paragraph offer leaves the existing bounded layout LRU.
        // These slots hold only scalars, never a Paragraph or a Source owner.
        private var answers: Array<Metrics?>? = null
        private var nextAnswer = 0
        fun answer(width: Int, ellipsis: Boolean): Metrics? {
            answers?.let { values ->
                for (value in values) {
                    if (value != null && value.width == width && value.ellipsis == ellipsis) return value
                }
            }
            return null
        }
        fun remember(answer: Metrics) {
            val values = answers ?: arrayOfNulls<Metrics>(8).also { answers = it }
            values[nextAnswer] = answer
            nextAnswer = (nextAnswer + 1) % values.size
        }
        // One weak offer costs no extra strong layout residency. Each source's
        // drawn lease and the existing 512-entry offer LRU still own layouts.
        var paragraph: WeakReference<Paragraph>? = null
        var width = -1
        var ellipsis = false
        fun layout(width: Int, ellipsis: Boolean, create: () -> Paragraph): Paragraph {
            if (this.width == width && this.ellipsis == ellipsis) paragraph?.get()?.let { return it }
            return create().also {
                paragraph = WeakReference(it)
                this.width = width
                this.ellipsis = ellipsis
            }
        }
    }
    private class Source(
        val view: Int, val index: Int, val generation: Int, val revision: Long,
        val strut: Style, val runs: List<Run>, val align: Int, val clamp: Int,
        val wrap: Int, val whiteSpace: Int, val direction: Int
    ) {
        // Paragraph keys use this retained object's identity. Parsing a new
        // offer compares its content explicitly, so a cache lookup does not
        // repeatedly hash every run/string in an unchanged paragraph.
        fun sameContent(other: Source) = view == other.view && index == other.index &&
            generation == other.generation && revision == other.revision && strut == other.strut &&
            runs == other.runs && align == other.align && clamp == other.clamp && wrap == other.wrap &&
            whiteSpace == other.whiteSpace && direction == other.direction
        var requestWords: LongArray? = null
        var requestLength = 0
        var paragraphKeys: ArrayList<Key>? = null
        fun rememberParagraphKey(key: Key) {
            val keys = paragraphKeys ?: ArrayList<Key>(3).also { paragraphKeys = it }
            keys.add(key)
        }
        fun forgetParagraphKey(key: Key) {
            paragraphKeys?.let { keys ->
                keys.remove(key)
                if (keys.isEmpty()) paragraphKeys = null
            }
        }
        // These derived objects are not part of content matching. Each live node
        // retains its shaped text and one painted width independently of the
        // bounded cache of transient layout offers.
        var intrinsic: ParagraphIntrinsics? = null
        var sharedText: SharedText? = null
        var drawn: Paragraph? = null
        var drawnWidth = -1
        var drawnEllipsis = false
        var richPaint: RichParagraph? = null
        // Cache the Compose paragraph's drawing commands, never a bitmap or a
        // second text layout. Background-only changes keep this display list.
        var paintNode: RenderNode? = null
        var recordedParagraph: Paragraph? = null
        var recordedColor = 0
        var recordedConfiguration = -1L
        fun discardPaint() {
            paintNode?.discardDisplayList()
            paintNode = null
            recordedParagraph = null
        }
    }
    private data class Key(val source: Source, val width: Int, val ellipsis: Boolean)
    private class RichParagraph(
        val model: InlinePaintModel?, val decoration: Int, val dark: Boolean,
        val configuration: Long, val intrinsic: ParagraphIntrinsics
    ) {
        var paragraph: Paragraph? = null
        var width = -1
        var ellipsis = false
    }

    private fun ByteBuffer.bodyWord(offset: Int, count: Int): Long {
        if (count == 8) return getLong(offset)
        var word = 0L
        repeat(count) { word = word or ((get(offset + it).toLong() and 255L) shl (it * 8)) }
        return word
    }
    private fun Source.matchesBody(input: ByteBuffer): Boolean {
        val words = requestWords ?: return false
        val start = input.position()
        if (input.remaining() != requestLength) return false
        for (index in words.indices) {
            val offset = index * 8
            if (words[index] != input.bodyWord(start + offset, minOf(8, requestLength - offset))) return false
        }
        return true
    }
    private fun Source.rememberBody(input: ByteBuffer, start: Int) {
        val length = input.limit() - start
        // Only small requests keep their exact validated bytes. Long rich text
        // falls back to parsing instead of doubling the retained text payload.
        if (length > 512) { requestWords = null; requestLength = 0; return }
        requestLength = length
        requestWords = LongArray((length + 7) / 8) { index ->
            val offset = index * 8
            input.bodyWord(start + offset, minOf(8, length - offset))
        }
    }

    /** Native executor calls this from a worker; all Rust calls remain on the main owner. */
    fun wake() { handler.post { if (!closed) onWake() } }

    private fun clearParagraphs(source: Source) {
        // A changed node owns only its active offers. Do not scan unrelated
        // paragraphs on every edit/removal; LRU eviction retires ownership too.
        source.paragraphKeys?.let { keys ->
            for (key in keys) paragraphs.remove(key)
            keys.clear()
        }
        source.paragraphKeys = null
    }
    private fun clearSources() {
        for (index in 0 until sources.size()) {
            val source = sources.valueAt(index)
            source.discardPaint()
            clearParagraphs(source)
        }
        sources.clear()
    }
    fun close() {
        closed = true; clearSources(); paragraphs.clear(); sharedTexts.clear(); families.clear()
        inlinePaint.clear(); decorations.clear()
    }
    fun configurationChanged() {
        configurationVersion++
        sharedTexts.clear()
        for (index in 0 until sources.size()) sources.valueAt(index).let { it.richPaint = null; it.discardPaint() }
    }
    fun remove(view: Int) {
        sources[view]?.let { source -> source.discardPaint(); clearParagraphs(source) }
        sources.remove(view)
        inlinePaint.remove(view); decorations.remove(view)
    }

    /** Paint-only paragraph updates do not evict the kernel's metric answers. */
    fun setParagraphPaint(view: Int, runs: JSONArray) {
        val model = InlineTextPaint.read(view, runs).takeIf { it.pieces.isNotEmpty() }
        if (inlinePaint[view] == model) return
        if (model == null) inlinePaint.remove(view) else inlinePaint.put(view, model)
        sources[view]?.let { it.richPaint = null; it.discardPaint() }
    }
    /** Accessibility reads the same collapsed string as platform measurement. */
    fun paragraphText(view: Int): String? = sources[view]?.runs?.joinToString("") { it.text }

    fun clearParagraphPaint(view: Int) {
        if (inlinePaint[view] == null) return
        inlinePaint.remove(view)
        sources[view]?.let { it.richPaint = null; it.discardPaint() }
    }
    fun setTextDecoration(view: Int, value: String) {
        val decoration = InlinePaintModel.decoration(value)
        if ((decorations[view] ?: 0) == decoration) return
        if (decoration == 0) decorations.remove(view) else decorations.put(view, decoration)
        sources[view]?.let { it.richPaint = null; it.discardPaint() }
    }
    /** The presenter invalidates text boxes/groups when the owner scheme changes. */
    fun setPaintDark(dark: Boolean) {
        if (paintDark == dark) return
        paintDark = dark
        for (index in 0 until sources.size()) sources.valueAt(index).let { it.richPaint = null; it.discardPaint() }
    }

    private fun ByteBuffer.utf8(): String {
        val count = int
        require(count in 0..remaining()) { "truncated text string" }
        val bytes = ByteArray(count)
        get(bytes)
        return bytes.toString(Charsets.UTF_8)
    }
    private fun ByteBuffer.style(): Style {
        val size = float; val weight = int; val family = int; val italic = int != 0
        val hasHeight = int != 0; val height = float; val spacing = float; val numeric = int
        require(size.isFinite() && size >= 0 && spacing.isFinite()) { "invalid text style" }
        return Style(size, weight, family, italic, if (hasHeight) height else null, spacing, numeric)
    }

    /** Called before layout; the plan's names and bytes are the font authority. */
    fun fonts(input: ByteBuffer) {
        input.order(ByteOrder.LITTLE_ENDIAN).position(0)
        require(input.int == 1)
        val count = input.int
        require(count in 0..65536)
        val faces = HashMap<Int, MutableList<Font>>()
        val generic = HashMap<Int, FontFamily>()
        repeat(count) {
            val stack = input.int; val weight = input.int; val italic = input.int != 0
            val alias = input.utf8()
            val source = input.utf8()
            if (source.isEmpty()) {
                require(weight == 1 && !italic && !faces.containsKey(stack)) { "invalid Android generic font member" }
                require(!generic.containsKey(stack)) { "Android ordered font fallback is not implemented" }
                generic[stack] = genericFamily(alias)
            } else {
                require(!generic.containsKey(stack)) { "Android ordered font fallback is not implemented" }
                faces.getOrPut(stack) { ArrayList() }.add(
                    Font(source, context.assets, FontWeight(weight), if (italic) FontStyle.Italic else FontStyle.Normal)
                )
            }
        }
        require(!input.hasRemaining())
        families.clear()
        families.putAll(generic)
        for ((stack, fonts) in faces) families[stack] = FontFamily(fonts)
        configurationVersion++
        clearSources(); paragraphs.clear(); sharedTexts.clear()
    }

    private fun genericFamily(name: String): FontFamily = when (name) {
        "system-ui", "ui-rounded" -> FontFamily.Default
        "ui-sans-serif", "sans-serif" -> FontFamily.SansSerif
        "ui-serif", "serif" -> FontFamily.Serif
        "ui-monospace", "monospace" -> FontFamily.Monospace
        "cursive" -> FontFamily.Cursive
        "fantasy" -> FontFamily(Typeface.create("fantasy", Typeface.NORMAL))
        else -> error("Android generic font '$name' is not implemented")
    }
    private fun family(id: Int): FontFamily = families[id] ?: when (id) {
        // PlanBuilder's eight stock stacks never enter the font catalog.
        0, 7 -> FontFamily.Default
        1, 2 -> FontFamily.SansSerif
        3, 4 -> FontFamily.Serif
        5, 6 -> FontFamily.Monospace
        else -> error("Android font stack $id was not installed")
    }

    private fun Style.span() = SpanStyle(
        fontSize = size.sp, fontWeight = FontWeight(weight.coerceIn(1, 1000)),
        fontFamily = family(family),
        fontStyle = if (italic) FontStyle.Italic else FontStyle.Normal,
        letterSpacing = spacing.sp,
        fontFeatureSettings = if (numeric and 1 != 0) "tnum" else null
    )
    private fun Source.text(): Pair<String, List<AnnotatedString.Range<SpanStyle>>> {
        // A uniform paragraph belongs in TextPaint. Adding an identical span
        // turns every plain label into a SpannableString and prevents the
        // platform BoringLayout from drawing its direct String path.
        if (runs.size == 1 && runs[0].style == strut && strut.numeric and 1 == 0) {
            return runs[0].text to emptyList()
        }
        val text = StringBuilder()
        val spans = ArrayList<AnnotatedString.Range<SpanStyle>>(runs.size)
        for (run in runs) {
            val start = text.length
            text.append(run.text)
            // Numeric features still use their explicit run span: the base
            // paragraph style does not carry them, including mixed runs.
            if (start != text.length && (run.style != strut || run.style.numeric and 1 != 0)) {
                spans.add(AnnotatedString.Range(run.style.span(), start, text.length))
            }
        }
        return text.toString() to spans
    }
    private fun Source.style() = TextStyle(
        fontSize = strut.size.sp, fontWeight = FontWeight(strut.weight.coerceIn(1, 1000)),
        fontFamily = family(strut.family),
        fontStyle = if (strut.italic) FontStyle.Italic else FontStyle.Normal,
        letterSpacing = strut.spacing.sp,
        lineHeight = strut.lineHeight?.sp ?: TextUnit.Unspecified,
        textAlign = when (align) { 1 -> TextAlign.Center; 2 -> TextAlign.End; 3 -> TextAlign.Justify; else -> TextAlign.Start },
        textDirection = if (direction == 1) TextDirection.Rtl else TextDirection.Ltr,
        platformStyle = PlatformTextStyle(includeFontPadding = false)
    )
    private fun intrinsics(source: Source): ParagraphIntrinsics {
        val cached = source.intrinsic
        if (cached != null && !cached.hasStaleResolvedFonts) return cached
        if (cached != null) {
            source.richPaint = null
            source.discardPaint()
            source.drawn = null
            clearParagraphs(source)
        }
        val key = TextKey(source.strut, source.runs, source.align, source.clamp, source.wrap,
            source.whiteSpace, source.direction, density.density, density.fontScale, configurationVersion)
        val lookup = WeakTextKey(key)
        val shared = sharedTexts[lookup]?.get()?.takeIf { it.key == key && !it.intrinsic.hasStaleResolvedFonts } ?: run {
            val (text, spans) = source.text()
            val shaped = ParagraphIntrinsics(
                text = text, style = source.style(), spanStyles = spans, placeholders = emptyList(),
                density = density, fontFamilyResolver = resolver
            )
            SharedText(key, shaped).also {
                // Replace the weak key too: a stale lease's key may die while
                // the new lease remains owned by a different source.
                sharedTexts.remove(lookup)
                sharedTexts[lookup] = WeakReference(it)
            }
        }
        source.sharedText = shared
        source.intrinsic = shared.intrinsic
        return shared.intrinsic
    }
    private fun Source.wraps() = whiteSpace != 2 && whiteSpace != 4
    private fun normalizedWidth(source: Source, shaped: ParagraphIntrinsics, width: Int, ellipsis: Boolean): Int? {
        if (ellipsis || source.align != 0 || source.direction != 0 || !source.wraps()) return null
        // A clamped Paragraph clips glyph ink to its own width when lines are
        // omitted, so even left-aligned text must retain the complete offer.
        if (source.clamp != 0) return null
        val intrinsic = shaped.maxIntrinsicWidth
        if (!intrinsic.isFinite() || intrinsic < 0f || intrinsic > 32767f) return null
        val unwrapped = ceil(intrinsic.toDouble()).toInt()
        // Left-aligned LTR text has the same ink, explicit lines and metrics at
        // every width that fits without soft breaks. The box retains its offer.
        return unwrapped.takeIf { width >= it }
    }
    private fun paragraph(source: Source, width: Int, ellipsis: Boolean = false, retain: Boolean = false,
        shaped: ParagraphIntrinsics = intrinsics(source)): Paragraph {
        val normalized = normalizedWidth(source, shaped, width, ellipsis)
        val used = normalized ?: width
        val cached = source.drawn
        if (cached != null && source.drawnWidth == used && source.drawnEllipsis == ellipsis) return cached
        val key = Key(source, used, ellipsis)
        val laidOut = paragraphs[key] ?: checkNotNull(source.sharedText).layout(used, ellipsis) {
            Paragraph(
                paragraphIntrinsics = shaped, constraints = Constraints(maxWidth = used),
                maxLines = if (ellipsis) 1 else if (source.clamp == 0) Int.MAX_VALUE else source.clamp,
                overflow = if (ellipsis) TextOverflow.Ellipsis else TextOverflow.Clip
            )
        }.also { source.rememberParagraphKey(key); paragraphs[key] = it }
        // One current paragraph per live source survives the bounded offer LRU.
        // Narrow intrinsic offers never displace a reusable wide paragraph.
        if (retain || normalized != null) {
            source.drawn = laidOut
            source.drawnWidth = used
            source.drawnEllipsis = ellipsis
        }
        return laidOut
    }
    private fun drawnParagraph(source: Source, width: Int, ellipsis: Boolean = false) =
        paragraph(source, width, ellipsis, retain = true)

    private fun decoration(flags: Int): TextDecoration = when (flags) {
        1 -> TextDecoration.Underline
        2 -> TextDecoration.LineThrough
        3 -> TextDecoration.combine(listOf(TextDecoration.Underline, TextDecoration.LineThrough))
        else -> TextDecoration.None
    }

    private fun paintedParagraph(source: Source, width: Int, ellipsis: Boolean = false): Paragraph {
        val model = inlinePaint[source.view]
        val ownDecoration = decorations[source.view] ?: 0
        if (model == null && ownDecoration == 0) return drawnParagraph(source, width, ellipsis)
        // Check font leases on the existing measurement path first. The paint
        // projection uses its exact string, metric spans and width offer.
        val measured = intrinsics(source)
        var rich = source.richPaint
        if (rich == null || rich.model !== model || rich.decoration != ownDecoration ||
            rich.dark != paintDark || rich.configuration != configurationVersion || rich.intrinsic.hasStaleResolvedFonts) {
            val (text, metricSpans) = source.text()
            val painted = model?.ranges(source.whiteSpace, paintDark, ownDecoration)
            require(painted == null || painted.text == text) { "Android inline paint text differs from measured paragraph" }
            val spans = ArrayList<AnnotatedString.Range<SpanStyle>>(metricSpans.size + (painted?.ranges?.size ?: 1))
            spans.addAll(metricSpans)
            if (painted == null) {
                if (text.isNotEmpty()) spans.add(AnnotatedString.Range(SpanStyle(textDecoration = decoration(ownDecoration)), 0, text.length))
            } else for (range in painted.ranges) {
                spans.add(AnnotatedString.Range(SpanStyle(
                    color = range.color?.let { Color(it) } ?: Color.Unspecified,
                    background = range.background?.let { Color(it) } ?: Color.Unspecified,
                    textDecoration = decoration(range.decoration)
                ), range.start, range.end))
            }
            rich = RichParagraph(model, ownDecoration, paintDark, configurationVersion, ParagraphIntrinsics(
                text = text, style = source.style(), spanStyles = spans, placeholders = emptyList(),
                density = density, fontFamilyResolver = resolver
            ))
            source.richPaint = rich
        }
        val used = normalizedWidth(source, measured, width, ellipsis) ?: width
        if (rich.paragraph == null || rich.width != used || rich.ellipsis != ellipsis) {
            rich.paragraph = Paragraph(
                paragraphIntrinsics = rich.intrinsic, constraints = Constraints(maxWidth = used),
                maxLines = if (ellipsis) 1 else if (source.clamp == 0) Int.MAX_VALUE else source.clamp,
                overflow = if (ellipsis) TextOverflow.Ellipsis else TextOverflow.Clip
            )
            rich.width = used; rich.ellipsis = ellipsis
        }
        return checkNotNull(rich.paragraph)
    }

    /** One callback per complete paragraph cache miss, never one per run or glyph. */
    fun measure(input: ByteBuffer, length: Int = input.limit()): ByteBuffer {
        check(Looper.myLooper() == Looper.getMainLooper())
        require(length in 0..input.capacity()) { "invalid text request length" }
        // JNI passes the initialized extent with this callback. Reset the
        // reused buffer here so native code need not call position and limit.
        input.clear()
        input.limit(length)
        input.order(ByteOrder.LITTLE_ENDIAN)
        require(input.int == 1)
        val view = input.int; val index = input.int; val generation = input.int; val revision = input.long
        val width = input.float; val height = input.float
        require(width.isFinite() && height.isFinite())
        val bodyStart = input.position()
        val previous = sources[view]
        val source = if (previous != null && previous.index == index && previous.generation == generation &&
            previous.revision == revision && previous.matchesBody(input)) {
            // The whole immutable body exactly matches one already validated.
            // Width/height remain fresh offers; font resolution is checked by
            // paragraph()/intrinsics() below even on this allocation-free path.
            input.position(input.limit())
            previous
        } else {
            val align = input.int; val clamp = input.int; val wrap = input.int; val whiteSpace = input.int
            val direction = input.int; val markup = input.int; val count = input.int; val exclusions = input.int
            require(markup == 0 && exclusions == 0) { "Android text flow/Markdown is not implemented" }
            require(count in 0..65536)
            require(wrap == 0 && whiteSpace in 0..4) { "Android overflow-wrap variants are not implemented" }
            val strut = input.style()
            val runs = ArrayList<Run>(count)
            repeat(count) { val style = input.style(); runs.add(Run(input.utf8(), style)) }
            require(runs.all { it.style.lineHeight == strut.lineHeight }) { "Android mixed inline line heights are not implemented" }
            require(!input.hasRemaining())
            val parsed = Source(view, index, generation, revision, strut, runs, align, clamp, wrap, whiteSpace, direction)
            val resolved = previous?.takeIf { it.sameContent(parsed) } ?: parsed.also {
                // Old revisions cannot satisfy a future offer. Do not keep their
                // shaped strings alive until unrelated layout offers evict them.
                if (previous != null) {
                    previous.discardPaint()
                    clearParagraphs(previous)
                }
            }
            resolved.rememberBody(input, bodyStart)
            resolved
        }
        if (source !== previous) sources.put(view, source)
        val offered = if (width < 0 || !source.wraps()) {
            val intrinsic = intrinsics(source)
            if (width == -2f && source.wraps()) intrinsic.minIntrinsicWidth else intrinsic.maxIntrinsicWidth
        } else width * scale
        val px = ceil(offered.toDouble()).toInt().coerceIn(0, 32767)
        // Keep the same stale-font check that paragraph() previously performed,
        // including the second check after an intrinsic-width probe. Width
        // normalization and metrics refer to this exact checked shaping lease.
        val shaped = intrinsics(source)
        val shared = checkNotNull(source.sharedText)
        val used = normalizedWidth(source, shaped, px, false) ?: px
        val answer = shared.answer(used, false) ?: run {
            val laidOut = paragraph(source, px, shaped = shaped)
            var inkWidth = 0f
            repeat(laidOut.lineCount) { inkWidth = maxOf(inkWidth, laidOut.getLineWidth(it)) }
            Metrics(used, false, inkWidth / scale, laidOut.height / scale, laidOut.firstBaseline / scale)
                .also { shared.remember(it) }
        }
        metrics.clear()
        metrics.putFloat(answer.inkWidth).putFloat(answer.height).putFloat(answer.baseline)
        metrics.flip()
        return metrics
    }

    // Compose paint mutates its intrinsic TextPaint. Metric leases stay shared;
    // rich paint leases belong to one source and never enter the metric index.
    // Every main-owned draw passes the owner's current default color explicitly.
    private fun paint(source: Source, paragraph: Paragraph, canvas: AndroidCanvas, color: Int, cachePaint: Boolean = true) {
        if (!canvas.isHardwareAccelerated || !cachePaint) {
            canvasHolder.drawInto(canvas) { paragraph.paint(this, Color(color)) }
            return
        }
        val node = source.paintNode ?: RenderNode("Exact paragraph").apply {
            // CSS clipping belongs to the retained box; glyph ink can overflow
            // the paragraph's nominal bounds, just as on the direct draw path.
            setClipToBounds(false)
        }.also { source.paintNode = it }
        if (source.recordedParagraph !== paragraph || source.recordedColor != color ||
            source.recordedConfiguration != configurationVersion || !node.hasDisplayList()) {
            node.discardDisplayList()
            source.recordedParagraph = null
            val width = ceil(paragraph.width.toDouble()).toInt().coerceAtLeast(1)
            val height = ceil(paragraph.height.toDouble()).toInt().coerceAtLeast(1)
            node.setPosition(0, 0, width, height)
            val recording = node.beginRecording(width, height)
            try {
                canvasHolder.drawInto(recording) { paragraph.paint(this, Color(color)) }
            } finally { node.endRecording() }
            source.recordedParagraph = paragraph
            source.recordedColor = color
            source.recordedConfiguration = configurationVersion
        }
        canvas.drawRenderNode(node)
    }

    /** Promotion/demotion changes command ownership, not paragraph metrics. */
    fun discardRecordedPaint(view: Int) { sources[view]?.discardPaint() }

    /** Existing native View path retains each paragraph's drawing commands. */
    fun draw(view: Int, canvas: AndroidCanvas, width: Int, color: Int, ellipsis: Boolean = false) =
        drawImpl(view, canvas, width, color, ellipsis, cachePaint = true)

    /** A group display list retains these commands; do not add one per source. */
    fun drawDirect(view: Int, canvas: AndroidCanvas, width: Int, color: Int, ellipsis: Boolean = false) =
        drawImpl(view, canvas, width, color, ellipsis, cachePaint = false)

    private fun drawImpl(view: Int, canvas: AndroidCanvas, width: Int, color: Int, ellipsis: Boolean, cachePaint: Boolean) {
        val source = sources[view] ?: return
        if (ellipsis && !source.wraps()) {
            val laidOut = paintedParagraph(source, width.coerceIn(0, 32767), true)
            paint(source, laidOut, canvas, color, cachePaint)
            return
        }
        val used = if (source.wraps()) width else ceil(intrinsics(source).maxIntrinsicWidth.toDouble()).toInt()
        val offset = if (source.wraps()) 0f else when (source.align) {
            1 -> (width - used) / 2f
            2 -> if (source.direction == 1) 0f else (width - used).toFloat()
            else -> if (source.direction == 1) (width - used).toFloat() else 0f
        }
        val checkpoint = canvas.save()
        canvas.translate(offset, 0f)
        val laidOut = paintedParagraph(source, used.coerceIn(0, 32767))
        paint(source, laidOut, canvas, color, cachePaint)
        canvas.restoreToCount(checkpoint)
    }
}
