package com.exact.views

import android.content.Context
import android.content.res.ColorStateList
import android.graphics.Typeface
import android.graphics.drawable.GradientDrawable
import android.graphics.drawable.RippleDrawable
import android.os.Build
import android.text.Editable
import android.text.TextWatcher
import android.util.TypedValue
import android.view.Gravity
import android.view.View
import android.view.WindowInsets
import android.widget.Button
import android.widget.EditText
import android.widget.FrameLayout
import android.widget.LinearLayout
import android.widget.ScrollView
import android.widget.TextView
import kotlin.math.roundToInt

/** The same eager scene, implemented only with Android's platform widgets. */
internal class ViewsScene(context: Context, initialRows: Int = 100, initialHeavy: Boolean = false) : LinearLayout(context) {
    private val scale = resources.displayMetrics.density
    private val ink = 0xff1a1c1e.toInt()
    private val blue = 0xff0061a4.toInt()
    private val paleBlue = ColorStateList.valueOf(0xffd1e4ff.toInt())
    private val white = ColorStateList.valueOf(0xffffffff.toInt())
    private val normalFace = Typeface.create(Typeface.DEFAULT, 400, false)
    private val buttonFace = Typeface.create(Typeface.DEFAULT, 600, false)
    private val boldFace = Typeface.create(Typeface.DEFAULT, 700, false)
    private val rowViews = ArrayList<View>()
    private val backgrounds = ArrayList<GradientDrawable>()
    private var rows = LinearLayout(context)
    private lateinit var scroll: ScrollView
    private val viewportWindowPosition = IntArray(2)
    private val contentWindowPosition = IntArray(2)
    private val counter = label("Count: 0", 20)
    private val echo = label("You typed: ", 16, lineHeight = 22)
    private val paintButton = button("", "toggle-batch", "Change all row backgrounds",
        0xffd1e4ff.toInt(), 0xff001d36.toInt(), 12, 20, ::toggleBackground)
    private var count = 0
    private var highlighted = false
    private var moved = false
    private var insetsApplied = false
    private var heavy = false
    var revision = 0L
        private set
    val appliedRevision: Long get() = revision
    val retainedRows: Int get() = rowViews.size
    val startupReady: Boolean get() = insetsApplied && !isLayoutRequested
    val scrollPositionPx: Int get() = scroll.scrollY
    val renderedScrollPositionPx: Int get() {
        scroll.getLocationInWindow(viewportWindowPosition)
        rows.getLocationInWindow(contentWindowPosition)
        return viewportWindowPosition[1] + scroll.paddingTop - contentWindowPosition[1]
    }
    val scrollViewportPx: Int get() = scroll.height - scroll.paddingTop - scroll.paddingBottom
    val scrollRangePx: Int get() = (rows.height - scrollViewportPx).coerceAtLeast(0)

    init {
        require(initialRows == 1 || initialRows == 100 || initialRows == 1000)
        orientation = VERTICAL
        tag = "core-root"
        setBackgroundColor(0xfff8f9ff.toInt())
        if (initialHeavy) heavyList() else buildSimple(initialRows)
    }

    private fun buildSimple(initialRows: Int) {
        removeAllViews()
        for (view in listOf(counter, echo, paintButton)) (view.parent as? android.view.ViewGroup)?.removeView(view)
        rowViews.clear(); backgrounds.clear()
        rows = LinearLayout(context)
        val header = LinearLayout(context).apply {
            orientation = VERTICAL
            setPadding(dp(20), dp(20), dp(20), dp(20))
        }
        fun headerChild(view: View, width: Int = LayoutParams.MATCH_PARENT) {
            header.addView(view, LayoutParams(width, LayoutParams.WRAP_CONTENT).apply {
                if (header.childCount > 0) topMargin = dp(14)
            })
        }
        headerChild(label("Exact on Android", 28, face = boldFace).apply {
            tag = "heavy-list"; contentDescription = "Open heavy list"; setOnClickListener { heavyList() }
        })
        headerChild(label("One Contract, native text, retained Android views.", 16, 0xff44474e.toInt(), 22))
        val incrementRow = LinearLayout(context).apply { orientation = HORIZONTAL; gravity = Gravity.CENTER_VERTICAL }
        incrementRow.addView(button("Increment", "increment", "Increment counter", blue, 0xffffffff.toInt(), 12, 20, ::increment),
            LayoutParams(LayoutParams.WRAP_CONTENT, LayoutParams.WRAP_CONTENT))
        counter.tag = "counter"
        incrementRow.addView(counter, LayoutParams(LayoutParams.WRAP_CONTENT, LayoutParams.WRAP_CONTENT).apply { leftMargin = dp(12) })
        headerChild(incrementRow)
        val editor = EditText(context).apply {
            tag = "draft"
            contentDescription = "Message"
            hint = "Type a message"
            isSingleLine = true
            includeFontPadding = false
            minHeight = 0; minWidth = 0
            minimumHeight = 0; minimumWidth = 0
            typeface = normalFace
            setTextSize(TypedValue.COMPLEX_UNIT_PX, dpFloat(16))
            setTextColor(ink)
            setHintTextColor(0xff74777f.toInt())
            // Native content inset accounts for 12dp CSS padding and the 1dp border.
            setPadding(dp(13), dp(13), dp(13), dp(13))
            background = shape(white, 8).apply { setStroke(dp(1), 0xff74777f.toInt()) }
            addTextChangedListener(object : TextWatcher {
                override fun beforeTextChanged(s: CharSequence?, start: Int, count: Int, after: Int) {}
                override fun onTextChanged(s: CharSequence?, start: Int, before: Int, count: Int) {
                    echo.text = "You typed: " + s
                    revision++
                }
                override fun afterTextChanged(s: Editable?) {}
            })
        }
        headerChild(editor)
        echo.tag = "echo"
        headerChild(echo)
        headerChild(paintButton)
        val selectors = LinearLayout(context).apply { orientation = HORIZONTAL; gravity = Gravity.CENTER_VERTICAL }
        fun selector(text: String, id: String, weight: Float, action: () -> Unit) {
            selectors.addView(button(text, id, text, 0xffe1e2ec.toInt(), ink, 8, 8, action, normalFace),
                LayoutParams(0, LayoutParams.WRAP_CONTENT, weight).apply {
                    if (selectors.childCount > 0) leftMargin = dp(8)
                })
        }
        selector("1 row", "rows-1", 53f) { setRows(1) }
        selector("100 rows", "rows-100", 56f) { setRows(100) }
        selector("1,000 rows", "rows-1000", 73f) { setRows(1000) }
        selector("Move rows", "toggle-move", 74f, ::toggleMove)
        headerChild(selectors)
        addView(header, LayoutParams(LayoutParams.MATCH_PARENT, LayoutParams.WRAP_CONTENT))
        rows.orientation = VERTICAL
        rows.tag = "rows-container"
        rows.setPadding(dp(20), 0, dp(20), dp(20))
        scroll = ScrollView(context).apply {
            tag = "core-scroll"
            isFillViewport = false
            clipToPadding = false
            addView(rows, FrameLayout.LayoutParams(LayoutParams.MATCH_PARENT, LayoutParams.WRAP_CONTENT))
        }
        addView(scroll, LayoutParams(LayoutParams.MATCH_PARENT, 0, 1f))
        setRows(initialRows)
    }

    private fun dp(value: Int) = dpFloat(value).roundToInt()
    private fun dpFloat(value: Int) = value * scale
    private fun label(value: String, size: Int, color: Int = ink, lineHeight: Int = 0,
        face: Typeface = normalFace): TextView = TextView(context).apply {
        text = value
        typeface = face
        includeFontPadding = false
        setTextSize(TypedValue.COMPLEX_UNIT_PX, dpFloat(size))
        setTextColor(color)
        if (lineHeight != 0) setLineSpacing(dpFloat(lineHeight) - paint.fontSpacing, 1f)
    }
    private fun shape(color: ColorStateList, radius: Int) = GradientDrawable().apply {
        shape = GradientDrawable.RECTANGLE
        cornerRadius = dpFloat(radius)
        setColor(color)
    }
    private fun button(value: String, id: String, description: String, fill: Int, color: Int,
        padding: Int, radius: Int, action: () -> Unit, face: Typeface = buttonFace): Button = Button(context).apply {
        tag = id
        contentDescription = description
        text = value
        isAllCaps = false
        includeFontPadding = false
        letterSpacing = 0f
        gravity = Gravity.START or Gravity.CENTER_VERTICAL
        stateListAnimator = null
        elevation = 0f
        typeface = face
        minWidth = 0; minHeight = 0; minimumWidth = 0; minimumHeight = 0
        setTextSize(TypedValue.COMPLEX_UNIT_PX, dpFloat(16))
        setTextColor(color)
        setPadding(dp(padding), dp(padding), dp(padding), dp(padding))
        background = RippleDrawable(ColorStateList.valueOf(0x1f000000), shape(ColorStateList.valueOf(fill), radius), null)
        setOnClickListener { action() }
    }

    fun increment() { count++; counter.text = "Count: " + count; revision++ }
    fun toggleBackground() {
        highlighted = !highlighted
        val color = if (highlighted) paleBlue else white
        // Immutable color lists are reused; updating paint never recreates row widgets.
        for (background in backgrounds) background.setColor(color)
        revision++
    }
    fun toggleMove() { moved = !moved; rows.translationX = if (moved) dpFloat(12) else 0f; revision++ }
    fun setRows(value: Int) {
        require(value == 1 || value == 100 || value == 1000)
        if (value == rowViews.size) return
        while (rowViews.size > value) {
            rows.removeViewAt(rowViews.lastIndex)
            rowViews.removeAt(rowViews.lastIndex)
            backgrounds.removeAt(backgrounds.lastIndex)
        }
        while (rowViews.size < value) {
            val index = rowViews.size
            val background = shape(if (highlighted) paleBlue else white, 8)
            val row = label("Native row " + (index + 1), 16).apply {
                tag = "row-" + index
                setPadding(dp(12), dp(12), dp(12), dp(12))
                this.background = background
            }
            rows.addView(row, LayoutParams(LayoutParams.MATCH_PARENT, LayoutParams.WRAP_CONTENT).apply {
                if (index > 0) topMargin = dp(6)
            })
            rowViews.add(row)
            backgrounds.add(background)
        }
        paintButton.text = "Change " + value + " row backgrounds"
        revision++
    }
    fun heavyList() {
        if (heavy) return
        heavy = true; highlighted = false; moved = false
        removeAllViews(); rowViews.clear(); backgrounds.clear()
        val header = LinearLayout(context).apply {
            orientation = HORIZONTAL; gravity = Gravity.CENTER_VERTICAL
            setPadding(dp(16), dp(16), dp(16), dp(16))
        }
        header.addView(label("Heavy list · 1,000", 18, lineHeight = 22, face = boldFace),
            LayoutParams(0, LayoutParams.WRAP_CONTENT, 1f))
        header.addView(label("Simple", 14, lineHeight = 20).apply {
            tag = "simple-list"; setPadding(dp(8), dp(6), dp(8), dp(6))
            background = shape(ColorStateList.valueOf(0xffe1e2ec.toInt()), 8)
            setOnClickListener { heavy = false; buildSimple(1000); revision++ }
        }, LayoutParams(LayoutParams.WRAP_CONTENT, LayoutParams.WRAP_CONTENT))
        addView(header, LayoutParams(LayoutParams.MATCH_PARENT, dp(64)))
        rows = LinearLayout(context).apply {
            orientation = VERTICAL; tag = "rows-container"
            setPadding(dp(20), 0, dp(20), dp(20))
        }
        for (index in 0 until 1000) {
            val card = heavyCard(index)
            rows.addView(card, LayoutParams(LayoutParams.MATCH_PARENT, dp(200)).apply {
                if (index > 0) topMargin = dp(8)
            })
            rowViews.add(card)
        }
        scroll = ScrollView(context).apply {
            tag = "core-scroll"; isFillViewport = false; clipToPadding = false
            addView(rows, FrameLayout.LayoutParams(LayoutParams.MATCH_PARENT, LayoutParams.WRAP_CONTENT))
        }
        addView(scroll, LayoutParams(LayoutParams.MATCH_PARENT, 0, 1f))
        revision++
    }
    private fun heavyCard(index: Int): View {
        val card = LinearLayout(context).apply {
            orientation = VERTICAL; tag = "row-$index"
            setPadding(dp(13), dp(13), dp(13), dp(13))
            background = shape(white, 12).apply { setStroke(dp(1), 0xffd9dce3.toInt()) }
            clipToOutline = true
        }
        fun child(view: View, height: Int) {
            card.addView(view, LayoutParams(LayoutParams.MATCH_PARENT, dp(height)).apply {
                if (card.childCount > 0) topMargin = dp(8)
            })
        }
        val header = LinearLayout(context).apply { orientation = HORIZONTAL; gravity = Gravity.CENTER_VERTICAL }
        header.addView(label("EX", 22, lineHeight = 40).apply {
            gravity = Gravity.CENTER; background = shape(paleBlue, 20)
        }, LayoutParams(dp(40), dp(40)))
        val author = LinearLayout(context).apply { orientation = VERTICAL }
        author.addView(label("Heavy row ${index + 1}", 16, lineHeight = 18, face = boldFace),
            LayoutParams(LayoutParams.MATCH_PARENT, LayoutParams.WRAP_CONTENT))
        author.addView(label("Native rendering / retained content", 12, 0xff44474e.toInt(), 16),
            LayoutParams(LayoutParams.MATCH_PARENT, LayoutParams.WRAP_CONTENT).apply { topMargin = dp(2) })
        header.addView(author, LayoutParams(0, LayoutParams.WRAP_CONTENT, 1f).apply { leftMargin = dp(10) })
        child(header, 40)
        val body = FrameLayout(context).apply {
            clipChildren = true
            addView(label("Card ${index + 1}: Retained content with a detailed status update. This card wraps text across several lines while the long list moves.",
                14, lineHeight = 20), FrameLayout.LayoutParams(LayoutParams.MATCH_PARENT, LayoutParams.WRAP_CONTENT))
        }
        child(body, 60)
        val metadata = LinearLayout(context).apply { orientation = HORIZONTAL; gravity = Gravity.CENTER_VERTICAL }
        metadata.addView(label("- 128 / 32 + 256", 12, 0xff44474e.toInt(), 18),
            LayoutParams(0, LayoutParams.WRAP_CONTENT, 1f))
        metadata.addView(label("12:34", 12, 0xff44474e.toInt(), 18), LayoutParams(LayoutParams.WRAP_CONTENT, LayoutParams.WRAP_CONTENT))
        child(metadata, 18)
        val badges = LinearLayout(context).apply { orientation = HORIZONTAL; gravity = Gravity.CENTER_VERTICAL }
        badges.addView(label("Android", 12, lineHeight = 16).apply {
            setPadding(dp(8), dp(6), dp(8), dp(6)); background = shape(ColorStateList.valueOf(0xffe1e2ec.toInt()), 6)
        }, LayoutParams(LayoutParams.WRAP_CONTENT, LayoutParams.WRAP_CONTENT))
        badges.addView(label("Row ${index + 1}", 12, 0xff001d36.toInt(), 16).apply {
            setPadding(dp(8), dp(6), dp(8), dp(6)); background = shape(paleBlue, 6)
        }, LayoutParams(LayoutParams.WRAP_CONTENT, LayoutParams.WRAP_CONTENT).apply { leftMargin = dp(8) })
        child(badges, 28)
        return card
    }
    fun prepare(workload: String) {
        if (workload == "heavy-scroll-1000") heavyList()
        if (workload == "scroll-1000" || workload == "heavy-scroll-1000") {
            if (highlighted) toggleBackground()
            if (moved) toggleMove()
            scroll.scrollTo(0, 0)
        }
        setRows(when (workload) {
            "counter", "paint-1" -> 1
            "paint-100" -> 100
            "paint-1000", "transform-1000", "scroll-1000", "heavy-scroll-1000" -> 1000
            else -> error("Unknown workload " + workload)
        })
    }
    // The shared harness supplies one absolute pixel offset per display frame.
    fun scrollTo(offsetPx: Int) { scroll.scrollTo(0, offsetPx) }
    fun performAction(workload: String) = when (workload) {
        "counter" -> increment()
        "paint-1", "paint-100", "paint-1000" -> toggleBackground()
        "transform-1000" -> toggleMove()
        else -> error("Unknown workload " + workload)
    }
    @Suppress("DEPRECATION")
    override fun onApplyWindowInsets(insets: WindowInsets): WindowInsets {
        if (Build.VERSION.SDK_INT >= 30) {
            val bars = insets.getInsets(WindowInsets.Type.systemBars() or WindowInsets.Type.displayCutout())
            val bottom = maxOf(bars.bottom, insets.getInsets(WindowInsets.Type.ime()).bottom)
            if (paddingLeft != bars.left || paddingTop != bars.top || paddingRight != bars.right || paddingBottom != bottom)
                setPadding(bars.left, bars.top, bars.right, bottom)
        } else {
            val cutout = insets.displayCutout
            val left = maxOf(insets.stableInsetLeft, cutout?.safeInsetLeft ?: 0)
            val top = maxOf(insets.stableInsetTop, cutout?.safeInsetTop ?: 0)
            val right = maxOf(insets.stableInsetRight, cutout?.safeInsetRight ?: 0)
            val bottom = maxOf(insets.stableInsetBottom, insets.systemWindowInsetBottom)
            if (paddingLeft != left || paddingTop != top || paddingRight != right || paddingBottom != bottom)
                setPadding(left, top, right, bottom)
        }
        insetsApplied = true
        return if (Build.VERSION.SDK_INT >= 30) WindowInsets.CONSUMED else insets.consumeSystemWindowInsets()
    }
    override fun onAttachedToWindow() { super.onAttachedToWindow(); requestApplyInsets() }
}
