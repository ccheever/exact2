package dev.exact.heavybench

import android.content.Context
import android.graphics.Canvas
import android.graphics.Color
import android.graphics.Outline
import android.graphics.Paint
import android.graphics.RectF
import android.graphics.Typeface
import android.graphics.drawable.GradientDrawable
import android.text.SpannableStringBuilder
import android.text.Spanned
import android.text.Layout
import android.text.TextUtils
import android.text.style.AbsoluteSizeSpan
import android.text.style.BackgroundColorSpan
import android.text.style.ForegroundColorSpan
import android.text.style.TypefaceSpan
import android.text.style.UnderlineSpan
import android.view.Gravity
import android.view.View
import android.view.ViewGroup
import android.view.ViewOutlineProvider
import android.widget.FrameLayout
import android.widget.ImageView
import android.widget.LinearLayout
import android.widget.TextView
import kotlin.math.roundToInt

private val regular = Typeface.create(Typeface.DEFAULT, 400, false)
private val semibold = Typeface.create(Typeface.DEFAULT, 600, false)
private val italic = Typeface.create(Typeface.DEFAULT, 400, true)
private val monospace = Typeface.create(Typeface.MONOSPACE, 400, false)
internal fun Context.dp(value: Float): Int = (value * resources.displayMetrics.density).roundToInt()
private fun Context.dp(value: Int): Int = dp(value.toFloat())
internal fun rounded(color: Int, radius: Float): GradientDrawable = GradientDrawable().apply { setColor(color); cornerRadius = radius }
private fun Context.label(size: Float, color: Int = Color.BLACK, bold: Boolean = false): TextView = TextView(this).apply {
    textSize = size; setTextColor(color); typeface = if (bold) semibold else regular
    includeFontPadding = false
    breakStrategy = Layout.BREAK_STRATEGY_SIMPLE; hyphenationFrequency = Layout.HYPHENATION_FREQUENCY_NONE
}

private fun paragraph(runs: List<TextRun>): CharSequence {
    val result = SpannableStringBuilder()
    for (run in runs) {
        val begin = result.length
        result.append(run.text)
        fun span(value: Any) { result.setSpan(value, begin, result.length, Spanned.SPAN_EXCLUSIVE_EXCLUSIVE) }
        when (run.style) {
            "bold" -> span(TypefaceSpan(semibold))
            "italic" -> span(TypefaceSpan(italic))
            "code" -> { span(TypefaceSpan(monospace)); span(AbsoluteSizeSpan(15, true)); span(BackgroundColorSpan(HeavyColors.fill)) }
            "link" -> { span(ForegroundColorSpan(HeavyColors.link)); span(UnderlineSpan()) }
            "mention" -> { span(TypefaceSpan(semibold)); span(ForegroundColorSpan(HeavyColors.link)) }
            "tag" -> span(ForegroundColorSpan(HeavyColors.tag))
        }
    }
    return result
}

private class BenchImage(context: Context, private val loader: HeavyImageLoader) : ImageView(context) {
    private var file: String? = null
    private var key: String? = null
    private var cancel: (() -> Unit)? = null
    init { scaleType = ScaleType.CENTER_CROP; setBackgroundColor(HeavyColors.hairline) }
    fun bind(value: String) {
        if (file == value) { request(); return }
        cancel?.invoke(); cancel = null; file = value; key = null; setImageDrawable(null); request()
    }
    private fun request() {
        val name = file ?: return
        if (width <= 0 || height <= 0 || !isAttachedToWindow) return
        val next = "$name|$width|$height"
        if (next == key) return
        cancel?.invoke(); key = next
        cancel = loader.request(name, width, height) { bitmap -> if (key == next && isAttachedToWindow) setImageBitmap(bitmap) }
    }
    override fun onSizeChanged(w: Int, h: Int, oldw: Int, oldh: Int) { super.onSizeChanged(w, h, oldw, oldh); request() }
    override fun onAttachedToWindow() { super.onAttachedToWindow(); request() }
    override fun onDetachedFromWindow() { cancel?.invoke(); cancel = null; key = null; super.onDetachedFromWindow() }
}

private class PhotoGrid(context: Context, photos: List<HeavyPhoto>, loader: HeavyImageLoader) : ViewGroup(context) {
    private val photos = photos.take(4)
    private val rectangles = ArrayList<RectF>()
    init {
        background = rounded(Color.WHITE, context.dp(12f).toFloat()); clipToOutline = true
        this.photos.forEach { addView(BenchImage(context, loader).apply { bind(it.src) }) }
    }
    override fun onMeasure(widthMeasureSpec: Int, heightMeasureSpec: Int) {
        val width = MeasureSpec.getSize(widthMeasureSpec).toFloat()
        val gap = context.dp(4f).toFloat()
        rectangles.clear()
        val height: Float
        when (photos.size) {
            1 -> { height = minOf(width * photos[0].height / photos[0].width, context.dp(320f).toFloat()); rectangles += RectF(0f, 0f, width, height) }
            2 -> { val side = (width - gap) / 2; height = side; rectangles += RectF(0f, 0f, side, side); rectangles += RectF(side + gap, 0f, width, side) }
            3 -> {
                val small = (width - gap) / 3; val big = width - gap - small; val short = (big - gap) / 2; height = big
                rectangles += RectF(0f, 0f, big, big); rectangles += RectF(big + gap, 0f, width, short); rectangles += RectF(big + gap, short + gap, width, big)
            }
            else -> {
                val side = (width - gap) / 2; height = side * 2 + gap
                rectangles += RectF(0f, 0f, side, side); rectangles += RectF(side + gap, 0f, width, side)
                rectangles += RectF(0f, side + gap, side, height); rectangles += RectF(side + gap, side + gap, width, height)
            }
        }
        for (i in 0 until childCount) {
            val rect = rectangles[i]
            getChildAt(i).measure(MeasureSpec.makeMeasureSpec(rect.width().roundToInt(), MeasureSpec.EXACTLY), MeasureSpec.makeMeasureSpec(rect.height().roundToInt(), MeasureSpec.EXACTLY))
        }
        setMeasuredDimension(width.roundToInt(), height.roundToInt())
    }
    override fun onLayout(changed: Boolean, l: Int, t: Int, r: Int, b: Int) {
        rectangles.forEachIndexed { i, rect -> getChildAt(i).layout(rect.left.roundToInt(), rect.top.roundToInt(), rect.right.roundToInt(), rect.bottom.roundToInt()) }
    }
}

private class ReactionFlow(context: Context) : ViewGroup(context) {
    private val positions = ArrayList<Pair<Int, Int>>()
    override fun onMeasure(widthMeasureSpec: Int, heightMeasureSpec: Int) {
        val width = MeasureSpec.getSize(widthMeasureSpec)
        val gap = context.dp(6)
        val height = context.dp(28)
        positions.clear()
        var x = 0; var y = 0
        for (i in 0 until childCount) {
            val child = getChildAt(i)
            child.measure(MeasureSpec.makeMeasureSpec(width, MeasureSpec.AT_MOST), MeasureSpec.makeMeasureSpec(height, MeasureSpec.EXACTLY))
            if (x > 0 && x + child.measuredWidth > width) { x = 0; y += height + gap }
            positions += x to y; x += child.measuredWidth + gap
        }
        setMeasuredDimension(width, if (childCount == 0) 0 else y + height)
    }
    override fun onLayout(changed: Boolean, l: Int, t: Int, r: Int, b: Int) {
        positions.forEachIndexed { i, p -> val child = getChildAt(i); child.layout(p.first, p.second, p.first + child.measuredWidth, p.second + child.measuredHeight) }
    }
}

private class LinkContainer(context: Context) : LinearLayout(context) {
    private val border = Paint(Paint.ANTI_ALIAS_FLAG).apply { color = HeavyColors.cardBorder; style = Paint.Style.STROKE; strokeWidth = resources.displayMetrics.density * .5f }
    init { orientation = VERTICAL; background = rounded(Color.WHITE, context.dp(12f).toFloat()); clipToOutline = true }
    override fun dispatchDraw(canvas: Canvas) {
        super.dispatchDraw(canvas)
        val inset = border.strokeWidth / 2
        canvas.drawRoundRect(inset, inset, width - inset, height - inset, context.dp(12f).toFloat(), context.dp(12f).toFloat(), border)
    }
}

/** Native reusable row. Each paragraph is one TextView with the full styled span flow. */
class MessageRowView(context: Context, private val store: HeavyStore, private val images: HeavyImageLoader) : FrameLayout(context) {
    private val avatar = BenchImage(context, images)
    private val body = LinearLayout(context).apply { orientation = LinearLayout.VERTICAL }
    private var message: HeavyMessage? = null
    private var timeLabel: TextView? = null
    private var reactionFlow: ReactionFlow? = null
    private var observation: (() -> Unit)? = null
    private val separator = Paint().apply { color = HeavyColors.hairline; strokeWidth = resources.displayMetrics.density * .5f }
    init {
        setBackgroundColor(Color.WHITE); setPadding(context.dp(16), context.dp(12), context.dp(16), context.dp(12))
        avatar.outlineProvider = object : ViewOutlineProvider() { override fun getOutline(view: View, outline: Outline) { outline.setOval(0, 0, view.width, view.height) } }
        avatar.clipToOutline = true
        addView(avatar, LayoutParams(context.dp(40), context.dp(40)))
        addView(body, LayoutParams(LayoutParams.MATCH_PARENT, LayoutParams.WRAP_CONTENT).apply { leftMargin = context.dp(52) })
    }
    fun bind(value: HeavyMessage, seconds: Int) {
        observation?.invoke(); observation = null; message = value
        avatar.bind(value.avatar); body.removeAllViews(); reactionFlow = null
        val header = LinearLayout(context).apply { orientation = LinearLayout.HORIZONTAL; isBaselineAligned = true; gravity = Gravity.START }
        header.addView(context.label(15f, bold = true).apply { text = value.author; maxLines = 1; ellipsize = TextUtils.TruncateAt.END })
        timeLabel = context.label(13f, HeavyColors.secondary).apply { maxLines = 1; ellipsize = TextUtils.TruncateAt.END }
        header.addView(timeLabel, LinearLayout.LayoutParams(LayoutParams.WRAP_CONTENT, LayoutParams.WRAP_CONTENT).apply { leftMargin = context.dp(6) })
        body.addView(header)
        value.quote?.let { quote ->
            val box = FrameLayout(context).apply { background = rounded(HeavyColors.fill, context.dp(8f).toFloat()); clipToOutline = true }
            val column = LinearLayout(context).apply { orientation = LinearLayout.VERTICAL; setPadding(context.dp(11), context.dp(8), context.dp(8), context.dp(8)) }
            column.addView(context.label(13f, HeavyColors.label2, true).apply { text = quote.author })
            column.addView(context.label(13f, HeavyColors.label2).apply { text = quote.excerpt; maxLines = 2; ellipsize = TextUtils.TruncateAt.END })
            box.addView(column, LayoutParams(LayoutParams.MATCH_PARENT, LayoutParams.WRAP_CONTENT))
            box.addView(View(context).apply { setBackgroundColor(HeavyColors.quoteBar) }, LayoutParams(context.dp(3), LayoutParams.MATCH_PARENT))
            addBody(box, 6)
        }
        value.paragraphs.forEachIndexed { i, runs -> addBody(context.label(16f).apply { text = paragraph(runs) }, if (i == 0) 6 else 8) }
        if (value.photos.isNotEmpty()) addBody(PhotoGrid(context, value.photos, images), 8)
        value.link?.let { link ->
            val card = LinkContainer(context)
            card.addView(BenchImage(context, images).apply { bind(link.thumb) }, LinearLayout.LayoutParams(LayoutParams.MATCH_PARENT, context.dp(140)))
            val column = LinearLayout(context).apply { orientation = LinearLayout.VERTICAL; setPadding(context.dp(10), context.dp(10), context.dp(10), context.dp(10)) }
            column.addView(context.label(12f, HeavyColors.secondary).apply { text = link.site.uppercase(java.util.Locale.ROOT) })
            column.addView(context.label(15f, bold = true).apply { text = link.title; maxLines = 2; ellipsize = TextUtils.TruncateAt.END })
            column.addView(context.label(13f, HeavyColors.label2).apply { text = link.description; maxLines = 2; ellipsize = TextUtils.TruncateAt.END })
            card.addView(column); addBody(card, 8)
        }
        if (value.reactions.isNotEmpty()) { reactionFlow = ReactionFlow(context); addBody(reactionFlow!!, 8); updateReactions() }
        updateTime(seconds)
        if (isAttachedToWindow) observe()
    }
    private fun addBody(view: View, top: Int) {
        body.addView(view, LinearLayout.LayoutParams(LayoutParams.MATCH_PARENT, LayoutParams.WRAP_CONTENT).apply { topMargin = context.dp(top) })
    }
    fun updateTime(seconds: Int) {
        val m = message ?: return
        val time = m.time(seconds)
        if (timeLabel?.text?.toString() != time) timeLabel?.text = time
        contentDescription = "${m.author}, $time"
    }
    private fun updateReactions() {
        val m = message ?: return
        val flow = reactionFlow ?: return
        flow.removeAllViews()
        for (reaction in m.reactions) {
            val count = store.count(m, reaction)
            val chip = LinearLayout(context).apply {
                orientation = LinearLayout.HORIZONTAL; gravity = Gravity.CENTER_VERTICAL
                setPadding(context.dp(10), 0, context.dp(10), 0)
                background = rounded(HeavyColors.fill, context.dp(14f).toFloat())
                isClickable = true; isFocusable = true; contentDescription = "${reaction.emoji} $count"
                setOnClickListener { store.bump(m, reaction) }
            }
            chip.addView(context.label(14f).apply { text = reaction.emoji })
            chip.addView(context.label(13f, HeavyColors.label2, true).apply { text = count.toString() },
                LinearLayout.LayoutParams(LayoutParams.WRAP_CONTENT, LayoutParams.WRAP_CONTENT).apply { leftMargin = context.dp(4) })
            flow.addView(chip)
        }
    }
    private fun observe() { observation?.invoke(); message?.let { observation = store.observe(it.id) { updateReactions() } } }
    override fun onAttachedToWindow() { super.onAttachedToWindow(); observe() }
    override fun onDetachedFromWindow() { observation?.invoke(); observation = null; super.onDetachedFromWindow() }
    override fun dispatchDraw(canvas: Canvas) {
        super.dispatchDraw(canvas)
        canvas.drawLine(context.dp(68f).toFloat(), height - separator.strokeWidth / 2, (width - context.dp(16f)).toFloat(), height - separator.strokeWidth / 2, separator)
    }
}
