package com.exact.android

import android.content.Context
import android.content.res.Configuration
import android.graphics.Canvas
import android.graphics.Color
import android.graphics.Matrix
import android.graphics.Path
import android.graphics.Rect
import android.graphics.drawable.GradientDrawable
import android.graphics.drawable.RippleDrawable
import android.text.Editable
import android.text.TextWatcher
import android.util.SparseArray
import android.view.View
import android.view.ViewGroup
import android.view.MotionEvent
import android.view.inputmethod.InputMethodManager
import android.view.accessibility.AccessibilityNodeInfo
import android.view.accessibility.AccessibilityNodeProvider
import android.widget.EditText
import android.widget.ImageView
import android.widget.ScrollView
import org.json.JSONArray
import org.json.JSONObject
import java.nio.ByteBuffer
import java.util.IdentityHashMap
import kotlin.math.roundToInt

/** Android Views retain their hardware RenderNodes; Rust is the sole layout owner. */
internal class Presenter(
    private val context: Context, private val text: TextEngine,
    private val dispatch: (Int, Int, String?) -> Unit,
    private val title: (String) -> Unit,
    private val intrinsic: (Int, Float, Float) -> Unit,
    private val viewportChanged: () -> Unit
) {
    private val scale = context.resources.displayMetrics.density
    val root = Box(context)
        .apply { isFocusableInTouchMode = true }
    private val nodes = SparseArray<Node>()
    private val dirty = HashSet<Node>()
    private val transformDirty = HashSet<Node>()
    private var layoutDirty = false
    private var closed = false
    private var authoredDark: Boolean? = null
    private var paintDark = context.resources.configuration.uiMode and Configuration.UI_MODE_NIGHT_MASK == Configuration.UI_MODE_NIGHT_YES
    private var firstRoot: Int? = null
    private var lastViewport = ""
    private var sharedStyles: IdentityHashMap<JSONObject, DecodedStyle>? = null
    // Logical ownership must not allocate an Android carrier.
    private val parentsDirty = HashSet<Node?>()
    private var reconcilePaths: Set<Node> = emptySet()
    private var rootChildren = IntArray(0)
    private val boxOwners = IdentityHashMap<Box, Node>()
    private val groupMembers = IdentityHashMap<Box, List<Node>>()
    private val flatParents = HashSet<Box>()
    private var semanticsGeometryDirty = false
    val coversWindow: Boolean get() = firstRoot?.let { nodes[it]?.props?.optString("viewportFit") } == "cover"
    val resizesForKeyboard: Boolean get() = firstRoot?.let { nodes[it]?.props?.optString("interactiveWidget") } == "resizes-content"
    val retainedNodes: Int get() = nodes.size()
    val materializedNodes: Int get() = (0 until nodes.size()).count { nodes.valueAt(it).materialized }
    data class Geometry(val id: Int, val kind: String, val frame: Rect, val attached: Boolean, val flat: Boolean, val materialized: Boolean)
    fun geometry(testId: String): Geometry? {
        for (index in 0 until nodes.size()) {
            val n = nodes.valueAt(index)
            if (n.props.optString("testId") == testId) return Geometry(n.key, n.kind, Rect(n.frame), n.logicalParent != null || n.rootAttached, n.flatParent != null, n.materialized)
        }
        return null
    }
    fun textGeometries(prefix: String): List<Geometry> = (0 until nodes.size()).map { nodes.valueAt(it) }
        .filter { it.kind == "text" && it.props.optString("testId").startsWith(prefix) }
        .map { Geometry(it.key, it.kind, Rect(it.frame), it.logicalParent != null || it.rootAttached, it.flatParent != null, it.materialized) }
    data class GroupInfo(val ids: List<Int>, val hasDisplayList: Boolean, val recordedLeafCount: Int, val recordingCount: Long, val usingHardwareNode: Boolean, val cachingText: Boolean)
    fun groupInfo(): List<GroupInfo> = flatParents.mapNotNull { parent -> parent.flatText?.let {
        GroupInfo(parent.logicalChildren.toList(), it.hasDisplayList, it.recordedLeafCount, it.recordingCount, it.usingHardwareNode, it.cachingText)
    } }
    fun accessibility(testId: String): Pair<AccessibilityNodeProvider, Int>? {
        for (index in 0 until nodes.size()) {
            val n = nodes.valueAt(index)
            if (n.kind == "text" && n.props.optString("testId") == testId) return n.flatParent?.flatText?.let { it to n.key }
        }
        return null
    }
    fun actionView(testId: String): View? {
        if (closed) return null
        for (index in 0 until nodes.size()) {
            val node = nodes.valueAt(index)
            if (node.props.optString("testId") == testId) return node.widget
        }
        return null
    }
    fun activate(testId: String): Boolean = actionView(testId)?.performClick() ?: false

    /** Pooled JSON is immutable; decode its paint geometry once per transaction. */
    private inner class DecodedStyle(style: JSONObject) {
        val background = colorPair(style, "background_color", Color.TRANSPARENT)
        val text = colorPair(style, "text_color", Color.BLACK)
        val borders = LongArray(4) { colorPair(style, "border_color_${BORDER_SIDES[it]}", Color.BLACK) }
        val widths = FloatArray(4) { number(style, "border_width_${BORDER_SIDES[it]}") * scale }
        val radii = FloatArray(8).apply {
            for (corner in 0..3) {
                val radius = number(style, "border_radius_${BORDER_CORNERS[corner]}") * scale
                this[corner * 2] = radius; this[corner * 2 + 1] = radius
            }
        }
        val controlInset = FloatArray(4) { widths[CONTENT_TO_BORDER[it]] }
        val inset = FloatArray(4) { controlInset[it] + number(style, "padding_${CONTENT_SIDES[it]}") * scale }
        val ellipsis = style.optString("text_overflow") == "ellipsis"
        val visible = style.optString("display") != "none" && style.optString("visibility") != "hidden"
        val overflowX = style.optString("overflow_x", "visible")
        val overflowY = style.optString("overflow_y", "visible")
        val rounded = radii.any { it > 0f }
        val scrollBorder = BORDER_SIDES.any { number(style, "border_width_$it") != 0f }
        val unsupported = UNSUPPORTED_STYLE.firstOrNull { style.has(it) }
        val decorated = style.optString("text_decoration_line", "none") != "none"
        val stacked = style.optInt("z_index", 0) != 0
        val pointerOverride = style.optString("pointer_events", "auto") != "auto"
        val editorFont = style.optInt("font_family", 0) == 0 && style.optInt("font_weight", 400) == 400 && style.optString("font_style", "normal") == "normal"
        val editorSpacing = number(style, "letter_spacing") == 0f && !style.has("line_height")
    }
    private fun decodeStyle(style: JSONObject): DecodedStyle {
        val pool = sharedStyles ?: IdentityHashMap<JSONObject, DecodedStyle>(STYLE_SLOTS).also { sharedStyles = it }
        pool[style]?.let { return it }
        val decoded = DecodedStyle(style)
        if (pool.size < STYLE_SLOTS) pool[style] = decoded
        return decoded
    }
    fun discardStylePool() { sharedStyles = null }

    private inner class Node(
        val key: Int, val kind: String,
        initialProps: JSONObject, initialStyle: JSONObject
    ) {
        var props = initialProps
        var style = initialStyle
            private set
        private var decodedStyle: DecodedStyle? = null
        private var styleDirty = true
        private var styleUnchecked = true
        private var eventsInstalled = false
        private var configured = false
        var handlers = emptySet<String>()
        var updating = false
        private var editorChanged = false
        val frame = Rect()
        private var imageSource: String? = null
        var tx = 0f; var ty = 0f; var sx = 1f; var angle = 0f
        var lx = 0f; var ly = 0f; var lw = 1f; var lh = 1f
        private var matrix: Matrix? = null
        private var transformed = false
        private var actualBox: Box? = null
        val materialized: Boolean get() = actualBox != null
        val box: Box get() = ensureBox()
        private val buttonFill = if (kind == "button") GradientDrawable() else null
        private val buttonMask = if (kind == "button") GradientDrawable().apply { setColor(Color.WHITE) } else null
        private var backgroundPair = colorPair(Color.TRANSPARENT, Color.TRANSPARENT)
        private var textPair = colorPair(Color.BLACK, Color.BLACK)
        private val borderPairs = LongArray(4) { colorPair(Color.BLACK, Color.BLACK) }
        private var textColor = Color.BLACK
        private var backgroundColor = Color.TRANSPARENT
        private val inset = FloatArray(4)
        private val controlInset = FloatArray(4)
        private var ellipsis = false
        private val borderWidths = FloatArray(4)
        private val borderColors = IntArray(4) { Color.BLACK }
        private val radii = FloatArray(8)
        private var visible = true
        private var clipContents = false
        private var overflowX = "visible"
        private var overflowY = "visible"
        var alpha = 1f
        var logicalParent: Node? = null
        var rootAttached = false
        var logicalChildren = IntArray(0)
        var childrenReconciled = false
        private var nativeRequested = false
        var flatParent: Box? = null
        private var leaf: FlatTextGroup.Leaf? = null
        private var flatOriginX = 0
        private var flatOriginY = 0
        private val scrollChildren = if (kind == "scroll") Box(context) else null
        val childrenBox: Box get() = scrollChildren ?: box
        val control: View? = when (kind) {
            "input", "textarea" -> EditText(context).apply {
                isSingleLine = kind == "input"
                includeFontPadding = false
                background = null
                setPadding(0, 0, 0, 0)
                addTextChangedListener(object : TextWatcher {
                    override fun beforeTextChanged(s: CharSequence?, start: Int, count: Int, after: Int) {}
                    override fun onTextChanged(s: CharSequence?, start: Int, before: Int, count: Int) {
                        if (!updating) {
                            editorChanged = true
                            if ("input" in handlers) dispatch(key, 23, s.toString())
                        }
                    }
                    override fun afterTextChanged(s: Editable?) {}
                })
                setOnFocusChangeListener { _, focused ->
                    if (!updating && !focused && editorChanged) {
                        editorChanged = false
                        if ("change" in handlers) dispatch(key, 1, this.text.toString())
                    }
                    if (!updating && (if (focused) "focus" else "blur") in handlers) dispatch(key, if (focused) 4 else 5, null)
                }
                setOnEditorActionListener { _, _, _ ->
                    if ("submit" in handlers) { dispatch(key, 7, null); true } else false
                }
            }
            "image" -> ImageView(context).apply { scaleType = ImageView.ScaleType.FIT_CENTER }
            "scroll" -> ScrollView(context).apply {
                isFillViewport = false
                clipToPadding = false
                addView(childrenBox)
                setOnScrollChangeListener { _, x, y, oldX, oldY ->
                    if ("scroll" in handlers && (x != oldX || y != oldY))
                        dispatch(key, 13, "${x / scale},${y / scale}")
                }
            }
            "text", "view", "button" -> null
            else -> error("Android node '$kind' is not implemented")
        }
        val widget: View get() = box
        init {
            scrollChildren?.let { boxOwners[it] = this }
            if (kind != "text" && kind != "view") ensureBox()
        }
        private fun ensureBox(): Box {
            check(!closed) { "Android presenter is closed" }
            // An embedder-requested View is a permanent carrier for this lifetime.
            // Release the containing group before constructing native descendants.
            nativeRequested = true
            flatParent?.let { materialize(ownerOf(it)) }
            actualBox?.let { return it }
            val box = Box(context)
            actualBox = box
            boxOwners[box] = this
            if (control != null) box.addView(control)
            if (kind == "text") box.paintText = { canvas ->
                val width = (box.width - inset[0] - inset[2]).coerceAtLeast(0f)
                val saved = canvas.save()
                canvas.translate(inset[0], inset[1])
                text.draw(key, canvas, width.roundToInt(), textColor, ellipsis)
                canvas.restoreToCount(saved)
            }
            box.id = View.generateViewId()
            box.accessibilityClass = if (kind == "button") "android.widget.Button" else if (kind == "text") "android.widget.TextView" else "android.view.View"
            if (kind == "button") {
                box.background = RippleDrawable(android.content.res.ColorStateList.valueOf(0x22000000), buttonFill, buttonMask)
                box.isClickable = true
            }
            applyGeometry(box)
            box.setFillColor(backgroundColor)
            if (configured && !updating) update()
            return box
        }
        private fun applyGeometry(box: Box) {
            borderWidths.copyInto(box.borderWidths)
            borderColors.copyInto(box.borderColors)
            radii.copyInto(box.radii)
            box.visibility = if (visible) View.VISIBLE else View.INVISIBLE
            box.alpha = alpha
            box.clipContents = clipContents
            box.geometryChanged()
        }
        fun invalidatePaint() {
            actualBox?.invalidate()
            flatParent?.flatText?.invalidatePaint()
            // Color-only text changes keep the current leaf plan. A collapsed
            // container gaining paint must first regain its platform carrier.
            if (kind == "view" && actualBox == null && flatParent != null) markParent(this)
        }
        fun canFlatten(): Boolean = key >= 0 && kind == "text" && actualBox == null && !nativeRequested && handlers.isEmpty() &&
            frame.width() > 0 && frame.height() > 0 && borderWidths.all { it == 0f } &&
            radii.all { it == radii[0] } &&
            overflowX == overflowY && (overflowX == "visible" || overflowX == "hidden") &&
            tx == 0f && ty == 0f && sx == 1f && angle == 0f && alpha == 1f &&
            lx == 0f && ly == 0f && lw == 1f && lh == 1f
        /** Structural draw-neutral nodes can lose their carrier, never their identity. */
        fun canCollapse(): Boolean = kind == "view" && actualBox == null && !nativeRequested &&
            handlers.isEmpty() && visible && control == null &&
            props.keys().asSequence().all { it == "testId" } &&
            Color.alpha(backgroundPair.toInt()) == 0 && Color.alpha((backgroundPair ushr 32).toInt()) == 0 &&
            borderWidths.all { it == 0f } && radii.all { it == 0f } &&
            overflowX == "visible" && overflowY == "visible" &&
            tx == 0f && ty == 0f && sx == 1f && angle == 0f && alpha == 1f &&
            lx == 0f && ly == 0f && lw == 1f && lh == 1f
        fun descriptor(dx: Int = flatOriginX, dy: Int = flatOriginY): FlatTextGroup.Leaf {
            // Paint/metadata updates preserve the host-relative structural origin.
            // A new plan supplies its origin; detachment retires it with the leaf.
            flatOriginX = dx; flatOriginY = dy
            val next = leaf ?: FlatTextGroup.Leaf(key).also { leaf = it }
            next.bounds.set(Math.addExact(frame.left, dx), Math.addExact(frame.top, dy),
                Math.addExact(frame.right, dx), Math.addExact(frame.bottom, dy))
            next.text = props.optString("text")
            next.label = props.optString("accessibilityLabel").ifEmpty { null }
            next.enabled = props.optString("disabled") != "true"
            next.accessibilityHidden = props.optString("accessibilityElementsHidden") == "true"
            next.visible = visible
            next.backgroundColor = backgroundColor
            next.textColor = textColor
            next.radius = radii[0]
            next.paddingLeft = inset[0]; next.paddingTop = inset[1]
            next.paddingRight = inset[2]; next.paddingBottom = inset[3]
            next.clip = clipContents
            next.ellipsis = ellipsis
            return next
        }
        fun detachFlat() { flatParent = null; leaf = null; flatOriginX = 0; flatOriginY = 0 }
        fun updateFlat() {
            if (kind == "text" && flatParent != null) {
                descriptor()
                flatParent?.flatText?.invalidatePaint()
                flatParent?.flatText?.semanticsChanged()
            }
        }
        fun detachNative() { actualBox?.let { (it.parent as? ViewGroup)?.removeView(it) } }
        fun nativeBox(): Box? = actualBox
        fun releaseChildren() {
            (scrollChildren ?: actualBox)?.let { releaseGroup(it); boxOwners.remove(it) }
            actualBox?.let { boxOwners.remove(it) }
            for (id in logicalChildren) nodes[id]?.let { child ->
                markParent(child)
                child.detachFlat(); child.logicalParent = null; child.rootAttached = false
            }
            parentsDirty.remove(this)
            logicalChildren = IntArray(0)
        }
        fun applyStyle(value: JSONObject, shared: Boolean) {
            style = value
            val decoded = if (shared) decodeStyle(value) else null
            decodedStyle = decoded
            if (decoded != null) {
                backgroundPair = decoded.background
                textPair = decoded.text
                decoded.borders.copyInto(borderPairs)
            } else {
                backgroundPair = colorPair(value, "background_color", Color.TRANSPARENT)
                textPair = colorPair(value, "text_color", Color.BLACK)
                for (side in 0..3) borderPairs[side] = colorPair(value, "border_color_${BORDER_SIDES[side]}", Color.BLACK)
            }
            styleDirty = true
            styleUnchecked = true
        }
        private fun readPaint() {
            validateStyle()
            val decoded = decodedStyle
            if (decoded != null) {
                decoded.widths.copyInto(borderWidths)
                decoded.radii.copyInto(radii)
                decoded.controlInset.copyInto(controlInset)
                decoded.inset.copyInto(inset)
            } else {
                for (side in 0..3) borderWidths[side] = number(style, "border_width_${BORDER_SIDES[side]}") * scale
                for (corner in 0..3) {
                    val radius = number(style, "border_radius_${BORDER_CORNERS[corner]}") * scale
                    radii[corner * 2] = radius; radii[corner * 2 + 1] = radius
                }
                for (axis in 0..3) {
                    val side = CONTENT_SIDES[axis]
                    controlInset[axis] = number(style, "border_width_$side") * scale
                    inset[axis] = controlInset[axis] + number(style, "padding_$side") * scale
                }
            }
            ellipsis = decoded?.ellipsis ?: (style.optString("text_overflow") == "ellipsis")
            buttonFill?.cornerRadii = radii
            buttonMask?.cornerRadii = radii
            visible = decoded?.visible ?: (style.optString("display") != "none" && style.optString("visibility") != "hidden")
            overflowX = decoded?.overflowX ?: style.optString("overflow_x", "visible")
            overflowY = decoded?.overflowY ?: style.optString("overflow_y", "visible")
            clipContents = overflowX != "visible" || overflowY != "visible" ||
                (kind == "image" && (decoded?.rounded ?: radii.any { it > 0f }))
            actualBox?.let(::applyGeometry)
            markParent(this)
            applyPaint()
            decodedStyle = null
        }
        fun paint(mask: Int, buffer: ByteBuffer) {
            // A preceding cold style in this same batch must establish its
            // geometry before a later color patch; its colors were captured by
            // the setter, so finish() cannot overwrite this newer paint.
            if (styleDirty) { readPaint(); styleDirty = false }
            for (field in 0..5) if (mask and (1 shl field) != 0) {
                val pair = colorPair(buffer.int, buffer.int)
                when (field) {
                    0 -> backgroundPair = pair
                    1 -> textPair = pair
                    else -> borderPairs[field - 2] = pair
                }
            }
            applyPaint()
        }
        fun applyPaint() {
            val background = resolveColor(backgroundPair)
            var changed = background != backgroundColor
            if (changed) {
                backgroundColor = background
                if (buttonFill != null) buttonFill.setColor(background) else actualBox?.setFillColor(background)
            }
            val ink = resolveColor(textPair)
            if (ink != textColor) {
                textColor = ink; changed = true
                (control as? EditText)?.setTextColor(ink)
            }
            var borderColor = 0
            var hasBorder = false
            var sameWidths = true
            for (side in 0..3) {
                val color = resolveColor(borderPairs[side])
                if (borderColors[side] != color) { borderColors[side] = color; changed = true }
                if (borderWidths[side] > 0f) {
                    require(!hasBorder || borderColor == color) { "Android borders with different side colors are not implemented" }
                    borderColor = color
                    hasBorder = true
                }
                if (borderWidths[side] != borderWidths[0]) sameWidths = false
            }
            require(radii.all { it == 0f } || !hasBorder || sameWidths) {
                "Android rounded borders with different side widths are not implemented"
            }
            if (changed) {
                leaf?.let { it.backgroundColor = backgroundColor; it.textColor = textColor }
                actualBox?.let { borderColors.copyInto(it.borderColors) }
                invalidatePaint()
            }
        }
        fun placeControl() {
            if (control == null) return
            // The native editor owns its content padding; an image has no such
            // padding, so its view occupies only the CSS content rectangle.
            val edges = if (kind == "image") inset else controlInset
            box.place(control, edges[0].roundToInt(), edges[1].roundToInt(),
                (frame.width() - edges[0] - edges[2]).roundToInt().coerceAtLeast(0),
                (frame.height() - edges[1] - edges[3]).roundToInt().coerceAtLeast(0))
        }
        fun transform() {
            if (tx == 0f && ty == 0f && sx == 1f && angle == 0f && lx == 0f && ly == 0f && lw == 1f && lh == 1f) {
                if (transformed) { actualBox?.setAnimationMatrix(null); transformed = false }
                return
            }
            val origin = style.optJSONArray("transform_origin")
            fun axis(index: Int, size: Int): Float {
                val value = origin?.opt(index) ?: return size / 2f
                return when (value) {
                    is Number -> value.toFloat() * scale
                    is JSONObject -> value.optDouble("pct", 0.0).toFloat() * size / 100f + value.optDouble("px", 0.0).toFloat() * scale
                    else -> error("invalid Android transform-origin")
                }
            }
            val px = axis(0, frame.width()); val py = axis(1, frame.height())
            // Matrix.pre* right-multiplies. Layout FLIP is outermost around
            // the box's top-left; authored motion uses transform-origin.
            val matrix = this.matrix ?: Matrix().also { this.matrix = it }
            matrix.setTranslate(lx, ly)
            matrix.preScale(lw, lh)
            matrix.preTranslate(tx, ty)
            matrix.preTranslate(px, py)
            matrix.preRotate(angle)
            matrix.preScale(sx, sx)
            matrix.preTranslate(-px, -py)
            widget.setAnimationMatrix(matrix)
            transformed = true
        }
        private fun installEvents() {
            if (eventsInstalled) return
            widget.setOnClickListener(if ("press" in handlers) View.OnClickListener {
                if ("press" in handlers) dispatch(key, 0, null)
            } else null)
            widget.isClickable = kind == "button" || "press" in handlers
            widget.setOnHoverListener { _, event ->
                if ("hover" in handlers) when (event.actionMasked) {
                    android.view.MotionEvent.ACTION_HOVER_ENTER -> dispatch(key, 2, null)
                    android.view.MotionEvent.ACTION_HOVER_EXIT -> dispatch(key, 3, null)
                }
                false
            }
            (control ?: widget).setOnKeyListener { _, keyCode, event ->
                if (event.action == android.view.KeyEvent.ACTION_DOWN && "key" in handlers) {
                    val name = when (keyCode) {
                        android.view.KeyEvent.KEYCODE_ENTER -> "Enter"
                        android.view.KeyEvent.KEYCODE_DEL -> "Backspace"
                        android.view.KeyEvent.KEYCODE_ESCAPE -> "Escape"
                        android.view.KeyEvent.KEYCODE_TAB -> "Tab"
                        android.view.KeyEvent.KEYCODE_DPAD_UP -> "ArrowUp"
                        android.view.KeyEvent.KEYCODE_DPAD_DOWN -> "ArrowDown"
                        android.view.KeyEvent.KEYCODE_DPAD_LEFT -> "ArrowLeft"
                        android.view.KeyEvent.KEYCODE_DPAD_RIGHT -> "ArrowRight"
                        else -> if (event.unicodeChar != 0) String(Character.toChars(event.unicodeChar)) else "Unidentified"
                    }
                    dispatch(key, 6, name)
                }
                false
            }
            eventsInstalled = true
        }
        fun update() {
            updating = true
            try {
                validateStyle()
                configured = true
                if ((kind == "text" || (kind == "view" && handlers.isEmpty())) && actualBox == null) {
                    if (styleDirty) { readPaint(); styleDirty = false }
                    updateFlat()
                    markParent(this)
                    return
                }
                if (control is EditText) require(props.optString("type", "text") == "text") {
                    "Android input type '${props.optString("type")}' is not implemented"
                }
                val description = props.optString("accessibilityLabel").ifEmpty { null }
                if (widget.contentDescription != description) widget.contentDescription = description
                val enabled = props.optString("disabled") != "true"
                if (widget.isEnabled != enabled) widget.isEnabled = enabled
                val accessibility = if (props.optString("accessibilityElementsHidden") == "true") View.IMPORTANT_FOR_ACCESSIBILITY_NO_HIDE_DESCENDANTS else View.IMPORTANT_FOR_ACCESSIBILITY_AUTO
                if (widget.importantForAccessibility != accessibility) widget.importantForAccessibility = accessibility
                box.accessibilityText = props.optString("text")
                val tag = props.optString("testId")
                if (widget.tag != tag) widget.tag = tag
                installEvents()
                if (styleDirty) { readPaint(); styleDirty = false }
                if (control is EditText) {
                    val next = props.optString("value")
                    if (control.text.toString() != next) {
                        val cursor = control.selectionStart.coerceAtLeast(0).coerceAtMost(next.length)
                        control.setText(next); control.setSelection(cursor)
                    }
                    control.hint = props.optString("placeholder")
                    control.isEnabled = widget.isEnabled
                    control.contentDescription = widget.contentDescription
                    control.setTextColor(textColor)
                    control.setTextSize(android.util.TypedValue.COMPLEX_UNIT_PX, number(style, "font_size", 16f) * scale)
                    control.setPadding(
                        (number(style, "padding_left") * scale).roundToInt(), (number(style, "padding_top") * scale).roundToInt(),
                        (number(style, "padding_right") * scale).roundToInt(), (number(style, "padding_bottom") * scale).roundToInt()
                    )
                }
                if (control is ImageView) {
                    control.scaleType = when (style.optString("object_fit", "fill")) {
                        "fill" -> ImageView.ScaleType.FIT_XY
                        "contain" -> ImageView.ScaleType.FIT_CENTER
                        "cover" -> ImageView.ScaleType.CENTER_CROP
                        "none" -> ImageView.ScaleType.CENTER
                        else -> error("Android object-fit '${style.optString("object_fit")}' is not implemented")
                    }
                    val src = props.optString("imageSource")
                    require(!src.contains(":") && !src.startsWith("/") && !src.split('/').contains("..")) { "Android remote images are not implemented" }
                    if (src != imageSource) {
                        if (src.isEmpty()) {
                            control.setImageDrawable(null)
                            intrinsic(key, -1f, -1f)
                        } else context.assets.open(src).use { stream ->
                            val options = android.graphics.BitmapFactory.Options().apply { inScaled = false }
                            val bitmap = android.graphics.BitmapFactory.decodeStream(stream, null, options)
                                ?: error("Android asset image '$src' could not be decoded")
                            control.setImageDrawable(android.graphics.drawable.BitmapDrawable(context.resources, bitmap))
                            intrinsic(key, bitmap.width.toFloat(), bitmap.height.toFloat())
                        }
                        imageSource = src
                    }
                }
                placeControl()
                transformDirty.add(this)
                invalidatePaint()
            } finally { updating = false }
        }
        private fun validateStyle() {
            if (!styleUnchecked) return
            refuseUnsupportedStyle()
            styleUnchecked = false
        }
        private fun refuseUnsupportedStyle() {
            val decoded = decodedStyle
            val unsupported = if (decoded != null) decoded.unsupported else UNSUPPORTED_STYLE.firstOrNull { style.has(it) }
            require(unsupported == null) { "Android style '$unsupported' is not implemented" }
            require(!(decoded?.decorated ?: (style.optString("text_decoration_line", "none") != "none"))) { "Android text decoration is not implemented" }
            require(!(decoded?.stacked ?: (style.optInt("z_index", 0) != 0))) { "Android z-index stacking is not implemented" }
            require(!(decoded?.pointerOverride ?: (style.optString("pointer_events", "auto") != "auto"))) { "Android pointer-events override is not implemented" }
            val ox = decoded?.overflowX ?: style.optString("overflow_x", "visible")
            val oy = decoded?.overflowY ?: style.optString("overflow_y", "visible")
            require(ox == oy || kind == "scroll") { "Android clipping on only one axis is not implemented" }
            require(kind == "scroll" || (ox != "scroll" && oy != "scroll")) { "Android scrolling requires a scroll node" }
            require(kind != "scroll" || ox != "scroll") { "Android horizontal scrolling is not implemented" }
            require(kind != "scroll" || !(decoded?.scrollBorder ?: BORDER_SIDES.any { number(style, "border_width_$it") != 0f })) {
                "Android scroll borders are not implemented"
            }
            if (control is EditText) {
                require(decoded?.editorFont ?: (style.optInt("font_family", 0) == 0 && style.optInt("font_weight", 400) == 400 && style.optString("font_style", "normal") == "normal")) {
                    "Android styled editor fonts are not implemented"
                }
                require(decoded?.editorSpacing ?: (number(style, "letter_spacing") == 0f && !style.has("line_height"))) { "Android editor letter spacing and line height are not implemented" }
            }
        }
    }

    /** Layout stores the kernel's rectangles, bypassing Android's content measurement. */
    internal class Box(context: Context) : ViewGroup(context) {
        var logicalChildren = IntArray(0)
        var flatText: FlatTextGroup? = null
        var paintText: ((Canvas) -> Unit)? = null
        var accessibilityClass = "android.view.View"
        var accessibilityText = ""
        val borderWidths = FloatArray(4)
        val borderColors = IntArray(4) { Color.BLACK }
        val radii = FloatArray(8)
        var clipContents = false
        var contentWidth: Int? = null
        var contentHeight: Int? = null
        private var borderPaint: android.graphics.Paint? = null
        private var fillPaint: android.graphics.Paint? = null
        private var fillColor = Color.TRANSPARENT
        private var outline: Path? = null
        private var borderPath: Path? = null
        private var innerRadii: FloatArray? = null
        private var outlineDirty = true
        private var borderDirty = true
        private val frames = HashMap<View, Rect>()
        init { setWillNotDraw(false); clipChildren = false; clipToPadding = false; isSaveEnabled = false }
        fun setFillColor(color: Int) { fillColor = color; fillPaint?.color = color }
        private fun fillPaint() = fillPaint ?: android.graphics.Paint(android.graphics.Paint.ANTI_ALIAS_FLAG)
            .apply { color = fillColor }.also { fillPaint = it }
        private fun borderPaint() = borderPaint ?: android.graphics.Paint(android.graphics.Paint.ANTI_ALIAS_FLAG)
            .also { borderPaint = it }
        fun geometryChanged() { outlineDirty = true; borderDirty = true }
        override fun onSizeChanged(w: Int, h: Int, oldw: Int, oldh: Int) {
            super.onSizeChanged(w, h, oldw, oldh)
            geometryChanged()
        }
        fun place(child: View, x: Int, y: Int, width: Int, height: Int) {
            val next = Rect(x, y, x + width, y + height)
            if (frames[child] != next) { frames[child] = next; requestLayout() }
        }
        override fun onMeasure(widthMeasureSpec: Int, heightMeasureSpec: Int) {
            fun extent(spec: Int, authored: Int?): Int = when (MeasureSpec.getMode(spec)) {
                MeasureSpec.UNSPECIFIED -> authored ?: MeasureSpec.getSize(spec)
                MeasureSpec.AT_MOST -> minOf(authored ?: MeasureSpec.getSize(spec), MeasureSpec.getSize(spec))
                else -> MeasureSpec.getSize(spec)
            }
            setMeasuredDimension(extent(widthMeasureSpec, contentWidth), extent(heightMeasureSpec, contentHeight))
            for (i in 0 until childCount) {
                val child = getChildAt(i); val rect = frames[child] ?: Rect()
                child.measure(MeasureSpec.makeMeasureSpec(rect.width().coerceAtLeast(0), MeasureSpec.EXACTLY), MeasureSpec.makeMeasureSpec(rect.height().coerceAtLeast(0), MeasureSpec.EXACTLY))
            }
        }
        override fun onLayout(changed: Boolean, l: Int, t: Int, r: Int, b: Int) {
            for (i in 0 until childCount) {
                val child = getChildAt(i); val rect = frames[child] ?: Rect()
                child.layout(rect.left, rect.top, rect.right, rect.bottom)
            }
        }
        override fun onDraw(canvas: Canvas) {
            super.onDraw(canvas)
            val rounded = radii.any { it > 0f }
            if (Color.alpha(fillColor) != 0) {
                val fillPaint = fillPaint()
                if (!rounded) canvas.drawRect(0f, 0f, width.toFloat(), height.toFloat(), fillPaint)
                else if (radii.all { it == radii[0] }) {
                    canvas.drawRoundRect(0f, 0f, width.toFloat(), height.toFloat(), radii[0], radii[0], fillPaint)
                } else canvas.drawPath(outline(), fillPaint)
            }
            if (rounded && borderWidths[0] > 0f) {
                val borderPaint = borderPaint()
                borderPaint.color = borderColors[0]
                canvas.drawPath(borderOutline(), borderPaint)
            } else {
                for (side in 0..3) {
                    val thickness = borderWidths[side]
                    if (thickness <= 0f) continue
                    val borderPaint = borderPaint()
                    borderPaint.color = borderColors[side]
                    when (side) {
                        0 -> canvas.drawRect(0f, 0f, width.toFloat(), thickness, borderPaint)
                        1 -> canvas.drawRect(width - thickness, 0f, width.toFloat(), height.toFloat(), borderPaint)
                        2 -> canvas.drawRect(0f, height - thickness, width.toFloat(), height.toFloat(), borderPaint)
                        3 -> canvas.drawRect(0f, 0f, thickness, height.toFloat(), borderPaint)
                    }
                }
            }
            if (clipContents) {
                val saved = canvas.save()
                try { clip(canvas); paintText?.invoke(canvas) }
                finally { canvas.restoreToCount(saved) }
            } else paintText?.invoke(canvas)
        }
        fun accessibilityContentClip(): Path? {
            if (!clipContents) return null
            return if (radii.any { it > 0f }) Path(outline()) else Path().apply {
                addRect(0f, 0f, width.toFloat(), height.toFloat(), Path.Direction.CW)
            }
        }
        private fun outline(): Path {
            val path = outline ?: Path().also { outline = it }
            if (outlineDirty) {
                path.reset()
                path.addRoundRect(0f, 0f, width.toFloat(), height.toFloat(), radii, Path.Direction.CW)
                outlineDirty = false
            }
            return path
        }
        private fun borderOutline(): Path {
            val path = borderPath ?: Path().also { borderPath = it }
            if (borderDirty) {
                val thickness = borderWidths[0]
                path.reset(); path.fillType = Path.FillType.EVEN_ODD
                path.addRoundRect(0f, 0f, width.toFloat(), height.toFloat(), radii, Path.Direction.CW)
                if (width > thickness * 2 && height > thickness * 2) {
                    val inner = innerRadii ?: FloatArray(8).also { innerRadii = it }
                    for (i in 0..7) inner[i] = (radii[i] - thickness).coerceAtLeast(0f)
                    path.addRoundRect(thickness, thickness, width - thickness, height - thickness, inner, Path.Direction.CW)
                }
                borderDirty = false
            }
            return path
        }
        private fun clip(canvas: Canvas) {
            if (radii.any { it > 0f }) {
                canvas.clipPath(outline())
            } else canvas.clipRect(0, 0, width, height)
        }
        override fun dispatchDraw(canvas: Canvas) {
            if (clipContents) {
                val saved = canvas.save()
                try {
                    clip(canvas)
                    flatText?.draw(canvas) ?: super.dispatchDraw(canvas)
                }
                finally { canvas.restoreToCount(saved) }
            } else flatText?.draw(canvas) ?: super.dispatchDraw(canvas)
        }
        override fun getAccessibilityNodeProvider(): AccessibilityNodeProvider? = flatText ?: super.getAccessibilityNodeProvider()
        override fun dispatchHoverEvent(event: MotionEvent): Boolean = flatText?.dispatchHoverEvent(event) == true || super.dispatchHoverEvent(event)
        override fun onInitializeAccessibilityNodeInfo(info: AccessibilityNodeInfo) {
            super.onInitializeAccessibilityNodeInfo(info)
            info.className = accessibilityClass
            if (accessibilityText.isNotEmpty()) info.text = accessibilityText
        }
        override fun onViewRemoved(child: View) { super.onViewRemoved(child); frames.remove(child) }
    }

    private fun number(style: JSONObject, key: String, fallback: Float = 0f): Float = (style.opt(key) as? Number)?.toFloat() ?: fallback
    private fun colorPair(light: Int, dark: Int): Long = (light.toLong() and 0xffffffffL) or (dark.toLong() shl 32)
    private fun resolveColor(pair: Long): Int = (if (paintDark) pair ushr 32 else pair).toInt()
    private fun colorPair(style: JSONObject, key: String, fallback: Int): Long {
        val channels = style.optJSONArray(key) ?: return colorPair(fallback, fallback)
        fun color(channels: JSONArray): Int {
            require(channels.length() == 4)
            return Color.argb(channels.getInt(3), channels.getInt(0), channels.getInt(1), channels.getInt(2))
        }
        if (channels.optJSONArray(0) != null) {
            return colorPair(color(channels.getJSONArray(0)), color(channels.getJSONArray(1)))
        }
        val value = color(channels)
        return colorPair(value, value)
    }
    private fun repaintScheme() {
        paintDark = authoredDark ?: (context.resources.configuration.uiMode and Configuration.UI_MODE_NIGHT_MASK == Configuration.UI_MODE_NIGHT_YES)
        for (index in 0 until nodes.size()) nodes.valueAt(index).applyPaint()
    }
    private fun node(id: Int) = nodes[id] ?: error("Android batch references absent node $id")
    private fun ownerOf(box: Box): Node? {
        if (box === root) return null
        return boxOwners[box] ?: error("Android carrier has no logical owner")
    }
    private fun childrenOf(parent: Node?) = parent?.logicalChildren ?: rootChildren
    private fun markParent(child: Node) {
        val host = child.flatParent
        if (host != null) parentsDirty.add(ownerOf(host))
        else if (child.logicalParent != null || child.rootAttached) parentsDirty.add(child.logicalParent)
    }
    private fun attach(parent: Node?, ids: JSONArray) {
        val next = IntArray(ids.length()) { ids.getInt(it) }
        require(next.toSet().size == next.size) { "duplicate Android logical child" }
        for (id in next) {
            var ancestor = parent
            while (ancestor != null) {
                require(ancestor.key != id) { "cyclic Android logical children" }
                ancestor = ancestor.logicalParent
            }
        }
        val retained = next.toHashSet()
        for (id in childrenOf(parent)) if (id !in retained) nodes[id]?.let { n ->
            markParent(n)
            if (n.logicalParent === parent) { n.logicalParent = null; n.rootAttached = false }
        }
        for (id in next) {
            val n = node(id)
            val previous = n.logicalParent
            if (previous !== parent || (parent == null && !n.rootAttached)) {
                markParent(n)
                if (previous != null) {
                    previous.logicalChildren = previous.logicalChildren.filter { it != id }.toIntArray()
                } else if (n.rootAttached) rootChildren = rootChildren.filter { it != id }.toIntArray()
            }
            n.logicalParent = parent; n.rootAttached = parent == null
        }
        if (parent == null) rootChildren = next else { parent.logicalChildren = next; parent.childrenReconciled = false }
        // A collapsed container's child-list change dirties its existing host.
        if (parent?.flatParent != null) parentsDirty.add(ownerOf(parent.flatParent!!))
        else parentsDirty.add(parent)
        layoutDirty = true
    }
    private fun releaseGroup(parent: Box) {
        if (parent.flatText == null) return
        parent.flatText?.close(); parent.flatText = null
        flatParents.remove(parent)
        for (n in groupMembers.remove(parent).orEmpty()) if (n.flatParent === parent) n.detachFlat()
    }
    private fun attachNative(owner: Node?, complete: Boolean) {
        val parent = owner?.childrenBox ?: root
        val ids = childrenOf(owner)
        parent.logicalChildren = ids
        val ordered = ids.map { node(it).box }
        if (parent.childCount > 0) {
            val retained = HashSet(ordered)
            for (i in parent.childCount - 1 downTo 0) if (parent.getChildAt(i) !in retained) parent.removeViewAt(i)
        }
        for ((index, child) in ordered.withIndex()) {
            if (parent.getChildAt(index) !== child) {
                (child.parent as? ViewGroup)?.removeView(child)
                parent.addView(child, index)
            }
            val n = node(ids[index]); val rect = n.frame
            parent.place(child, rect.left, rect.top, rect.width(), rect.height())
            // A native ancestor transform must not re-record unchanged child
            // groups. New/uninitialized carriers and dirty descendant paths
            // still reconcile, even when their Box existed before attachment.
            if (complete || !n.childrenReconciled || n in reconcilePaths) reconcile(n, complete)
        }
        layoutDirty = true
    }
    /** Promotion is permanent for nodes whose Views have been returned. */
    private fun materialize(owner: Node?, complete: Boolean = true) {
        val parent = owner?.childrenBox ?: root
        val hadGroup = parent.flatText != null
        releaseGroup(parent)
        attachNative(owner, complete || hadGroup)
        owner?.childrenReconciled = true
    }
    private data class FlatPlan(val members: List<Node>, val leaves: List<FlatTextGroup.Leaf>)
    private fun flatPlan(owner: Node?): FlatPlan? {
        // Preserve the existing unbounded immediate-leaf fast path. Only the
        // newly collapsed structural walk is bounded, never the retained list.
        val direct = childrenOf(owner).map(::node)
        if (direct.isNotEmpty() && direct.all { it.canFlatten() && it.logicalChildren.isEmpty() }) {
            return FlatPlan(direct, direct.map { it.descriptor(0, 0) })
        }
        val members = ArrayList<Node>(); val leaves = ArrayList<FlatTextGroup.Leaf>()
        val visited = HashSet<Int>()
        fun visit(n: Node, dx: Int, dy: Int): Boolean {
            if (!visited.add(n.key) || members.size >= SUBTREE_NODES) return false
            members.add(n)
            if (n.canFlatten()) {
                if (n.logicalChildren.isNotEmpty()) return false
                leaves.add(n.descriptor(dx, dy))
                return true
            }
            if (!n.canCollapse()) return false
            val x = Math.addExact(dx, n.frame.left); val y = Math.addExact(dy, n.frame.top)
            return n.logicalChildren.all { visit(node(it), x, y) }
        }
        if (!childrenOf(owner).all { visit(node(it), 0, 0) } || leaves.isEmpty()) return null
        return FlatPlan(members, leaves)
    }
    private fun reconcile(owner: Node?, complete: Boolean = false) {
        if (owner?.flatParent != null) return // Its complete subtree belongs to an ancestor group.
        // Controls own their child widget; an empty logical child-list must not remove it.
        if (owner != null && owner.control != null && owner.kind != "scroll") { owner.childrenReconciled = true; return }
        if (owner?.kind == "text" && owner.logicalChildren.isEmpty()) { owner.childrenReconciled = true; return }
        val parent = owner?.childrenBox ?: root
        val plan = flatPlan(owner)
        if (plan == null) { materialize(owner, complete); return }
        val group = parent.flatText ?: FlatTextGroup(parent, FlatTextGroup.TextDrawer(text::drawDirect),
            FlatTextGroup.TextDrawer(text::draw),
            contentClip = { view -> (view as? Box)?.accessibilityContentClip() }).also {
            parent.removeAllViews(); parent.flatText = it; flatParents.add(parent)
        }
        val retained = plan.members.toHashSet()
        for (old in groupMembers[parent].orEmpty()) if (old !in retained && old.flatParent === parent) old.detachFlat()
        for (n in plan.members) {
            if (n.flatParent !== parent) text.discardRecordedPaint(n.key)
            n.flatParent = parent
        }
        groupMembers[parent] = plan.members
        parent.logicalChildren = plan.leaves.map { it.id }.toIntArray()
        group.setLeaves(plan.leaves)
        owner?.childrenReconciled = true
    }
    fun cold(op: JSONObject, resolvedStyle: JSONObject? = null) {
        val id = op.optInt("id")
        when (op.getString("op")) {
            "create" -> {
                require(nodes.indexOfKey(id) < 0)
                val n = Node(id, op.getString("kind"), op.getJSONObject("props"),
                    resolvedStyle ?: op.getJSONObject("style"))
                n.applyStyle(n.style, resolvedStyle != null)
                val handlers = op.getJSONArray("handlers")
                n.handlers = (0 until handlers.length()).map { handlers.getString(it) }.toSet()
                require(n.handlers.all { it in setOf("press", "hover", "key", "input", "change", "focus", "blur", "submit", "scroll") }) {
                    "Android event handlers ${n.handlers - setOf("press", "hover", "key", "input", "change", "focus", "blur", "submit", "scroll")} are not implemented"
                }
                nodes.put(id, n); dirty.add(n)
            }
            "props" -> {
                val n = node(id); val set = op.getJSONObject("set"); val clear = op.getJSONArray("clear")
                for (key in set.keys()) n.props.put(key, set.get(key))
                repeat(clear.length()) { n.props.remove(clear.getString(it)) }
                dirty.add(n)
            }
            "style" -> { val n = node(id); n.applyStyle(resolvedStyle ?: op.getJSONObject("style"), resolvedStyle != null); dirty.add(n) }
            "children" -> {
                val n = node(id); val ids = op.getJSONArray("ids")
                if (n.control != null && n.kind != "scroll") require(ids.length() == 0) { "Android control children are not implemented" }
                else if (n.kind == "text" && ids.length() == 0 && !n.materialized) Unit
                else attach(n, ids)
            }
            "roots" -> {
                val ids = op.getJSONArray("ids")
                firstRoot = if (ids.length() > 0) ids.getInt(0) else null
                attach(null, ids)
            }
            "destroy" -> nodes[id]?.let { n ->
                markParent(n)
                n.logicalParent?.let { parent ->
                    parent.logicalChildren = parent.logicalChildren.filter { it != id }.toIntArray()
                }
                if (n.rootAttached) rootChildren = rootChildren.filter { it != id }.toIntArray()
                n.releaseChildren()
                nodes.remove(id)
                n.detachNative(); n.detachFlat()
                dirty.remove(n); transformDirty.remove(n); text.remove(id)
            }
            "paragraph" -> {
                val runs = op.getJSONArray("runs")
                require(runs.length() == 0) { "Android styled inline paragraph paint is not implemented" }
                node(id).invalidatePaint()
            }
            "collections" -> require(op.getJSONArray("items").length() == 0) { "Android viewport collections are not implemented" }
            "title" -> title(op.optString("title", ""))
            "language" -> root.layoutDirection = if (op.optString("dir") == "rtl") View.LAYOUT_DIRECTION_RTL else View.LAYOUT_DIRECTION_LTR
            "router" -> Unit // Rust owns navigation; the retained roots carry the active page.
            "command" -> when (op.getString("name")) {
                "setScheme" -> {
                    val args = op.getJSONArray("args")
                    require(args.length() == 1)
                    authoredDark = when (val scheme = args.getString(0)) {
                        "dark" -> true
                        "light" -> false
                        "system" -> null
                        else -> error("invalid Android color scheme '$scheme'")
                    }
                    repaintScheme()
                }
                "focus" -> node(op.getJSONArray("args").getInt(0)).let {
                    val target = it.control ?: it.widget
                    target.requestFocus()
                    if (target is EditText) target.post {
                        if (target.hasFocus()) context.getSystemService(InputMethodManager::class.java).showSoftInput(target, InputMethodManager.SHOW_IMPLICIT)
                    }
                }
                "blur" -> {
                    root.findFocus()?.clearFocus()
                    root.requestFocus()
                    context.getSystemService(InputMethodManager::class.java).hideSoftInputFromWindow(root.windowToken, 0)
                }
                else -> error("Android command '${op.getString("name")}' is not implemented")
            }
            "animations" -> require(op.getJSONArray("specs").length() == 0) { "Android lowered animations are not implemented" }
            else -> error("Android operation '${op.getString("op")}' is not implemented")
        }
    }
    fun frame(id: Int, x: Float, y: Float, w: Float, h: Float) {
        val n = node(id)
        val left = (x * scale).roundToInt(); val top = (y * scale).roundToInt()
        val width = (w * scale).roundToInt(); val height = (h * scale).roundToInt()
        n.frame.set(left, top, left + width, top + height)
        n.nativeBox()?.let { widget -> (widget.parent as? Box)?.place(widget, left, top, width, height) }
        markParent(n)
        n.placeControl()
        transformDirty.add(n)
        layoutDirty = true
    }
    fun content(id: Int, width: Float, height: Float) {
        val n = node(id)
        if (n.control is ScrollView) {
            n.childrenBox.contentWidth = (width * scale).roundToInt().coerceAtLeast(0)
            n.childrenBox.contentHeight = (height * scale).roundToInt().coerceAtLeast(0)
            n.childrenBox.layoutParams = android.widget.FrameLayout.LayoutParams(n.childrenBox.contentWidth!!, n.childrenBox.contentHeight!!)
        }
        layoutDirty = true
    }
    fun present(id: Int, property: Int, x: Double, y: Double, w: Double, h: Double) {
        val n = node(id)
        when (property) {
            1 -> { n.tx = x.toFloat() * scale; n.ty = y.toFloat() * scale }
            2 -> n.sx = x.toFloat()
            3 -> n.angle = x.toFloat()
            4 -> { n.alpha = x.toFloat(); n.nativeBox()?.alpha = n.alpha; semanticsGeometryDirty = true }
            5 -> { n.lx = x.toFloat() * scale; n.ly = y.toFloat() * scale; n.lw = w.toFloat(); n.lh = h.toFloat() }
        }
        if (property != 4) transformDirty.add(n)
        markParent(n)
    }
    /** Opcode 5 changes only cached paint, preserving cold style geometry and events. */
    fun paint(id: Int, mask: Int, buffer: ByteBuffer) = node(id).paint(mask, buffer)
    /** Opcode 6 preserves paragraph invalidations without parsing an empty JSON run list. */
    fun invalidate(id: Int) = node(id).invalidatePaint()
    fun finish() {
        try {
            val ancestorChanged = semanticsGeometryDirty || dirty.isNotEmpty() || transformDirty.isNotEmpty() || parentsDirty.isNotEmpty()
            for (node in dirty) node.update()
            dirty.clear()
            val parents = parentsDirty.toSet()
            parentsDirty.clear()
            val paths = HashSet<Node>()
            for (parent in parents) {
                var cursor = parent
                while (cursor != null && paths.add(cursor)) cursor = cursor.logicalParent
            }
            reconcilePaths = paths
            // A changed ancestor follows only paths to dirty child owners;
            // cold/uninitialized descendants still establish their complete tree.
            for (parent in parents) {
                var ancestor = parent?.logicalParent
                var covered = parent != null && null in parents && parent.rootAttached
                while (ancestor != null && !covered) {
                    covered = ancestor in parents || (null in parents && ancestor.rootAttached)
                    ancestor = ancestor.logicalParent
                }
                if (!covered) reconcile(parent)
            }
            // Carrier creation configures the same already-visited logical tree.
            parentsDirty.clear()
            for (node in transformDirty) node.transform()
            transformDirty.clear()
            if (ancestorChanged) for (parent in flatParents) parent.flatText?.semanticsChanged()
            semanticsGeometryDirty = false
            for (parent in flatParents) parent.flatText?.flushSemantics()
            if (layoutDirty) { root.requestLayout(); layoutDirty = false }
            val viewport = "$coversWindow:$resizesForKeyboard"
            if (viewport != lastViewport) { lastViewport = viewport; viewportChanged() }
        } finally { reconcilePaths = emptySet(); discardStylePool() }
    }
    fun appearanceChanged() {
        text.configurationChanged()
        // A box's existing display list can still reference the discarded text
        // node. Record it again even when the resolved text color is unchanged.
        for (index in 0 until nodes.size()) {
            val node = nodes.valueAt(index)
            if (node.kind == "text") node.invalidatePaint()
        }
        repaintScheme()
    }
    fun close() {
        if (closed) return
        closed = true
        for (parent in flatParents.toList()) releaseGroup(parent)
        groupMembers.clear(); boxOwners.clear()
        flatParents.clear(); parentsDirty.clear(); semanticsGeometryDirty = false; discardStylePool()
    }

    private companion object {
        const val SUBTREE_NODES = 64
        const val STYLE_SLOTS = 128
        val UNSUPPORTED_STYLE = arrayOf("background_image", "clip_path", "filter", "backdrop_filter", "backdrop_blur", "background_material", "shadow_color", "shadow_offset", "shadow_radius", "shadow_opacity", "material", "press_scale", "tint_color")
        val BORDER_SIDES = arrayOf("top", "right", "bottom", "left")
        val CONTENT_TO_BORDER = intArrayOf(3, 0, 1, 2)
        val CONTENT_SIDES = arrayOf("left", "top", "right", "bottom")
        val BORDER_CORNERS = arrayOf("top_left", "top_right", "bottom_right", "bottom_left")
    }
}
