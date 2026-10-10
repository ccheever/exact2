package com.exact.android

import android.content.Context
import android.graphics.Canvas as AndroidCanvas
import android.graphics.Typeface
import android.graphics.Paint
import android.text.TextPaint
import android.text.SpannableString
import android.text.style.MetricAffectingSpan
import androidx.compose.runtime.State
import kotlin.math.roundToInt
import android.os.Handler
import android.os.Looper
import android.util.SparseArray
import androidx.compose.ui.text.font.Font
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.font.createFontFamilyResolver
import androidx.compose.ui.unit.Density
import org.json.JSONArray
import java.nio.ByteBuffer
import java.nio.ByteOrder
import java.lang.ref.WeakReference
import kotlin.math.ceil

/** Public Android Layouts, owned by the imperative Views host. */
internal class TextEngine(private val context: Context, private val onWake: () -> Unit) {
    private val handler = Handler(Looper.getMainLooper())
    private val scale = context.resources.displayMetrics.density
    // CSS lengths are logical pixels. User font scaling belongs in the kernel's root font size.
    private val density = Density(scale, fontScale = 1f)
    private val resolver = createFontFamilyResolver(context)
    private val metrics = ByteBuffer.allocateDirect(12).order(ByteOrder.LITTLE_ENDIAN)
    private val families = HashMap<Int, FontFamily>()
    // Drawing visits every retained text node. Primitive keys avoid a boxed
    // Integer allocation for each lookup outside Integer.valueOf's cache.
    private val sources = SparseArray<Source>()
    private val paragraphs = object : LinkedHashMap<Key, NativeParagraph>(128, .75f, true) {
        override fun removeEldestEntry(eldest: MutableMap.MutableEntry<Key, NativeParagraph>): Boolean {
            if (size <= 512) return false
            eldest.key.source.forgetParagraphKey(eldest.key)
            return true
        }
    }
    // Exact content identity is separate from the view/incarnation owner. Weak
    // leases share only still-live shaping work; the index cannot keep native
    // NativeParagraph objects alive after source/offer retirement.
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
    private class SharedText(val key: TextKey, val intrinsic: NativeText) {
        // Answers remain valid for this exact public shaping lease even when a
        // transient NativeParagraph offer leaves the existing bounded layout LRU.
        // These slots hold only scalars, never a NativeParagraph or a Source owner.
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
        var paragraph: WeakReference<NativeParagraph>? = null
        var width = -1
        var ellipsis = false
        fun layout(width: Int, ellipsis: Boolean, create: () -> NativeParagraph): NativeParagraph {
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
        // NativeParagraph keys use this retained object's identity. Parsing a new
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
        var intrinsic: NativeText? = null
        var sharedText: SharedText? = null
        var drawn: NativeParagraph? = null
        var drawnWidth = -1
        var drawnEllipsis = false
        var richPaint: RichParagraph? = null
    }
    private data class Key(val source: Source, val width: Int, val ellipsis: Boolean)
    private class RichParagraph(val model: InlinePaintModel?, val decoration: Int, val dark: Boolean,
        val configuration: Long, val ranges: List<InlinePaintRange>)

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
        for (index in 0 until sources.size()) clearParagraphs(sources.valueAt(index))
        sources.clear()
    }
    fun close() {
        closed = true; clearSources(); paragraphs.clear(); sharedTexts.clear(); families.clear()
        inlinePaint.clear(); decorations.clear()
    }
    fun configurationChanged() {
        configurationVersion++
        sharedTexts.clear()
        for (index in 0 until sources.size()) sources.valueAt(index).let { it.richPaint = null }
    }
    fun remove(view: Int) {
        sources[view]?.let { source -> clearParagraphs(source) }
        sources.remove(view)
        inlinePaint.remove(view); decorations.remove(view)
    }

    /** Rust can measure the new row before its renewal reaches the presenter.
     * Keep those content-addressed metrics and authored inline data; only
     * owner-specific presentation/decoration must be made fresh here.
     */
    fun renew(view: Int) {
        decorations.remove(view)
        sources[view]?.let { it.richPaint = null }
    }

    /** Paint-only paragraph updates do not evict the kernel's metric answers. */
    fun setParagraphPaint(view: Int, runs: JSONArray) {
        val model = InlineTextPaint.read(view, runs).takeIf { it.pieces.isNotEmpty() }
        if (inlinePaint[view] == model) return
        if (model == null) inlinePaint.remove(view) else inlinePaint.put(view, model)
        sources[view]?.let { it.richPaint = null }
    }
    /** Accessibility reads the same collapsed string as platform measurement. */
    fun paragraphText(view: Int): String? = sources[view]?.runs?.joinToString("") { it.text }

    fun clearParagraphPaint(view: Int) {
        if (inlinePaint[view] == null) return
        inlinePaint.remove(view)
        sources[view]?.let { it.richPaint = null }
    }
    fun setTextDecoration(view: Int, value: String) {
        val decoration = InlinePaintModel.decoration(value)
        if ((decorations[view] ?: 0) == decoration) return
        if (decoration == 0) decorations.remove(view) else decorations.put(view, decoration)
        sources[view]?.let { it.richPaint = null }
    }
    /** The presenter invalidates text boxes/groups when the owner scheme changes. */
    fun setPaintDark(dark: Boolean) {
        if (paintDark == dark) return
        paintDark = dark
        for (index in 0 until sources.size()) sources.valueAt(index).let { it.richPaint = null }
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

    private fun Style.paint(fonts: MutableList<Pair<State<Any>, Any>>, base: Boolean = false): TextPaint {
        val resolved = resolver.resolve(family(family), FontWeight(weight.coerceIn(1, 1000)),
            if (italic) FontStyle.Italic else FontStyle.Normal)
        val face = resolved.value
        fonts.add(resolved to face)
        return TextPaint(Paint.ANTI_ALIAS_FLAG).apply {
            density = scale; textSize = size * scale; typeface = face as Typeface
            textLocales = context.resources.configuration.locales
            letterSpacing = if (textSize == 0f) 0f else spacing * scale / textSize
            fontFeatureSettings = if (!base && numeric and 1 != 0) "tnum" else null
        }
    }
    private class RunSpan(private val style: TextPaint) : MetricAffectingSpan() {
        override fun updateMeasureState(paint: TextPaint) {
            paint.textSize = style.textSize.roundToInt().toFloat(); paint.typeface = style.typeface
            paint.letterSpacing = if (paint.textSize == 0f) 0f else style.letterSpacing * style.textSize / paint.textSize
            paint.fontFeatureSettings = style.fontFeatureSettings
        }
        override fun updateDrawState(paint: TextPaint) = updateMeasureState(paint)
    }
    private fun Source.prepare(): NativeText {
        val fonts = ArrayList<Pair<State<Any>, Any>>()
        val text = SpannableString(runs.joinToString("") { it.text })
        var start = 0
        for (run in runs) {
            val end = start + run.text.length
            if (end > start && (run.style != strut || run.style.numeric and 1 != 0)) text.setSpan(RunSpan(run.style.paint(fonts)), start, end, 33)
            start = end
        }
        return NativeText(text, strut.paint(fonts, base = true), direction, align, strut.lineHeight?.times(scale),
            runs.any { it.style.spacing != 0f }) { fonts.any { it.first.value !== it.second } }
    }
    private fun intrinsics(source: Source): NativeText {
        val cached = source.intrinsic
        if (cached != null && !cached.hasStaleResolvedFonts) return cached
        if (cached != null) {
            source.richPaint = null
            source.drawn = null
            clearParagraphs(source)
        }
        val key = TextKey(source.strut, source.runs, source.align, source.clamp, source.wrap,
            source.whiteSpace, source.direction, density.density, density.fontScale, configurationVersion)
        val lookup = WeakTextKey(key)
        val shared = sharedTexts[lookup]?.get()?.takeIf { it.key == key && !it.intrinsic.hasStaleResolvedFonts } ?: run {
            val shaped = source.prepare()
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
    private fun normalizedWidth(source: Source, shaped: NativeText, width: Int, ellipsis: Boolean): Int? {
        if (ellipsis || source.align != 0 || source.direction != 0 || !source.wraps()) return null
        // A clamped NativeParagraph clips glyph ink to its own width when lines are
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
        shaped: NativeText = intrinsics(source)): NativeParagraph {
        val normalized = normalizedWidth(source, shaped, width, ellipsis)
        val used = normalized ?: width
        val cached = source.drawn
        if (cached != null && source.drawnWidth == used && source.drawnEllipsis == ellipsis) return cached
        val key = Key(source, used, ellipsis)
        val laidOut = paragraphs[key] ?: checkNotNull(source.sharedText).layout(used, ellipsis) {
            shaped.pure { NativeParagraph(shaped, used,
                if (ellipsis) 1 else if (source.clamp == 0) Int.MAX_VALUE else source.clamp, ellipsis) }
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

    private fun paintedParagraph(source: Source, width: Int, ellipsis: Boolean = false): NativeParagraph {
        val measured = intrinsics(source)
        val model = inlinePaint[source.view]
        val ownDecoration = decorations[source.view] ?: 0
        var rich = source.richPaint
        if (rich == null || rich.model !== model || rich.decoration != ownDecoration ||
            rich.dark != paintDark || rich.configuration != configurationVersion) {
            val painted = model?.ranges(source.whiteSpace, paintDark, ownDecoration)
            require(painted == null || painted.text == measured.text.toString()) {
                "Android inline paint text differs from measured paragraph"
            }
            val ranges = painted?.ranges ?: if (ownDecoration != 0 && measured.text.isNotEmpty())
                listOf(InlinePaintRange(0, measured.text.length, null, null, ownDecoration)) else emptyList()
            rich = RichParagraph(model, ownDecoration, paintDark, configurationVersion, ranges)
            source.richPaint = rich
        }
        val layout = drawnParagraph(source, width, ellipsis)
        measured.setPaint(rich.ranges)
        return layout
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
            val inkWidth = shaped.pure {
                var result = 0f
                repeat(laidOut.lineCount) { result = maxOf(result, laidOut.getLineWidth(it)) }
                result
            }
            Metrics(used, false, inkWidth / scale, laidOut.height / scale, laidOut.firstBaseline / scale)
                .also { shared.remember(it) }
        }
        metrics.clear()
        metrics.putFloat(answer.inkWidth).putFloat(answer.height).putFloat(answer.baseline)
        metrics.flip()
        return metrics
    }

    // The native View or flattened group owns the drawing commands. Keep
    // one platform display list rather than another RenderNode per paragraph.
    fun draw(view: Int, canvas: AndroidCanvas, width: Int, color: Int, ellipsis: Boolean = false) =
        drawImpl(view, canvas, width, color, ellipsis)

    private fun drawImpl(view: Int, canvas: AndroidCanvas, width: Int, color: Int, ellipsis: Boolean) {
        val source = sources[view] ?: return
        if (ellipsis && !source.wraps()) {
            val laidOut = paintedParagraph(source, width.coerceIn(0, 32767), true)
            laidOut.paint(canvas, color)
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
        laidOut.paint(canvas, color)
        canvas.restoreToCount(checkpoint)
    }
}
