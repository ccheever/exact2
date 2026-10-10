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
import android.view.accessibility.AccessibilityManager
import android.view.accessibility.AccessibilityNodeInfo
import android.view.accessibility.AccessibilityNodeProvider
import android.widget.EditText
import android.widget.ImageView
import android.widget.ScrollView
import org.json.JSONArray
import org.json.JSONObject
import java.nio.ByteBuffer
import java.util.IdentityHashMap
import kotlin.math.ceil
import kotlin.math.roundToInt

/** Android Views retain their hardware RenderNodes; Rust is the sole layout owner. */
internal class Presenter(
    private val context: Context, private val text: TextEngine,
    private val dispatch: (Int, Int, String?) -> Unit,
    private val title: (String) -> Unit,
    private val intrinsic: (Int, Float, Float) -> Unit,
    private val viewportChanged: () -> Unit,
    private val collectionReport: (ByteArray) -> Unit,
    private val nativeFactory: NativeViewFactory?,
    private val scrolled: (Int, Double, Double) -> Unit
) {
    private val scale = context.resources.displayMetrics.density
    private val images = NativeImages(context)
    fun rasterViewport(width: Int, height: Int) = images.viewport(width, height)
    fun trimRaster() = images.trim()
    private val accessibility = context.getSystemService(AccessibilityManager::class.java)
    val root = Box(context)
        .apply { isFocusableInTouchMode = true }
    private val nodes = SparseArray<Node>()
    private var constructingNative: Node? = null
    fun intrinsicOwner(id: Int): Any? = if (closed) null else
        (nodes[id] ?: constructingNative?.takeIf { it.key == id })?.intrinsicIdentity
    private fun dispatchInteractive(id: Int, kind: Int, value: String?) {
        if (closed) return
        val node = nodes[id] ?: return
        val event = when (kind) {
            0 -> "press"
            1, 24 -> "change"
            2, 3 -> "hover"
            4 -> "focus"
            5 -> "blur"
            6 -> "key"
            7 -> "submit"
            13 -> "scroll"
            23, 25 -> "input"
            else -> null
        }
        // SDK controls emit input/change and focus independently. Only deliver
        // the authored channel, as the Apple control adapters do.
        if (event != null && event !in node.handlers) return
        when (kind) {
            0, 1, 2, 4, 6, 7, 23, 24, 25 -> if (node.isInert()) return
        }
        dispatch(id, kind, value)
    }
    private val navigation = NativeNavigation(context) { id -> dispatchInteractive(id, 0, null) }
    private var navigationDirty = false
    private val navigationOwners = HashSet<Int>()
    private var pendingFocus: Any? = null
    private var nativeMounts = emptyList<NativeNavigation.Mount>()
    fun back(): Boolean = navigation.back()
    private fun trackNavigation(n: Node) {
        if (n.props.has("navigationKey") || n.props.optString("accessibilityRole") in setOf("toolbar", "tablist")) navigationOwners.add(n.key)
        else navigationOwners.remove(n.key)
        if (navigationOwners.isNotEmpty() || nativeMounts.isNotEmpty()) navigationDirty = true
    }
    private val controls = NativeControls(context, ::dispatchInteractive, intrinsic)
    private val collections = NativeCollections(root, scale, { id ->
        nodes[id]?.let { n -> (n.control as? ScrollView)?.let { scroll ->
            n.collectionPort(scroll)
        } }
    }, { id -> nodes[id]?.let { n ->
        NativeCollections.RowBox(n.logicalHeight.toDouble(), n.logicalWidth.toDouble(),
            n.frame.top / scale.toDouble(), n.nativeBox())
    } }, collectionReport)
    fun flushCollectionScroll() = collections.flushDeferredScroll()
    fun begin() = collections.beginBatch()
    fun abort() = collections.abortBatch()
    fun controlsChanged() {
        controls.controlsChanged()
        if (navigationOwners.isNotEmpty() || nativeMounts.isNotEmpty()) navigationDirty = true
    }
    fun resolveControls(query: (Int, Int) -> JSONObject) {
        controls.resolve(query)
        if (navigationDirty) synchronizeNavigation()
        placeNativeMounts()
        focusPendingElement()
        collections.endBatch()
    }
    private fun focusPendingElement() {
        val requested = pendingFocus ?: return
        pendingFocus = null
        val n = if (requested is Number) nodes[requested.toInt()] else
            (0 until nodes.size()).map { nodes.valueAt(it) }.firstOrNull { it.props.optString("id") == requested.toString() }
        require(n != null) { "Android focus target '$requested' is absent" }
        val target = if (n.kind == "control") controls.action(n.key) else n.control ?: n.widget
        require(target != null && n.props.optString("disabled") != "true" && !n.isInert()) { "Android focus target '$requested' is disabled or inert" }
        val owner = n.incarnation
        target.post {
            if (closed || nodes[n.key] !== n || n.incarnation !== owner || n.isInert() || !target.isShown || !target.isEnabled) return@post
            target.requestFocus()
            if (target is EditText && target.hasFocus()) context.getSystemService(InputMethodManager::class.java).showSoftInput(target, InputMethodManager.SHOW_IMPLICIT)
        }
    }
    private fun synchronizeNavigation() {
        navigationDirty = false
        for (mount in nativeMounts) (mount.view.parent as? ViewGroup)?.removeView(mount.view)
        val tree = (0 until nodes.size()).map { index -> nodes.valueAt(index).let { n ->
            NativeNavigation.Node(n.key, n.kind, n.props.keys().asSequence().associateWith { n.props.get(it).toString() },
                n.logicalChildren, n.nativeBox(), Rect(n.frame), n.handlers, controls.title(n.key), n.authoredVisible())
        } }
        val projection = navigation.sync(rootChildren, tree)
        for (index in 0 until nodes.size()) {
            val n = nodes.valueAt(index)
            n.hideForHost(n.key in projection.hiddenRoutes || n.key in projection.projected)
        }
        nativeMounts = projection.mounts
    }
    private fun placeNativeMounts() {
        for (mount in nativeMounts) {
            val owner = node(mount.owner)
            val box = owner.box
            (mount.view.parent as? ViewGroup)?.let { if (it !== box) it.removeView(mount.view) }
            if (mount.view.parent == null) box.addView(mount.view)
            box.place(mount.view, 0, 0, owner.frame.width(), owner.frame.height())
        }
    }
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
    private var focusedNode: Int? = null
    private var touchedNode: Int? = null
    private var pointerGeneration = 0L
    private fun platformOwner(view: View?): Int? {
        var cursor = view
        while (cursor != null) {
            if (cursor is Box) boxOwners[cursor]?.let { return it.key }
            cursor = cursor.parent as? View
        }
        return null
    }
    private val focusListener = android.view.ViewTreeObserver.OnGlobalFocusChangeListener { _, current ->
        focusedNode = platformOwner(current)
        collections.pins(focusedNode, touchedNode)
    }
    init {
        root.touchDispatch = ::pointerEvent
        root.viewTreeObserver.addOnGlobalFocusChangeListener(focusListener)
    }
    /** Only the outer physical stream releases contact. A child's synthetic
     * CANCEL when ScrollView takes the drag must not release that row's pin.
     */
    private fun pointerEvent(event: MotionEvent) {
        if (closed) return
        when (event.actionMasked) {
            MotionEvent.ACTION_DOWN -> {
                pointerGeneration++
                touchedNode = null
                collections.pins(focusedNode, null)
            }
            MotionEvent.ACTION_UP, MotionEvent.ACTION_CANCEL -> {
                val released = pointerGeneration
                root.post {
                    if (!closed && pointerGeneration == released) {
                        touchedNode = null
                        collections.pins(focusedNode, null)
                    }
                }
            }
        }
    }
    /** Carrier observers run before delegation: each live descendant replaces
     * its ancestor, so the deepest native target wins even for SDK controls.
     */
    private fun contact(node: Node, event: MotionEvent) {
        if (closed || event.actionMasked != MotionEvent.ACTION_DOWN || nodes[node.key] !== node) return
        touchedNode = node.key
        if (node.kind == "list") collections.intent(node.key, travel = true)
        collections.pins(focusedNode, touchedNode)
    }
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
    data class CollectionInfo(val mounted: Int, val visible: Int, val firstVisibleTestId: String?, val scroll: ScrollView)
    fun collectionInfo(testId: String): CollectionInfo? {
        var owner: Node? = null
        for (index in 0 until nodes.size()) {
            val candidate = nodes.valueAt(index)
            if (candidate.props.optString("testId") == testId) { owner = candidate; break }
        }
        val collection = owner ?: return null
        val scroll = collection.control as? ScrollView ?: return null
        val rows = collections.mountedRows(collection.key)
        val start = scroll.scrollY
        val end = start + scroll.height - scroll.paddingTop - scroll.paddingBottom
        var visible = 0
        var first: NativeCollections.Row? = null
        var firstTop = Int.MAX_VALUE
        for (row in rows) {
            val n = nodes[row.view] ?: continue
            if (n.frame.bottom > start && n.frame.top < end) {
                visible++
                if (n.frame.top < firstTop) { firstTop = n.frame.top; first = row }
            }
        }
        fun rowName(id: Int): String? {
            val n = nodes[id] ?: return null
            val name = n.props.optString("testId")
            if (name.startsWith("row-")) return name.removePrefix("row-")
            for (child in n.logicalChildren) rowName(child)?.let { return it }
            return null
        }
        return CollectionInfo(rows.size, visible, first?.let { rowName(it.root) }, scroll)
    }
    fun textGeometries(prefix: String): List<Geometry> = (0 until nodes.size()).map { nodes.valueAt(it) }
        .filter { it.kind == "text" && it.props.optString("testId").startsWith(prefix) }
        .map { Geometry(it.key, it.kind, Rect(it.frame), it.logicalParent != null || it.rootAttached, it.flatParent != null, it.materialized) }
    data class GroupInfo(val ids: List<Int>, val hasDisplayList: Boolean, val recordedLeafCount: Int, val recordingCount: Long, val usingHardwareNode: Boolean)
    fun groupInfo(): List<GroupInfo> = flatParents.mapNotNull { parent -> parent.flatText?.let {
        GroupInfo(parent.logicalChildren.toList(), it.hasDisplayList, it.recordedLeafCount, it.recordingCount, it.usingHardwareNode)
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
            if (node.props.optString("testId") == testId) return if (node.isInert()) null else if (node.kind == "control") controls.action(node.key) else node.widget
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
        val logicalTextInset = (number(style, "padding_left") + number(style, "border_width_left")) +
            (number(style, "padding_right") + number(style, "border_width_right"))
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
        private var editorType = -1
        val frame = Rect()
        var logicalWidth = 0f
        var logicalHeight = 0f
        private var logicalTextInset = 0f
        var incarnation: Any = this
            private set
        var intrinsicIdentity: Any = this
        private fun current(owner: Any) = !closed && nodes[key] === this && incarnation === owner
        fun renew() {
            incarnation = Any(); intrinsicIdentity = incarnation
            // Pure same-source pixels survive; the observer and size report
            // belong to the new incarnation. loadImage clears changed sources.
            images.cancel(imageRequest); imageRequest = null
            imageSize = 0L; imageFit = ""; intrinsicSource = null
            tx = 0f; ty = 0f; sx = 1f; angle = 0f
            lx = 0f; ly = 0f; lw = 1f; lh = 1f; alpha = 1f
            matrix = null; transformed = false; hostHidden = false
            // A new descriptor also retires a virtual accessibility focus or
            // hover held by FlatTextGroup, even when id/text are unchanged.
            leaf = null; eventsInstalled = false; configured = false; childrenReconciled = false
            actualBox?.let { box ->
                NativeRenewal.reset(box)
                val owner = incarnation
                box.touchDispatch = { event -> if (current(owner)) contact(this, event) }
            }
            text.renew(key)
            // PAINT updates retain the latest authored pairs separately from
            // immutable cold JSON. Re-reading its older colors would regress
            // an unchanged target; Rust republishes changed motion paint.
            decodedStyle = decodedStyle ?: decodeStyle(style); styleDirty = true; styleUnchecked = true
            updateFlat(); markParent(this)
        }
        private var imageLease: NativeImages.Image? = null
        private var imageSource: String? = null
        private var imageRequest: NativeImages.Request? = null
        private var imageSize = 0L
        private var imageFit = ""
        private var intrinsicSource: String? = null
        fun releaseImage() {
            images.cancel(imageRequest); imageRequest = null
            (control as? ImageView)?.setImageDrawable(null)
            imageLease?.release(); imageLease = null
        }
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
        private var hostHidden = false
        fun authoredVisible(): Boolean = visible
        fun hideForHost(hidden: Boolean) {
            if (hostHidden == hidden) return
            hostHidden = hidden
            actualBox?.visibility = if (visible && !hidden) View.VISIBLE else View.INVISIBLE
            updateFlat()
            markParent(this)
        }
        private var clipContents = false
        private var overflowX = "visible"
        private var overflowY = "visible"
        var alpha = 1f
        var paintRank = 0L
        var logicalParent: Node? = null
        var rootAttached = false
        var logicalChildren = IntArray(0)
        var childrenReconciled = false
        fun isInert(): Boolean {
            var ancestor: Node? = this
            while (ancestor != null) {
                if (ancestor.props.optString("inert") == "true") return true
                ancestor = ancestor.logicalParent
            }
            return false
        }
        private var nativeRequested = false
        var flatParent: Box? = null
        private var leaf: FlatTextGroup.Leaf? = null
        private var flatOriginX = 0
        private var flatOriginY = 0
        private val scrollChildren = if (kind == "scroll" || kind == "list") Box(context) else null
        val childrenBox: Box get() = scrollChildren ?: box
        val nativeComponent = if (kind == "native") {
            // A factory may report its natural size synchronously in create().
            // Keep that owner available until the node enters the batch map;
            // ExactView still revalidates it after the whole batch is consumed.
            constructingNative = this
            try {
                checkNotNull(nativeFactory) { "NativeView requires the embedder's NativeViewFactory" }.create(
                    context, props.getString("nativeViewModuleName"), JSONObject(props.optString("nativeViewProps", "{}")),
                    { message -> dispatchInteractive(key, 9, message) }, { w, h ->
                        if (nodes[key] === this || constructingNative === this) intrinsic(key, w, h)
                    })
            } finally { constructingNative = null }
        } else null
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
                            if ("input" in handlers) dispatchInteractive(key, 23, s.toString())
                        }
                    }
                    override fun afterTextChanged(s: Editable?) {}
                })
                setOnFocusChangeListener { _, focused ->
                    if (!updating && !focused && editorChanged) {
                        editorChanged = false
                        if ("change" in handlers) dispatchInteractive(key, 1, this.text.toString())
                    }
                    if (!updating && (if (focused) "focus" else "blur") in handlers) dispatchInteractive(key, if (focused) 4 else 5, null)
                }
                setOnEditorActionListener { _, _, _ ->
                    if ("submit" in handlers) { dispatchInteractive(key, 7, null); true } else false
                }
            }
            "image" -> ImageView(context).apply {
                scaleType = ImageView.ScaleType.FIT_CENTER
                // Replaced content clips to its own viewport even when the
                // surrounding CSS box allows descendants to overflow.
                cropToPadding = true
            }
            "native" -> checkNotNull(nativeComponent).view
            "control" -> controls.create(key, props)
            "scroll", "list" -> (if (kind == "list") NativeCollectionScrollView(context) else ScrollView(context)).apply {
                isFillViewport = false
                clipToPadding = false
                addView(childrenBox)
                setOnScrollChangeListener { _, x, y, oldX, oldY ->
                    scrolled(key, x / scale.toDouble(), y / scale.toDouble())
                    if (kind == "list" && (x != oldX || y != oldY)) collections.changed(key, true)
                    if ("scroll" in handlers && (x != oldX || y != oldY))
                        dispatchInteractive(key, 13, "${x / scale},${y / scale}")
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
            box.paintRank = paintRank
            boxOwners[box] = this
            val owner = incarnation
            box.touchDispatch = { event -> if (current(owner)) contact(this, event) }
            if (control != null) box.addView(control)
            if (kind == "text") box.paintText = { canvas ->
                val saved = canvas.save()
                canvas.translate(inset[0], inset[1])
                text.draw(key, canvas, textOfferWidth(), textColor, ellipsis)
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
            box.visibility = if (visible && !hostHidden) View.VISIBLE else View.INVISIBLE
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
        fun canFlatten(): Boolean = key >= 0 && kind == "text" && paintRank == 0L && actualBox == null && !nativeRequested && handlers.isEmpty() &&
            frame.width() > 0 && frame.height() > 0 && borderWidths.all { it == 0f } &&
            radii.all { it == radii[0] } &&
            overflowX == overflowY && (overflowX == "visible" || overflowX == "hidden") &&
            tx == 0f && ty == 0f && sx == 1f && angle == 0f && alpha == 1f &&
            lx == 0f && ly == 0f && lw == 1f && lh == 1f
        /** Structural draw-neutral nodes can lose their carrier, never their identity. */
        fun canCollapse(): Boolean = kind == "view" && paintRank == 0L && actualBox == null && !nativeRequested &&
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
            next.text = props.optString("text").ifEmpty { text.paragraphText(key) ?: "" }
            next.label = props.optString("accessibilityLabel").ifEmpty { null }
            next.enabled = props.optString("disabled") != "true"
            next.accessibilityHidden = props.optString("inert") == "true" || props.optString("accessibilityElementsHidden") == "true"
            next.visible = visible && !hostHidden
            next.backgroundColor = backgroundColor
            next.textColor = textColor
            next.radius = radii[0]
            next.paddingLeft = inset[0]; next.paddingTop = inset[1]
            next.paddingRight = inset[2]; next.paddingBottom = inset[3]
            next.textWidth = textOfferWidth()
            next.clip = clipContents
            next.ellipsis = ellipsis
            return next
        }
        private fun textOfferWidth(): Int {
            // TextEngine.measure ceilings the logical content offer in pixels.
            // Rounded native View bounds must not narrow that offer at draw time.
            val content = (logicalWidth - logicalTextInset).coerceAtLeast(0f)
            return ceil((content * scale).toDouble()).toInt()
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
                logicalTextInset = decoded.logicalTextInset
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
                logicalTextInset = (number(style, "padding_left") + number(style, "border_width_left")) +
                    (number(style, "padding_right") + number(style, "border_width_right"))
            }
            ellipsis = decoded?.ellipsis ?: (style.optString("text_overflow") == "ellipsis")
            if (kind == "text") text.setTextDecoration(key, style.optString("text_decoration_line", "none"))
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
            for (side in 0..3) {
                val color = resolveColor(borderPairs[side])
                if (borderColors[side] != color) { borderColors[side] = color; changed = true }
            }
            if (changed) {
                leaf?.let { it.backgroundColor = backgroundColor; it.textColor = textColor }
                actualBox?.let { borderColors.copyInto(it.borderColors) }
                invalidatePaint()
            }
        }
        fun collectionPort(scroll: ScrollView) = NativeCollections.Port(scroll, childrenBox,
            (inset[0] - controlInset[0]) / scale.toDouble(), (inset[1] - controlInset[1]) / scale.toDouble(),
            (inset[2] - controlInset[2]) / scale.toDouble(), (inset[3] - controlInset[3]) / scale.toDouble())
        private fun loadImage(control: ImageView) {
            val src = props.optString("imageSource")
            require(!src.contains(":") && !src.startsWith("/") && !src.split('/').contains("..")) { "Android remote images are not implemented" }
            val width = (frame.width() - inset[0] - inset[2]).roundToInt().coerceAtLeast(1)
            val height = (frame.height() - inset[1] - inset[3]).roundToInt().coerceAtLeast(1)
            val size = (width.toLong() shl 32) or height.toLong()
            val fit = style.optString("object_fit", "fill")
            if (src != imageSource || size != imageSize || fit != imageFit) {
                images.cancel(imageRequest)
                imageRequest = null
                if (src != imageSource) releaseImage()
                intrinsicIdentity = PendingIntrinsics.sourceOwner(intrinsicIdentity, imageSource, src)
                imageSource = src; imageSize = size; imageFit = fit
                if (src.isEmpty()) {
                    intrinsicSource = null
                    intrinsic(key, -1f, -1f)
                } else {
                    val owner = incarnation
                    imageRequest = images.request(src, width, height, fit) { image ->
                        if (current(owner) && imageSource == src && imageSize == size && imageFit == fit) {
                            if (imageLease !== image) {
                                image.retain()
                                val previous = imageLease
                                imageLease = image
                                control.setImageBitmap(image.bitmap)
                                previous?.release()
                            }
                            if (intrinsicSource != src) {
                                intrinsicSource = src
                                intrinsic(key, image.width.toFloat(), image.height.toFloat())
                                viewportChanged()
                            }
                        }
                    }
                }
            }
        }
        fun placeControl() {
            if (control == null) return
            // The native editor owns its content padding; an image has no such
            // padding, so its view occupies only the CSS content rectangle.
            val edges = if (kind == "image" || kind == "control") inset else controlInset
            box.place(control, edges[0].roundToInt(), edges[1].roundToInt(),
                (frame.width() - edges[0] - edges[2]).roundToInt().coerceAtLeast(0),
                (frame.height() - edges[1] - edges[3]).roundToInt().coerceAtLeast(0))
            if (control is ImageView) loadImage(control)
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
            val owner = incarnation
            widget.setOnClickListener(if ("press" in handlers) View.OnClickListener {
                if (current(owner) && "press" in handlers) dispatchInteractive(key, 0, null)
            } else null)
            widget.isClickable = kind == "button" || "press" in handlers
            widget.setOnHoverListener { _, event ->
                if (current(owner) && "hover" in handlers) when (event.actionMasked) {
                    android.view.MotionEvent.ACTION_HOVER_ENTER -> dispatchInteractive(key, 2, null)
                    android.view.MotionEvent.ACTION_HOVER_EXIT -> dispatchInteractive(key, 3, null)
                }
                false
            }
            (control ?: widget).setOnKeyListener { _, keyCode, event ->
                if (current(owner) && event.action == android.view.KeyEvent.ACTION_DOWN && "key" in handlers) {
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
                    dispatchInteractive(key, 6, name)
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
                val description = props.optString("accessibilityLabel").ifEmpty { null }
                if (widget.contentDescription != description) widget.contentDescription = description
                val enabled = props.optString("disabled") != "true"
                if (widget.isEnabled != enabled) widget.isEnabled = enabled
                box.inert = props.optString("inert") == "true"
                val accessibility = if (box.inert || props.optString("accessibilityElementsHidden") == "true") View.IMPORTANT_FOR_ACCESSIBILITY_NO_HIDE_DESCENDANTS else View.IMPORTANT_FOR_ACCESSIBILITY_AUTO
                if (widget.importantForAccessibility != accessibility) widget.importantForAccessibility = accessibility
                box.accessibilityText = props.optString("text").ifEmpty { text.paragraphText(key) ?: "" }
                val tag = props.optString("testId")
                if (widget.tag != tag) widget.tag = tag
                installEvents()
                if (styleDirty) { readPaint(); styleDirty = false }
                if (kind == "control") controls.update(key, props, style)
                nativeComponent?.update(JSONObject(props.optString("nativeViewProps", "{}")))
                if (control is EditText) {
                    val mode = when (val type = props.optString("type", "text")) {
                        "text", "search" -> android.text.InputType.TYPE_CLASS_TEXT
                        "email" -> android.text.InputType.TYPE_CLASS_TEXT or android.text.InputType.TYPE_TEXT_VARIATION_EMAIL_ADDRESS
                        "url" -> android.text.InputType.TYPE_CLASS_TEXT or android.text.InputType.TYPE_TEXT_VARIATION_URI
                        "tel" -> android.text.InputType.TYPE_CLASS_PHONE
                        "number" -> android.text.InputType.TYPE_CLASS_NUMBER or android.text.InputType.TYPE_NUMBER_FLAG_DECIMAL or android.text.InputType.TYPE_NUMBER_FLAG_SIGNED
                        "password" -> android.text.InputType.TYPE_CLASS_TEXT or android.text.InputType.TYPE_TEXT_VARIATION_PASSWORD
                        else -> error("Android input type '$type' is not implemented")
                    } or (if (kind == "textarea") android.text.InputType.TYPE_TEXT_FLAG_MULTI_LINE else 0)
                    val readOnly = props.optString("readOnly") == "true"
                    if (mode != editorType || (!readOnly && control.keyListener == null)) {
                        control.inputType = mode; editorType = mode
                    }
                    // HTML readonly remains focusable and selectable. Disable editing
                    // through the SDK key listener, without disabling the control.
                    if (readOnly) control.keyListener = null
                    if (control.isTextSelectable != readOnly) {
                        control.setTextIsSelectable(readOnly)
                        control.movementMethod = android.text.method.ArrowKeyMovementMethod.getInstance()
                    }
                    control.isFocusable = true
                    control.isFocusableInTouchMode = true
                    control.isCursorVisible = !readOnly
                    control.showSoftInputOnFocus = !readOnly
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
                    loadImage(control)
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
            require(!(decoded?.stacked ?: (style.optInt("z_index", 0) != 0))) { "Android z-index stacking is not implemented" }
            require(!(decoded?.pointerOverride ?: (style.optString("pointer_events", "auto") != "auto"))) { "Android pointer-events override is not implemented" }
            val ox = decoded?.overflowX ?: style.optString("overflow_x", "visible")
            val oy = decoded?.overflowY ?: style.optString("overflow_y", "visible")
            require(ox == oy || kind == "scroll" || kind == "list") { "Android clipping on only one axis is not implemented" }
            require(kind == "scroll" || kind == "list" || (ox != "scroll" && oy != "scroll")) { "Android scrolling requires a scroll node" }
            require((kind != "scroll" && kind != "list") || ox != "scroll") { "Android horizontal scrolling is not implemented" }
            require((kind != "scroll" && kind != "list") || !(decoded?.scrollBorder ?: BORDER_SIDES.any { number(style, "border_width_$it") != 0f })) {
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
        var touchDispatch: ((MotionEvent) -> Unit)? = null
        private var focusBeforeInert = FOCUS_BEFORE_DESCENDANTS
        var inert = false
            set(value) {
                if (field == value) return
                field = value
                if (value) {
                    focusBeforeInert = descendantFocusability
                    descendantFocusability = FOCUS_BLOCK_DESCENDANTS
                    findFocus()?.clearFocus()
                } else descendantFocusability = focusBeforeInert
            }
        private fun inertInTree(): Boolean {
            var ancestor: View? = this
            while (ancestor != null) {
                if ((ancestor as? Box)?.inert == true) return true
                ancestor = ancestor.parent as? View
            }
            return false
        }
        override fun requestFocus(direction: Int, previouslyFocusedRect: Rect?): Boolean =
            !inertInTree() && super.requestFocus(direction, previouslyFocusedRect)
        override fun dispatchKeyEvent(event: android.view.KeyEvent): Boolean =
            !inert && super.dispatchKeyEvent(event)
        var accessibilityClass = "android.view.View"
        var accessibilityText = ""
        val borderWidths = FloatArray(4)
        val borderColors = IntArray(4) { Color.BLACK }
        val radii = FloatArray(8)
        var clipContents = false
        var contentWidth: Int? = null
        var contentHeight: Int? = null
        private val borders = NativeBorders()
        private var fillPaint: android.graphics.Paint? = null
        private var fillColor = Color.TRANSPARENT
        private var outline: Path? = null
        private var outlineDirty = true
        private val frames = HashMap<View, Rect>()
        var paintRank = 0L
            set(value) {
                if (field == value) return
                field = value
                (parent as? Box)?.paintOrderChanged()
            }
        private var paintOrderDirty = true
        private var paintOrder: IntArray? = null
        init {
            setWillNotDraw(false); clipChildren = false; clipToPadding = false; isSaveEnabled = false
            setChildrenDrawingOrderEnabled(true)
        }
        private fun paintOrderChanged() { paintOrderDirty = true; invalidate() }
        /** Android uses this same order for drawing and native pointer dispatch.
         * Rebuild only after rank or containment changes, preserving tree order.
         */
        override fun getChildDrawingOrder(childrenCount: Int, drawingPosition: Int): Int {
            if (paintOrderDirty) {
                paintOrderDirty = false
                val first = (getChildAt(0) as? Box)?.paintRank ?: 0L
                val differs = (1 until childrenCount).any { ((getChildAt(it) as? Box)?.paintRank ?: 0L) != first }
                paintOrder = if (!differs) null else (0 until childrenCount).sortedWith(
                    compareBy<Int> { (getChildAt(it) as? Box)?.paintRank ?: 0L }.thenBy { it }).toIntArray()
            }
            return paintOrder?.get(drawingPosition) ?: drawingPosition
        }
        override fun dispatchTouchEvent(event: MotionEvent): Boolean {
            // Every inert carrier stops the native traversal before its children.
            if (inert) return false
            touchDispatch?.invoke(event)
            return super.dispatchTouchEvent(event)
        }
        fun setFillColor(color: Int) { fillColor = color; fillPaint?.color = color }
        private fun fillPaint() = fillPaint ?: android.graphics.Paint(android.graphics.Paint.ANTI_ALIAS_FLAG)
            .apply { color = fillColor }.also { fillPaint = it }
        fun geometryChanged() { outlineDirty = true; borders.invalidate() }
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
            borders.draw(canvas, width.toFloat(), height.toFloat(), borderWidths, borderColors, radii)
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
        override fun getAccessibilityNodeProvider(): AccessibilityNodeProvider? =
            if (inertInTree()) null else flatText ?: super.getAccessibilityNodeProvider()
        override fun dispatchHoverEvent(event: MotionEvent): Boolean =
            !inert && (flatText?.dispatchHoverEvent(event) == true || super.dispatchHoverEvent(event))
        override fun onInitializeAccessibilityNodeInfo(info: AccessibilityNodeInfo) {
            super.onInitializeAccessibilityNodeInfo(info)
            info.className = accessibilityClass
            if (accessibilityText.isNotEmpty()) info.text = accessibilityText
        }
        override fun bringChildToFront(child: View) {
            super.bringChildToFront(child)
            paintOrderChanged()
        }
        override fun onViewAdded(child: View) { super.onViewAdded(child); paintOrderChanged() }
        override fun onViewRemoved(child: View) { super.onViewRemoved(child); frames.remove(child); paintOrderChanged() }
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
        val before = paintDark
        paintDark = authoredDark ?: (context.resources.configuration.uiMode and Configuration.UI_MODE_NIGHT_MASK == Configuration.UI_MODE_NIGHT_YES)
        text.setPaintDark(paintDark)
        for (index in 0 until nodes.size()) {
            val n = nodes.valueAt(index)
            n.applyPaint()
            if (before != paintDark && n.kind == "text") n.invalidatePaint()
        }
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
        // Keep SDK accessibility publication unchanged while a service owns it.
        if (parent.childCount > 1 && accessibility?.isEnabled != true) {
            // Order retained carriers before descending or inserting new children.
            val retainedOrder = ordered.filter { it.parent === parent }
            var prefix = 0
            for (index in 0 until parent.childCount) {
                if (prefix < retainedOrder.size && parent.getChildAt(index) === retainedOrder[prefix]) prefix++
            }
            for (index in prefix until retainedOrder.size) parent.bringChildToFront(retainedOrder[index])
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
        if (owner != null && owner.control != null && owner.kind != "scroll" && owner.kind != "list") { owner.childrenReconciled = true; return }
        if (owner?.kind == "text" && owner.logicalChildren.isEmpty()) { owner.childrenReconciled = true; return }
        val parent = owner?.childrenBox ?: root
        val plan = flatPlan(owner)
        if (plan == null) { materialize(owner, complete); return }
        val group = parent.flatText ?: FlatTextGroup(parent, FlatTextGroup.TextDrawer(text::draw),
            clipsToHostBounds = { parent.clipContents },
            contentClip = { view -> (view as? Box)?.accessibilityContentClip() }).also {
            parent.removeAllViews(); parent.flatText = it; flatParents.add(parent)
        }
        val retained = plan.members.toHashSet()
        for (old in groupMembers[parent].orEmpty()) if (old !in retained && old.flatParent === parent) old.detachFlat()
        for (n in plan.members) {
            n.flatParent = parent
        }
        groupMembers[parent] = plan.members
        parent.logicalChildren = plan.leaves.map { it.id }.toIntArray()
        group.setLeaves(plan.leaves)
        owner?.childrenReconciled = true
    }
    private fun renew(ids: JSONArray) {
        val keys = (0 until ids.length()).map { ids.getInt(it) }
        require(keys.toSet().size == keys.size) { "duplicate Android renewal" }
        // A branch may be created later in this batch; it is already fresh.
        val targets = keys.mapNotNull { nodes[it] }
        fun interacting(target: Node): Boolean {
            for (id in listOf(focusedNode, touchedNode)) {
                var owner = id?.let { nodes[it] }
                while (owner != null) { if (owner === target) return true; owner = owner.logicalParent }
            }
            return target.nativeBox()?.hasFocus() == true
        }
        // Validate the whole renewal before canceling any live owner.
        for (n in targets) NativeRenewal.requireSupported(n.kind, interacting(n), n.key in navigationOwners)
        val pending = pendingFocus
        if (targets.any { n -> if (pending is Number) pending.toInt() == n.key else pending != null && n.props.optString("id") == pending.toString() }) pendingFocus = null
        for (n in targets) { n.renew(); dirty.add(n) }
        semanticsGeometryDirty = true
    }
    fun cold(op: JSONObject, resolvedStyle: JSONObject? = null) {
        navigationDirty = navigationDirty || navigationOwners.isNotEmpty() || nativeMounts.isNotEmpty()
        val id = op.optInt("id")
        when (op.getString("op")) {
            "renew" -> renew(op.getJSONArray("ids"))
            "create" -> {
                require(nodes.indexOfKey(id) < 0)
                val n = Node(id, op.getString("kind"), op.getJSONObject("props"),
                    resolvedStyle ?: op.getJSONObject("style"))
                n.applyStyle(n.style, resolvedStyle != null)
                val handlers = op.getJSONArray("handlers")
                n.handlers = (0 until handlers.length()).map { handlers.getString(it) }.toSet()
                require(n.handlers.all { it in setOf("press", "hover", "key", "input", "change", "focus", "blur", "submit", "scroll", "message") }) {
                    "Android event handlers ${n.handlers - setOf("press", "hover", "key", "input", "change", "focus", "blur", "submit", "scroll", "message")} are not implemented"
                }
                nodes.put(id, n); dirty.add(n); trackNavigation(n)
            }
            "props" -> {
                val n = node(id); val set = op.getJSONObject("set"); val clear = op.getJSONArray("clear")
                for (key in set.keys()) n.props.put(key, set.get(key))
                repeat(clear.length()) { n.props.remove(clear.getString(it)) }
                dirty.add(n); trackNavigation(n)
            }
            "style" -> { val n = node(id); n.applyStyle(resolvedStyle ?: op.getJSONObject("style"), resolvedStyle != null); dirty.add(n) }
            "children" -> {
                val n = node(id); val ids = op.getJSONArray("ids")
                if (n.control != null && n.kind != "scroll" && n.kind != "list") require(ids.length() == 0) { "Android control children are not implemented" }
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
                nodes.remove(id); navigationOwners.remove(id)
                n.detachNative(); n.detachFlat()
                dirty.remove(n); transformDirty.remove(n); text.remove(id); controls.remove(id); n.releaseImage(); n.nativeComponent?.close()
            }
            "paragraph" -> {
                val runs = op.getJSONArray("runs")
                text.setParagraphPaint(id, runs)
                node(id).invalidatePaint()
            }
            "rank" -> nodes[id]?.let { n ->
                val rank = op.getLong("rank")
                if (n.paintRank != rank) {
                    n.paintRank = rank
                    n.nativeBox()?.paintRank = rank
                    // Like Apple's flat leaves, only rank zero shares a plane.
                    // Reconciliation promotes a changed flat/collapsed target.
                    markParent(n)
                }
            }
            "collections" -> collections.snapshots(op.getJSONArray("items"))
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
                "focus" -> {
                    val args = op.getJSONArray("args")
                    require(args.length() == 1)
                    pendingFocus = args.get(0)
                }
                "blur" -> {
                    pendingFocus = null
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
        // The shared host also lays out kernel-only native-control contents.
        // Like Apple, geometry needs an existing presentation target.
        val n = nodes[id] ?: return
        val textWidthChanged = n.kind == "text" && n.logicalWidth != w
        n.logicalWidth = w; n.logicalHeight = h
        val left = (x * scale).roundToInt(); val top = (y * scale).roundToInt()
        val width = (w * scale).roundToInt(); val height = (h * scale).roundToInt()
        n.frame.set(left, top, left + width, top + height)
        // A fractional resize can change the paragraph offer while leaving
        // rounded View bounds unchanged. Its display list still needs a redraw.
        if (textWidthChanged) n.invalidatePaint()
        n.nativeBox()?.let { widget -> (widget.parent as? Box)?.place(widget, left, top, width, height) }
        // A retained carrier's frame changes native layout, not containment.
        // Unmaterialized leaves still re-evaluate their grouping geometry.
        if (!n.materialized) markParent(n)
        n.placeControl()
        transformDirty.add(n)
        layoutDirty = true
    }
    fun content(id: Int, width: Float, height: Float) {
        val n = nodes[id] ?: return
        if (n.control is ScrollView) {
            n.childrenBox.contentWidth = (width * scale).roundToInt().coerceAtLeast(0)
            n.childrenBox.contentHeight = (height * scale).roundToInt().coerceAtLeast(0)
            n.childrenBox.layoutParams = android.widget.FrameLayout.LayoutParams(n.childrenBox.contentWidth!!, n.childrenBox.contentHeight!!)
        }
        layoutDirty = true
    }
    fun present(id: Int, property: Int, x: Double, y: Double, w: Double, h: Double) {
        val n = nodes[id] ?: return
        when (property) {
            1 -> { n.tx = x.toFloat() * scale; n.ty = y.toFloat() * scale }
            2 -> n.sx = x.toFloat()
            3 -> n.angle = x.toFloat()
            4 -> { n.alpha = x.toFloat(); n.nativeBox()?.alpha = n.alpha; semanticsGeometryDirty = true }
            5 -> { n.lx = x.toFloat() * scale; n.ly = y.toFloat() * scale; n.lw = w.toFloat(); n.lh = h.toFloat() }
        }
        if (property != 4) transformDirty.add(n)
        // A retained carrier cannot become flat again. Its presentation only
        // changes native properties, not containment or layout. Unmaterialized
        // leaves still reconcile so their first transform/opacity can promote.
        if (!n.materialized) markParent(n)
    }
    /** Opcode 5 changes only cached paint, preserving cold style geometry and events. */
    fun paint(id: Int, mask: Int, buffer: ByteBuffer) = node(id).paint(mask, buffer)
    /** Opcode 6 preserves paragraph invalidations without parsing an empty JSON run list. */
    fun invalidate(id: Int) {
        text.clearParagraphPaint(id)
        node(id).invalidatePaint()
    }
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
        root.touchDispatch = null
        if (root.viewTreeObserver.isAlive) root.viewTreeObserver.removeOnGlobalFocusChangeListener(focusListener)
        controls.close(); collections.close(); navigation.close(); images.close()
        for (index in 0 until nodes.size()) nodes.valueAt(index).let { node ->
            node.releaseImage(); node.nativeComponent?.close()
        }
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
