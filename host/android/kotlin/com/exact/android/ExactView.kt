package com.exact.android

import android.content.Context
import android.content.res.Configuration
import android.graphics.Rect
import android.os.Build
import android.os.Handler
import android.os.Looper
import android.os.SystemClock
import android.view.Choreographer
import android.view.View
import android.view.WindowInsets
import android.view.ViewTreeObserver
import android.widget.FrameLayout
import java.nio.ByteBuffer
import kotlin.math.ceil

/** An embeddable imperative session. The main thread owns Rust and Android presentation. */
class ExactView(context: Context, initialPress: String? = null, nativeFactory: NativeViewFactory? = null) : FrameLayout(context), AutoCloseable {
    companion object {
        private fun ownerLooper(): Looper {
            val owner = Looper.getMainLooper()
            check(Looper.myLooper() == owner) { "ExactView must be created on the main thread" }
            return owner
        }
    }
    private val createdAt = System.nanoTime()
    private val initialPressBytes = initialPress?.toByteArray(Charsets.UTF_8)
    private val handler = Handler(ownerLooper())
    private val choreographer = Choreographer.getInstance()
    private val scale = resources.displayMetrics.density
    private var applying = false
    private var booted = false
    private var visible = false
    private var hostVisible = true
    private var closed = false
    private var firstPixel = false
    private var frameQueued = false
    private var pumping = false
    private var pumpNeeded = false
    private var fitQueued = false
    private var fitting = false
    private var insetsReceived = false
    private var viewportValid = false
    private var fittedWidth = 0
    private var fittedHeight = 0
    private val systemInsets = Rect()
    private var imeBottom = 0
    private val viewport = Rect()
    private val deliveredInsets = Rect()
    private var clockOffset = -SystemClock.uptimeMillis().toDouble()
    private var schedule: BatchReader.Schedule? = null
    private val pending = java.util.ArrayDeque<() -> Unit>()
    private val imageSizes = LinkedHashMap<Int, Pair<Float, Float>>()
    private val scrollPositions = LinkedHashMap<Int, Pair<Double, Double>>()
    private val text = TextEngine(context) { requestPump() }
    private val presenter = Presenter(context, text, ::event,
        ::notifyTitle, { id, w, h -> imageSizes[id] = w to h }, ::requestFit, ::collectionFeedback, nativeFactory, { id, x, y -> scrollPositions[id] = x to y })
    private var handle = run { NativeEnvironment.configure(context); Native.create(text) }
    var onTitle: ((String) -> Unit)? = null
    internal var onBoot: (() -> Unit)? = null
    internal var onFirstDraw: (() -> Unit)? = null
    internal var observeTransaction: ((Long, Long) -> Unit)? = null
    internal var creationToFirstHostDrawNs: Long? = null
        private set
    internal val retainedNodes: Int get() = presenter.retainedNodes
    internal val materializedNodes: Int get() = presenter.materializedNodes
    internal fun geometry(testId: String): Presenter.Geometry? = presenter.geometry(testId)
    internal fun collectionInfo(testId: String): Presenter.CollectionInfo? = presenter.collectionInfo(testId)
    internal fun textGeometries(prefix: String): List<Presenter.Geometry> = presenter.textGeometries(prefix)
    internal fun groupInfo(): List<Presenter.GroupInfo> = presenter.groupInfo()
    internal fun accessibility(testId: String): Pair<android.view.accessibility.AccessibilityNodeProvider, Int>? = presenter.accessibility(testId)
    internal val startupReady: Boolean get() = booted && !closed && viewportValid && !fitQueued && !fitting && !applying
    fun navigateBack(): Boolean = !closed && booted && presenter.back()
    internal fun activate(testId: String): Boolean = presenter.activate(testId)
    internal fun actionView(testId: String): View? = presenter.actionView(testId)
    internal fun bridgeStats(): LongArray = Native.bridgeStats(handle)

    init {
        addView(presenter.root, LayoutParams(LayoutParams.MATCH_PARENT, LayoutParams.MATCH_PARENT))
    }
    private fun now() = SystemClock.uptimeMillis() + clockOffset
    private val fit = Runnable { fitViewport() }
    private val timer = Runnable {
        if (!closed && visible && booted) apply { Native.advance(handle, now()) }
    }
    private val frame = Choreographer.FrameCallback {
        frameQueued = false
        if (!closed && visible && booted) apply { Native.frame(handle, now()) }
    }
    private val pump = Runnable {
        pumping = false
        if (!closed && visible && booted) {
            pumpNeeded = false
            apply { Native.pump(handle, now()) }
        }
    }
    private fun requestPump() {
        if (closed) return
        pumpNeeded = true
        if (booted && visible && !pumping) { pumping = true; handler.post(pump) }
    }
    private fun event(id: Int, kind: Int, value: String?) {
        if (closed || !booted) return
        val deliver = { apply { Native.dispatch(handle, id, kind, value?.toByteArray(Charsets.UTF_8), now()) } }
        if (applying) pending.add(deliver) else deliver()
    }
    private fun collectionFeedback(bytes: ByteArray) {
        if (closed || !booted) return
        val deliver = { if (!closed) apply { Native.collectionFeedback(handle, bytes, now()) } }
        if (applying) pending.add(deliver) else deliver()
    }
    private fun notifyTitle(value: String) {
        val notify = { if (!closed) onTitle?.invoke(value); Unit }
        // Public callbacks may close the session or dispatch another turn. The
        // borrowed native output must be fully consumed before either happens.
        if (applying) pending.add(notify) else notify()
    }
    private fun apply(call: () -> ByteBuffer) {
        check(Looper.myLooper() == Looper.getMainLooper())
        check(!applying && !closed) { "reentrant or closed Android owner" }
        applying = true
        try {
            // The direct buffer lease ends at the next native call. All callbacks
            // caused by applying props are deferred until decoding has finished.
            val observer = observeTransaction
            val start = if (observer != null) System.nanoTime() else 0L
            // Scrolling stays entirely native. Coalesce its metadata and cross
            // JNI once only when an authored turn needs visible frame()/measure().
            if (scrollPositions.isNotEmpty()) {
                val positions = ByteBuffer.allocate(scrollPositions.size * 20).order(java.nio.ByteOrder.LITTLE_ENDIAN)
                for ((id, offset) in scrollPositions) positions.putInt(id).putDouble(offset.first).putDouble(offset.second)
                scrollPositions.clear()
                Native.scrolled(handle, positions.array())
            }
            val buffer = call()
            val committed = if (observer != null) System.nanoTime() else 0L
            schedule = BatchReader.apply(buffer, presenter)
            // Queries may overwrite the native output, so read faces only after
            // BatchReader has finished consuming the entire direct-buffer lease.
            presenter.resolveControls { id, kind ->
                val response = Native.controlQuery(handle, id, kind)
                val bytes = ByteArray(response.remaining()); response.get(bytes)
                org.json.JSONObject(String(bytes, Charsets.UTF_8))
            }
            observer?.invoke(committed - start, System.nanoTime() - committed)
            val current = schedule!!
            if (current.flags and 64 != 0) clockOffset = current.clock - SystemClock.uptimeMillis()
        } catch (error: Throwable) {
            presenter.abort()
            throw error
        } finally { applying = false }
        arm()
        if (imageSizes.isNotEmpty()) {
            val sizes = ByteBuffer.allocate(imageSizes.size * 12).order(java.nio.ByteOrder.LITTLE_ENDIAN)
            for ((id, size) in imageSizes) sizes.putInt(id).putFloat(size.first).putFloat(size.second)
            imageSizes.clear()
            apply { Native.intrinsics(handle, sizes.array()) }
        }
        while (pending.isNotEmpty() && !closed) pending.removeFirst().invoke()
    }
    private fun arm() {
        handler.removeCallbacks(timer)
        val current = schedule ?: return
        val frames = visible && current.flags and (2 or 16) != 0
        if (frames && !frameQueued) { frameQueued = true; choreographer.postFrameCallback(frame) }
        if (!frames && frameQueued) { choreographer.removeFrameCallback(frame); frameQueued = false }
        if (visible && !frames && current.flags and 129 == 129) {
            val delay = ceil(current.due - now()).toLong().coerceAtLeast(0)
            handler.postDelayed(timer, delay)
        }
        if (pumpNeeded) requestPump()
    }
    override fun onMeasure(widthMeasureSpec: Int, heightMeasureSpec: Int) {
        val w = MeasureSpec.getSize(widthMeasureSpec)
        val h = MeasureSpec.getSize(heightMeasureSpec)
        if (!closed && !applying && insetsReceived && w > 0 && h > 0 &&
            MeasureSpec.getMode(widthMeasureSpec) == MeasureSpec.EXACTLY &&
            MeasureSpec.getMode(heightMeasureSpec) == MeasureSpec.EXACTLY &&
            (!booted || fitQueued || w != fittedWidth || h != fittedHeight)) {
            // Insets and exact local bounds are available before Android measures
            // descendants. Boot and settle authored viewport policy now so this
            // traversal measures the real tree rather than an empty placeholder.
            fitViewport(w, h)
        }
        super.onMeasure(widthMeasureSpec, heightMeasureSpec)
    }
    override fun onSizeChanged(w: Int, h: Int, oldw: Int, oldh: Int) {
        super.onSizeChanged(w, h, oldw, oldh)
        fitViewport(w, h)
    }
    override fun dispatchDraw(canvas: android.graphics.Canvas) {
        super.dispatchDraw(canvas)
        if (booted && !firstPixel && !closed) {
            firstPixel = true
            creationToFirstHostDrawNs = System.nanoTime() - createdAt
            handler.post { if (!closed) apply { Native.painted(handle) } }
            handler.post { if (!closed) onFirstDraw?.invoke() }
        }
    }
    private fun preferences() {
        val dark = resources.configuration.uiMode and Configuration.UI_MODE_NIGHT_MASK == Configuration.UI_MODE_NIGHT_YES
        apply { Native.preferences(handle, if (dark) 16 else 0) }
        if (!closed) presenter.appearanceChanged()
    }
    override fun onConfigurationChanged(newConfig: Configuration) {
        super.onConfigurationChanged(newConfig)
        if (booted && !closed) preferences()
        requestApplyInsets()
    }
    private fun requestFit() {
        if (closed || fitQueued) return
        fitQueued = true
        if (!fitting) handler.post(fit)
    }
    @Suppress("DEPRECATION")
    override fun onApplyWindowInsets(insets: WindowInsets): WindowInsets {
        if (Build.VERSION.SDK_INT >= 30) {
            val bars = insets.getInsets(WindowInsets.Type.systemBars() or WindowInsets.Type.displayCutout())
            systemInsets.set(bars.left, bars.top, bars.right, bars.bottom)
            imeBottom = insets.getInsets(WindowInsets.Type.ime()).bottom
        } else {
            val stable = insets.stableInsets
            val cutout = insets.displayCutout
            systemInsets.set(maxOf(stable.left, cutout?.safeInsetLeft ?: 0),
                maxOf(stable.top, cutout?.safeInsetTop ?: 0),
                maxOf(stable.right, cutout?.safeInsetRight ?: 0),
                maxOf(stable.bottom, cutout?.safeInsetBottom ?: 0))
            imeBottom = if (insets.systemWindowInsetBottom > systemInsets.bottom) insets.systemWindowInsetBottom else 0
        }
        insetsReceived = true
        requestFit()
        // Exact applies insets to the one viewport; descendants retain kernel frames.
        return if (Build.VERSION.SDK_INT >= 30) WindowInsets.CONSUMED else insets.consumeSystemWindowInsets()
    }
    private fun fitViewport(w: Int = width, h: Int = height) {
        if (closed || w <= 0 || h <= 0) return
        if (applying || fitting) { requestFit(); return }
        fitting = true
        handler.removeCallbacks(fit)
        try {
            var passes = 0
            do {
                fitQueued = false
                fitViewportOnce(w, h)
                fittedWidth = w; fittedHeight = h
                passes++
            } while (fitQueued && !closed && passes < 3)
        } finally {
            fitting = false
            // A policy that continues changing still owes a real fit and draw.
            // Keep it pending rather than reporting a partially settled startup.
            if (fitQueued && !closed) handler.post(fit)
        }
    }
    private fun fitViewportOnce(w: Int, h: Int) {
        val cover = presenter.coversWindow
        val left = if (cover) 0 else systemInsets.left
        val top = if (cover) 0 else systemInsets.top
        val right = w - if (cover) 0 else systemInsets.right
        var bottom = h - if (cover) 0 else systemInsets.bottom
        val keyboard = presenter.resizesForKeyboard && imeBottom > systemInsets.bottom
        if (keyboard) bottom = minOf(bottom, h - imeBottom)
        val valid = right > left && bottom > top
        if (!valid && booted) {
            // Wait for different bounds/insets without polling an impossible
            // contained viewport or claiming that provisional content is ready.
            viewportValid = false
            return
        }
        // A small embedded view can fit only after boot reveals authored cover.
        // Use positive local bounds provisionally, then settle the actual policy.
        val next = if (valid) Rect(left, top, right, bottom) else Rect(0, 0, w, h)
        val safe = if (cover) Rect(systemInsets) else Rect()
        if (keyboard) safe.bottom = 0
        val resized = next.width() != viewport.width() || next.height() != viewport.height()
        if (next != viewport) {
            viewport.set(next)
            presenter.root.layoutParams = LayoutParams(next.width(), next.height()).apply {
                leftMargin = next.left; topMargin = next.top
            }
        }
        if (!booted) {
            viewportValid = false
            booted = true
            apply { Native.boot(handle, next.width() / scale, next.height() / scale, initialPressBytes) }
            if (closed) return
            preferences()
            if (closed) return
            onBoot?.invoke()
            if (closed) return
            requestFit() // Boot exposes policy; the bounded owner settles it before measure.
        } else {
            viewportValid = true
            if (safe != deliveredInsets) {
                deliveredInsets.set(safe)
                apply { Native.insets(handle, safe.top / scale, safe.right / scale, safe.bottom / scale, safe.left / scale) }
                if (closed) return
            }
            if (resized) apply { Native.resize(handle, next.width() / scale, next.height() / scale) }
        }
    }
    private val oldImeLayout = ViewTreeObserver.OnGlobalLayoutListener {
        if (Build.VERSION.SDK_INT < 30 && !closed) {
            val shown = Rect()
            getWindowVisibleDisplayFrame(shown)
            val location = IntArray(2)
            getLocationOnScreen(location)
            val occluded = (location[1] + height - shown.bottom).coerceAtLeast(0)
            val next = if (occluded > systemInsets.bottom) occluded else 0
            if (next != imeBottom) { imeBottom = next; requestFit() }
        }
    }
    override fun onAttachedToWindow() {
        super.onAttachedToWindow()
        if (Build.VERSION.SDK_INT < 30) viewTreeObserver.addOnGlobalLayoutListener(oldImeLayout)
        requestApplyInsets()
        setSessionVisible(hostVisible)
    }
    override fun onDetachedFromWindow() {
        viewTreeObserver.removeOnGlobalLayoutListener(oldImeLayout)
        visible = false
        handler.removeCallbacks(timer); handler.removeCallbacks(pump); pumping = false
        choreographer.removeFrameCallback(frame); frameQueued = false
        super.onDetachedFromWindow()
    }
    override fun onWindowVisibilityChanged(visibility: Int) {
        super.onWindowVisibilityChanged(visibility)
        setSessionVisible(hostVisible)
    }
    /** Stop display callbacks while the Activity is hidden; no periodic idle pump. */
    fun setSessionVisible(value: Boolean) {
        check(Looper.myLooper() == Looper.getMainLooper())
        hostVisible = value
        visible = value && isAttachedToWindow && windowVisibility == View.VISIBLE
        if (!visible) {
            handler.removeCallbacks(timer); handler.removeCallbacks(pump); pumping = false
            choreographer.removeFrameCallback(frame); frameQueued = false
        } else if (!closed) arm()
    }
    /** Explicit teardown, after executor retirement has synchronized native callbacks. */
    override fun close() {
        check(Looper.myLooper() == Looper.getMainLooper())
        if (closed) return
        if (applying) { pending.add { close() }; return }
        closed = true
        handler.removeCallbacks(timer); handler.removeCallbacks(pump); handler.removeCallbacks(fit)
        choreographer.removeFrameCallback(frame)
        pending.clear()
        imageSizes.clear()
        Native.close(handle)
        handle = 0
        presenter.close()
        text.close()
    }
}
