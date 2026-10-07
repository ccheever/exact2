package com.exact.android

import android.graphics.Canvas
import android.graphics.Color
import android.graphics.Matrix
import android.graphics.Paint
import android.graphics.Path
import android.graphics.Rect
import android.graphics.RectF
import android.graphics.RenderNode
import android.os.Bundle
import android.util.SparseArray
import android.view.MotionEvent
import android.view.View
import android.view.ViewGroup
import android.view.accessibility.AccessibilityEvent
import android.view.accessibility.AccessibilityManager
import android.view.accessibility.AccessibilityNodeInfo
import android.view.accessibility.AccessibilityNodeProvider
import kotlin.math.ceil
import kotlin.math.floor
import kotlin.math.roundToInt

/**
 * One existing Box hosts an ordered group of passive
 * text leaves. Eligibility, logical parent ownership, fallback and invalidation
 * belong to Presenter; this helper owns neither layout nor application state.
 *
 * Cold groups draw public Paragraph.paint directly. Repeated dirty recordings
 * retain each Source's paragraph commands while backgrounds stay ordered here.
 */
internal class FlatTextGroup(
    private val host: View,
    private val drawText: TextDrawer,
    private val drawCachedText: TextDrawer,
    // Presenter supplies its Box.clipContents path, in Box content coordinates.
    // A null path means no custom CSS canvas clipping for this ancestor.
    private val contentClip: (View) -> Path? = { null }
) : AccessibilityNodeProvider(), AutoCloseable {
    /** Retained draw callbacks keep leaf id, width and paint channels primitive. */
    fun interface TextDrawer {
        fun draw(view: Int, canvas: Canvas, width: Int, color: Int, ellipsis: Boolean)
    }

    /** One instance per live logical leaf; never a substitute Android View. */
    class Leaf(val id: Int, val bounds: Rect = Rect()) {
        var text = ""
        var label: String? = null
        var enabled = true
        var visible = true
        var accessibilityHidden = false
        var backgroundColor = Color.TRANSPARENT
        var textColor = Color.BLACK
        var radius = 0f
        var paddingLeft = 0f
        var paddingTop = 0f
        var paddingRight = 0f
        var paddingBottom = 0f
        var clip = false
        var ellipsis = false
    }

    private val ordered = ArrayList<Leaf>()
    private val byId = SparseArray<Leaf>()
    private var displayList: RenderNode? = null
    private var paint: Paint? = null
    private var clipPath: Path? = null
    private var dirtyPaint = true
    private var dirtySemantics = false
    private var closed = false
    private var recordedWidth = -1
    private var recordedHeight = -1
    private var recordedLeaves = 0
    private var recordings = 0L
    private var recordedWithCache = false
    private var focused = NONE
    private var hovered = NONE
    private val accessibility = host.context.getSystemService(AccessibilityManager::class.java)
    val size: Int get() = ordered.size
    val hasDisplayList: Boolean get() = displayList?.hasDisplayList() == true
    val recordedLeafCount: Int get() = recordedLeaves
    val recordingCount: Long get() = recordings
    val usingHardwareNode: Boolean get() = displayList != null
    val cachingText: Boolean get() = recordedWithCache

    /** Copy order, retain the node-owned mutable descriptors. Call after a batch. */
    fun setLeaves(leaves: List<Leaf>) {
        check(!closed)
        val next = SparseArray<Leaf>()
        for (leaf in leaves) {
            require(leaf.id >= 0 && next.indexOfKey(leaf.id) < 0) { "invalid or duplicate virtual text id" }
            next.put(leaf.id, leaf)
        }
        // Events still refer to the old descriptors while clearing old focus.
        if (focused != NONE && next[focused] !== byId[focused]) clearAccessibilityFocus(focused)
        if (hovered != NONE && next[hovered] !== byId[hovered]) updateHovered(NONE)
        ordered.clear(); ordered.addAll(leaves)
        byId.clear()
        for (index in 0 until next.size()) byId.put(next.keyAt(index), next.valueAt(index))
        invalidatePaint()
        semanticsChanged()
    }

    /** The caller invokes this after any color/text/geometry/visibility change. */
    fun invalidatePaint() {
        check(!closed)
        // All 1,000 background patches dirty this host only once per transaction.
        if (!dirtyPaint) { dirtyPaint = true; host.invalidate() }
        else if (displayList == null) host.invalidate()
    }

    /** Background colors alone do not change semantics. Flush once at finish(). */
    fun semanticsChanged() { if (!closed) dirtySemantics = true }
    fun flushSemantics() {
        if (!dirtySemantics || closed) return
        dirtySemantics = false
        if (focused != NONE && (!isExposed(byId[focused]) || !hostCanExpose())) clearAccessibilityFocus(focused)
        if (hovered != NONE && (!isExposed(byId[hovered]) || !hostCanExpose())) updateHovered(NONE)
        if (accessibility?.isEnabled != true) return
        val event = AccessibilityEvent.obtain(AccessibilityEvent.TYPE_WINDOW_CONTENT_CHANGED)
        event.contentChangeTypes = AccessibilityEvent.CONTENT_CHANGE_TYPE_SUBTREE
        event.packageName = host.context.packageName
        event.setSource(host)
        host.parent?.requestSendAccessibilityEvent(host, event)
    }

    fun draw(canvas: Canvas) {
        check(!closed)
        if (!canvas.isHardwareAccelerated) { drawLeaves(canvas); return }
        val node = displayList ?: RenderNode("Exact passive text group").apply {
            // Inherited CSS clipping remains on the existing Box. Overflowing
            // glyph ink must not gain an implicit group-rectangle clip.
            setClipToBounds(false)
        }.also { displayList = it }
        val width = host.width.coerceAtLeast(1)
        val height = host.height.coerceAtLeast(1)
        if (dirtyPaint || width != recordedWidth || height != recordedHeight || !node.hasDisplayList()) {
            node.discardDisplayList()
            node.setPosition(0, 0, width, height)
            val recording = node.beginRecording(width, height)
            // Record the cold scene directly. Only a repeatedly changed group
            // pays for retained paragraph commands; there is no workload key.
            val cacheText = recordings >= 2
            val completedLeaves: Int
            try { completedLeaves = drawLeaves(recording, cacheText) }
            finally { node.endRecording() }
            recordedWidth = width; recordedHeight = height
            recordedLeaves = completedLeaves
            recordedWithCache = cacheText
            recordings++
            dirtyPaint = false
        }
        canvas.drawRenderNode(node)
    }

    private fun drawLeaves(canvas: Canvas, cacheText: Boolean = false): Int {
        // Deliberately record the complete eager group. Viewport culling here
        // would leave missing rows when ScrollView reuses this display list.
        var completedLeaves = 0
        for (leaf in ordered) {
            if (!leaf.visible) continue
            val frame = leaf.bounds
            val width = frame.width().toFloat(); val height = frame.height().toFloat()
            val saved = canvas.save()
            try {
                canvas.translate(frame.left.toFloat(), frame.top.toFloat())
                if (Color.alpha(leaf.backgroundColor) != 0) {
                    val fill = paint ?: Paint(Paint.ANTI_ALIAS_FLAG).also { paint = it }
                    fill.color = leaf.backgroundColor
                    if (leaf.radius == 0f) canvas.drawRect(0f, 0f, width, height, fill)
                    else canvas.drawRoundRect(0f, 0f, width, height, leaf.radius, leaf.radius, fill)
                }
                // Existing Box paints its background first, then clips text.
                if (leaf.clip) {
                    if (leaf.radius == 0f) canvas.clipRect(0f, 0f, width, height)
                    else {
                        val path = clipPath ?: Path().also { clipPath = it }
                        path.reset()
                        path.addRoundRect(0f, 0f, width, height, leaf.radius, leaf.radius, Path.Direction.CW)
                        canvas.clipPath(path)
                    }
                }
                canvas.translate(leaf.paddingLeft, leaf.paddingTop)
                val offer = (width - leaf.paddingLeft - leaf.paddingRight).coerceAtLeast(0f).roundToInt()
                if (cacheText) drawCachedText.draw(leaf.id, canvas, offer, leaf.textColor, leaf.ellipsis)
                else drawText.draw(leaf.id, canvas, offer, leaf.textColor, leaf.ellipsis)
                completedLeaves++
            } finally { canvas.restoreToCount(saved) }
        }
        return completedLeaves
    }

    private fun isExposed(leaf: Leaf?) = leaf != null && leaf.visible && !leaf.accessibilityHidden
    private fun hostCanExpose(): Boolean {
        if (!host.isAttachedToWindow || host.windowVisibility != View.VISIBLE) return false
        var ancestor: View? = host
        while (ancestor != null) {
            if (ancestor.visibility != View.VISIBLE || ancestor.alpha <= 0f || ancestor.transitionAlpha <= 0f) return false
            ancestor = ancestor.parent as? View
        }
        return true
    }

    private data class ScreenGeometry(val bounds: Rect, val visible: Boolean)
    private fun roundedBounds(value: RectF) = Rect(floor(value.left.toDouble()).toInt(), floor(value.top.toDouble()).toInt(),
        ceil(value.right.toDouble()).toInt(), ceil(value.bottom.toDouble()).toInt())
    private fun RectF.finite() = left.isFinite() && top.isFinite() && right.isFinite() && bottom.isFinite()
    private fun rectangle(value: RectF) = Path().apply { if (value.width() > 0f && value.height() > 0f) addRect(value, Path.Direction.CW) }
    private fun mapPath(value: Path, matrix: Matrix): Path? {
        val local = RectF(); value.computeBounds(local, true)
        val corners = floatArrayOf(local.left, local.top, local.right, local.top, local.right, local.bottom, local.left, local.bottom)
        matrix.mapPoints(corners)
        if (corners.any { !it.isFinite() }) return null
        val result = Path(value); result.transform(matrix)
        val mapped = RectF(); result.computeBounds(mapped, true)
        return result.takeIf { mapped.finite() }
    }

    /**
     * View.transformMatrixToGlobal and getLocationOnScreen omit animationMatrix.
     * Rendering uses T(left,top) * animationMatrix * propertyMatrix instead.
     * Derive the root translation without an inverse (zero scale is legal),
     * preserving ViewRootImpl's window offset and current root scroll.
     */
    private fun screenGeometry(leaf: Leaf): ScreenGeometry {
        val views = ArrayList<View>()
        var current: View? = host
        while (current != null) { views.add(current); current = current.parent as? View }
        views.reverse()
        val top = views.first()
        val propertyTop = top.matrix
        val globalTop = Matrix(); top.transformMatrixToGlobal(globalTop)
        val globalOrigin = floatArrayOf(0f, 0f); globalTop.mapPoints(globalOrigin)
        val propertyOrigin = floatArrayOf(0f, 0f); propertyTop.mapPoints(propertyOrigin)
        val matrix = Matrix().apply { setTranslate(globalOrigin[0] - propertyOrigin[0], globalOrigin[1] - propertyOrigin[1]) }
        val viewportMatrices = ArrayList<Matrix>(views.size)
        for ((index, view) in views.withIndex()) {
            if (index > 0) {
                val parent = views[index - 1]
                matrix.preTranslate(-parent.scrollX.toFloat(), -parent.scrollY.toFloat())
                matrix.preTranslate(view.left.toFloat(), view.top.toFloat())
            }
            view.animationMatrix?.let(matrix::preConcat)
            matrix.preConcat(view.matrix)
            viewportMatrices.add(Matrix(matrix))
        }
        val contentMatrix = Matrix(matrix).apply { preTranslate(-host.scrollX.toFloat(), -host.scrollY.toFloat()) }
        val shape = mapPath(rectangle(RectF(leaf.bounds)), contentMatrix) ?: return ScreenGeometry(Rect(), false)
        val full = RectF(); shape.computeBounds(full, true)
        val fullBounds = roundedBounds(full)
        if (!isExposed(leaf) || !hostCanExpose() || full.width() <= 0f || full.height() <= 0f) return ScreenGeometry(fullBounds, false)
        val visible = Path(shape)
        fun intersect(clip: Path?): Boolean = clip != null && visible.op(clip, Path.Op.INTERSECT) && !visible.isEmpty

        // Ordinary Activity window: this fixed viewport must not move with the
        // root's animation or current scroll. Rounding cancels in this difference.
        val screen = IntArray(2); val window = IntArray(2)
        top.getLocationOnScreen(screen); top.getLocationInWindow(window)
        val windowBounds = RectF((screen[0] - window[0]).toFloat(), (screen[1] - window[1]).toFloat(),
            (screen[0] - window[0] + top.width).toFloat(), (screen[1] - window[1] + top.height).toFloat())
        if (!intersect(rectangle(windowBounds))) return ScreenGeometry(fullBounds, false)
        for ((index, view) in views.withIndex()) {
            val viewport = viewportMatrices[index]
            // clipChildren controls the CHILD RenderNode's own bounds.
            if ((view.parent as? ViewGroup)?.clipChildren == true &&
                !intersect(mapPath(rectangle(RectF(0f, 0f, view.width.toFloat(), view.height.toFloat())), viewport))) return ScreenGeometry(fullBounds, false)
            view.clipBounds?.let { clip ->
                if (!intersect(mapPath(rectangle(RectF(clip)), viewport))) return ScreenGeometry(fullBounds, false)
            }
            // The flat host bypasses super.dispatchDraw, so only native ancestor
            // ViewGroups apply its padding clip. Box clips come from contentClip.
            if (view !== host && view is ViewGroup && view.clipToPadding &&
                (view.paddingLeft != 0 || view.paddingTop != 0 || view.paddingRight != 0 || view.paddingBottom != 0)) {
                val padded = RectF(view.paddingLeft.toFloat(), view.paddingTop.toFloat(),
                    (view.width - view.paddingRight).toFloat(), (view.height - view.paddingBottom).toFloat())
                if (!intersect(mapPath(rectangle(padded), viewport))) return ScreenGeometry(fullBounds, false)
            }
            contentClip(view)?.let { path ->
                val content = Matrix(viewport).apply { preTranslate(-view.scrollX.toFloat(), -view.scrollY.toFloat()) }
                if (!intersect(mapPath(path, content))) return ScreenGeometry(fullBounds, false)
            }
        }
        val clipped = RectF(); visible.computeBounds(clipped, true)
        return if (clipped.finite() && clipped.width() > 0f && clipped.height() > 0f)
            ScreenGeometry(roundedBounds(clipped), true) else ScreenGeometry(fullBounds, false)
    }

    override fun createAccessibilityNodeInfo(virtualViewId: Int): AccessibilityNodeInfo? {
        if (closed) return null
        if (virtualViewId == HOST_VIEW_ID) {
            val info = AccessibilityNodeInfo.obtain()
            host.onInitializeAccessibilityNodeInfo(info)
            info.setSource(host)
            // All native children must have been detached before flattening.
            for (leaf in ordered) if (isExposed(leaf)) info.addChild(host, leaf.id)
            return info
        }
        val leaf = byId[virtualViewId] ?: return null
        if (!isExposed(leaf)) return null
        val geometry = screenGeometry(leaf)
        return AccessibilityNodeInfo.obtain().apply {
            setSource(host, leaf.id)
            setParent(host)
            packageName = host.context.packageName
            className = "android.widget.TextView"
            text = leaf.text
            contentDescription = leaf.label
            // A provider makes the ScrollView content host accessible itself.
            // Speakable leaves must remain individual screen-reader targets.
            isImportantForAccessibility = true
            isScreenReaderFocusable = leaf.text.isNotEmpty() || !leaf.label.isNullOrEmpty()
            isEnabled = leaf.enabled
            isVisibleToUser = geometry.visible
            isAccessibilityFocused = focused == leaf.id
            setBoundsInParent(leaf.bounds)
            setBoundsInScreen(geometry.bounds)
            addAction(AccessibilityNodeInfo.AccessibilityAction.ACTION_SHOW_ON_SCREEN)
            addAction(if (focused == leaf.id)
                AccessibilityNodeInfo.AccessibilityAction.ACTION_CLEAR_ACCESSIBILITY_FOCUS
                else AccessibilityNodeInfo.AccessibilityAction.ACTION_ACCESSIBILITY_FOCUS)
        }
    }

    override fun findAccessibilityNodeInfosByText(text: String, virtualViewId: Int): List<AccessibilityNodeInfo> {
        if (closed) return emptyList()
        val query = text.lowercase(java.util.Locale.ROOT)
        val matches = ArrayList<AccessibilityNodeInfo>()
        for (leaf in ordered) {
            if (virtualViewId != HOST_VIEW_ID && virtualViewId != leaf.id) continue
            if (!isExposed(leaf)) continue
            if (leaf.text.lowercase(java.util.Locale.ROOT).contains(query) ||
                leaf.label?.lowercase(java.util.Locale.ROOT)?.contains(query) == true) {
                createAccessibilityNodeInfo(leaf.id)?.let(matches::add)
            }
        }
        return matches
    }

    override fun findFocus(focus: Int): AccessibilityNodeInfo? =
        if (focus == AccessibilityNodeInfo.FOCUS_ACCESSIBILITY && focused != NONE)
            createAccessibilityNodeInfo(focused) else null

    override fun performAction(virtualViewId: Int, action: Int, arguments: Bundle?): Boolean {
        if (closed) return false
        if (virtualViewId == HOST_VIEW_ID) return host.performAccessibilityAction(action, arguments)
        val leaf = byId[virtualViewId] ?: return false
        if (!isExposed(leaf)) return false
        return when (action) {
            AccessibilityNodeInfo.ACTION_ACCESSIBILITY_FOCUS -> {
                if (focused == leaf.id) false else {
                    if (focused != NONE) clearAccessibilityFocus(focused)
                    focused = leaf.id
                    host.invalidate()
                    send(leaf.id, AccessibilityEvent.TYPE_VIEW_ACCESSIBILITY_FOCUSED)
                    true
                }
            }
            AccessibilityNodeInfo.ACTION_CLEAR_ACCESSIBILITY_FOCUS -> clearAccessibilityFocus(leaf.id)
            AccessibilityNodeInfo.AccessibilityAction.ACTION_SHOW_ON_SCREEN.id ->
                host.requestRectangleOnScreen(Rect(leaf.bounds), true)
            else -> false // Eligibility excludes click/key/focus/selection handlers.
        }
    }

    private fun clearAccessibilityFocus(id: Int): Boolean {
        if (focused != id) return false
        focused = NONE
        host.invalidate()
        send(id, AccessibilityEvent.TYPE_VIEW_ACCESSIBILITY_FOCUS_CLEARED)
        return true
    }

    /** Existing Box.dispatchHoverEvent forwards here before its normal fallback. */
    fun dispatchHoverEvent(event: MotionEvent): Boolean {
        val manager = accessibility ?: return false
        if (closed || !manager.isEnabled || !manager.isTouchExplorationEnabled) return false
        return when (event.actionMasked) {
            MotionEvent.ACTION_HOVER_ENTER, MotionEvent.ACTION_HOVER_MOVE -> {
                var hit = NONE
                for (index in ordered.lastIndex downTo 0) {
                    val leaf = ordered[index]
                    if (isExposed(leaf) && leaf.bounds.contains(event.x.toInt(), event.y.toInt())) {
                        hit = leaf.id; break
                    }
                }
                val previouslyHovered = hovered != NONE
                updateHovered(hit)
                hit != NONE || previouslyHovered
            }
            MotionEvent.ACTION_HOVER_EXIT -> {
                if (hovered == NONE) false else { updateHovered(NONE); true }
            }
            else -> false
        }
    }

    private fun updateHovered(id: Int) {
        if (hovered == id) return
        val previous = hovered
        hovered = id
        if (id != NONE) send(id, AccessibilityEvent.TYPE_VIEW_HOVER_ENTER)
        if (previous != NONE) send(previous, AccessibilityEvent.TYPE_VIEW_HOVER_EXIT)
    }

    private fun send(id: Int, type: Int) {
        if (accessibility?.isEnabled != true) return
        val leaf = byId[id] ?: return
        val event = AccessibilityEvent.obtain(type)
        event.packageName = host.context.packageName
        event.className = "android.widget.TextView"
        event.isEnabled = leaf.enabled
        if (leaf.text.isNotEmpty()) event.text.add(leaf.text)
        event.contentDescription = leaf.label
        event.setSource(host, id)
        host.parent?.requestSendAccessibilityEvent(host, event)
    }

    override fun close() {
        if (closed) return
        if (focused != NONE) clearAccessibilityFocus(focused)
        if (hovered != NONE) updateHovered(NONE)
        closed = true
        displayList?.discardDisplayList(); displayList = null
        ordered.clear(); byId.clear()
    }

    private companion object { const val NONE = Int.MIN_VALUE }
}
