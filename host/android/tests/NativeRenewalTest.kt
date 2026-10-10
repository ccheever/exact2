package com.exact.android

import android.app.Activity
import android.content.Context
import android.graphics.Bitmap
import android.graphics.Canvas
import android.graphics.Color
import android.graphics.Rect
import android.graphics.drawable.BitmapDrawable
import android.os.Handler
import android.os.Looper
import android.os.SystemClock
import android.util.LruCache
import android.util.SparseArray
import android.view.MotionEvent
import android.view.View
import android.view.ViewGroup
import android.view.accessibility.AccessibilityNodeInfo
import android.widget.ImageView
import org.json.JSONArray
import org.json.JSONObject
import java.nio.ByteBuffer
import java.nio.ByteOrder
import kotlin.math.roundToInt

/** Real SDK carriers and text/pixel comparisons; invoke outside timing cohorts.
 * Cached images use deterministic real Bitmap leases, not a decoder mock. The
 * separate ImageDecoder probe verifies asset decoding; this tests owner renewal.
 */
internal object NativeRenewalTest {
    // Constant class/field references also survive the minified verification APK.
    private fun field(owner: Any, name: String): Any? = when (name) {
        "nodes" -> Presenter::class.java.getDeclaredField("nodes")
        "collections" -> Presenter::class.java.getDeclaredField("collections")
        "images" -> Presenter::class.java.getDeclaredField("images")
        "sources" -> TextEngine::class.java.getDeclaredField("sources")
        "rowGeometry" -> NativeCollections::class.java.getDeclaredField("rowGeometry")
        "cache" -> NativeImages::class.java.getDeclaredField("cache")
        "imageRequest" -> owner.javaClass.declaredFields.single { it.type == NativeImages.Request::class.java }
        else -> error("Unknown renewal probe field $name")
    }.let { it.isAccessible = true; it.get(owner) }
    private fun create(id: Int, kind: String, props: JSONObject, style: JSONObject = JSONObject(), handlers: List<String> = emptyList()) =
        JSONObject().put("op", "create").put("id", id).put("kind", kind)
            .put("props", props).put("style", style).put("handlers", JSONArray(handlers))
    private fun props(id: Int, values: JSONObject) = JSONObject().put("op", "props").put("id", id)
        .put("set", values).put("clear", JSONArray())
    private fun renew(vararg ids: Int) = JSONObject().put("op", "renew").put("ids", JSONArray(ids.toList()))
    private fun colors(color: Int) = JSONArray(listOf(Color.red(color), Color.green(color), Color.blue(color), Color.alpha(color)))
    private fun textRequest(body: String = "Same authored text"): ByteBuffer = ByteBuffer.allocateDirect(512).order(ByteOrder.LITTLE_ENDIAN).apply {
        putInt(1); putInt(2); putInt(2); putInt(1); putLong(1)
        putFloat(190.17f); putFloat(-1f)
        repeat(6) { putInt(0) }; putInt(1); putInt(0)
        fun style() { putFloat(24f); putInt(400); putInt(0); putInt(0); putInt(0); putFloat(0f); putFloat(0f); putInt(0) }
        style(); style()
        val bytes = body.toByteArray(Charsets.UTF_8)
        putInt(bytes.size); put(bytes); flip()
    }
    private class Fixture(val context: Context) : AutoCloseable {
        val text = TextEngine(context) {}
        val events = ArrayList<Int>()
        val natural = ArrayList<Pair<Float, Float>>()
        val pending = PendingIntrinsics<Any>()
        lateinit var presenter: Presenter
            private set
        private val container = (context as? Activity)?.findViewById<ViewGroup>(android.R.id.content)
        private val scale = context.resources.displayMetrics.density
        init {
            presenter = Presenter(context, text, { _, kind, _ -> events.add(kind) }, {}, { id, w, h ->
                natural.add(w to h)
                presenter.intrinsicOwner(id)?.let { pending.put(id, it, w, h) }
            }, {}, {}, null, { _, _, _ -> })
            container?.addView(presenter.root, ViewGroup.LayoutParams((260 * scale).roundToInt(), (220 * scale).roundToInt()))
        }
        fun commit(vararg ops: JSONObject) {
            presenter.begin()
            try { for (op in ops) presenter.cold(op); presenter.finish(); presenter.resolveControls { _, _ -> JSONObject() } }
            catch (failure: Throwable) { presenter.abort(); throw failure }
            layout()
        }
        fun layout() {
            val width = (260 * scale).roundToInt(); val height = (220 * scale).roundToInt()
            presenter.root.measure(View.MeasureSpec.makeMeasureSpec(width, View.MeasureSpec.EXACTLY), View.MeasureSpec.makeMeasureSpec(height, View.MeasureSpec.EXACTLY))
            presenter.root.layout(0, 0, width, height)
        }
        fun populate(buttonTag: String = "old-button", image: String? = null, flat: Boolean = false) {
            text.measure(textRequest(), textRequest().limit())
            presenter.begin()
            presenter.cold(create(1, "view", JSONObject().put("id", "row").put("testId", "row"),
                JSONObject().put("background_color", colors(Color.WHITE))))
            presenter.cold(create(2, "text", JSONObject().put("text", "Same authored text").put("testId", "text"),
                JSONObject().put("text_color", colors(Color.BLUE)).put("text_decoration_line", "underline")))
            if (!flat) presenter.cold(create(3, "button", JSONObject().put("id", "authored-button").put("testId", buttonTag).put("accessibilityLabel", "Authored label"),
                JSONObject().put("background_color", colors(Color.GREEN)).put("border_radius_top_left", 4), listOf("press", "hover", "key")))
            if (image != null) presenter.cold(create(4, "image", JSONObject().put("testId", "image").put("imageSource", image), JSONObject().put("object_fit", "fill")))
            val ids = mutableListOf(2); if (!flat) ids.add(3); if (image != null) ids.add(4)
            presenter.cold(JSONObject().put("op", "children").put("id", 1).put("ids", JSONArray(ids)))
            presenter.cold(JSONObject().put("op", "roots").put("ids", JSONArray(listOf(1))))
            presenter.frame(1, 0f, 0f, 241.17f, 197.29f)
            presenter.frame(2, 8.17f, 4.29f, 190.17f, 39.29f)
            if (!flat) presenter.frame(3, 8.17f, 60.29f, 117.17f, 49.29f)
            if (image != null) presenter.frame(4, 8.17f, 120.29f, 72.31f, 52.19f)
            presenter.finish(); presenter.resolveControls { _, _ -> JSONObject() }; layout()
        }
        fun imageView() = (presenter.actionView("image") as ViewGroup).getChildAt(0) as ImageView
        fun pixels(): IntArray {
            layout()
            val bitmap = Bitmap.createBitmap(presenter.root.width, presenter.root.height, Bitmap.Config.ARGB_8888)
            try {
                presenter.root.draw(Canvas(bitmap))
                return IntArray(bitmap.width * bitmap.height).also { bitmap.getPixels(it, 0, bitmap.width, 0, 0, bitmap.width, bitmap.height) }
            } finally { bitmap.recycle() }
        }
        @Suppress("UNCHECKED_CAST") fun node(id: Int): Any = (field(presenter, "nodes") as SparseArray<Any>)[id]
        fun source(): Any? = (field(text, "sources") as SparseArray<*>)[2]
        fun imageRequest() = field(node(4), "imageRequest") as NativeImages.Request
        @Suppress("UNCHECKED_CAST") fun seed(source: String, bitmap: Bitmap) {
            val images = field(presenter, "images") as NativeImages
            val constructor = Class.forName("com.exact.android.NativeImages\$Key")
                .getDeclaredConstructor(String::class.java, Integer.TYPE, Integer.TYPE, String::class.java)
                .apply { isAccessible = true }
            val key = constructor.newInstance(source, (72.31f * scale).roundToInt(), (52.19f * scale).roundToInt(), "fill")
            (field(images, "cache") as LruCache<Any, NativeImages.Image>).put(key, NativeImages.Image(bitmap, bitmap.width, bitmap.height))
        }
        override fun close() { container?.removeView(presenter.root); presenter.close(); text.close() }
    }
    private fun fails(message: String, action: () -> Unit) {
        check(runCatching(action).exceptionOrNull() is IllegalArgumentException) { message }
    }
    fun runEmptyCollectionBatch(context: Context): String {
        fun packet(flags: Int = 64, metadata: String = "", invalidate: Boolean = false): ByteBuffer {
            val bytes = metadata.toByteArray(Charsets.UTF_8)
            return ByteBuffer.allocate(32 + (if (invalidate) 9 else 0) + bytes.size).order(ByteOrder.LITTLE_ENDIAN).apply {
                putInt(0x31415845); putShort(1); putShort(flags.toShort()); putInt(if (invalidate) 1 else 0)
                putDouble(123.0); putDouble(456.0); putInt(bytes.size)
                if (invalidate) { put(6); putInt(4); putInt(2) }
                put(bytes); flip()
            }
        }
        val fixture = Fixture(context)
        try {
            fixture.populate()
            val before = fixture.pixels()
            val source = fixture.source()
            for (flags in listOf(0, 64, 64 or 1 or 128, 64 or 2 or 4 or 16)) {
                val result = BatchReader.apply(packet(flags, "{\"seq\":[1,2]}"), fixture.presenter, skipEmpty = true)
                check(!result.presented && result.flags == flags && result.clock == 123.0 && result.due == 456.0)
                check(result.metadata!!.getJSONArray("seq").length() == 2)
                check(fixture.source() === source && before.contentEquals(fixture.pixels()))
            }
            // Ordinary event replies must still reconcile SDK control state.
            check(BatchReader.apply(packet(), fixture.presenter).presented)
            fixture.presenter.resolveControls { _, _ -> JSONObject() }
            check(BatchReader.apply(packet(invalidate = true), fixture.presenter, skipEmpty = true).presented)
            fixture.presenter.resolveControls { _, _ -> JSONObject() }
            check(before.contentEquals(fixture.pixels()))
            val empty = Fixture(context)
            try {
                check(BatchReader.apply(packet(flags = 256), empty.presenter, skipEmpty = true).presented)
                empty.presenter.resolveControls { _, _ -> JSONObject() }
            } finally { empty.close() }
            fails("truncated reply escaped empty handling") { BatchReader.apply(packet().apply { limit(31) }, fixture.presenter, true) }
            fails("trailing bytes escaped empty handling") { BatchReader.apply(ByteBuffer.allocate(33).put(packet()).put(0).apply { flip() }, fixture.presenter, true) }
            check(runCatching { BatchReader.apply(packet(metadata = "{\"error\":\"expected rejection\"}"), fixture.presenter, true) }.isFailure)
            check(runCatching { BatchReader.apply(packet(flags = 8), fixture.presenter, true) }.isFailure)
            check(fixture.source() === source && before.contentEquals(fixture.pixels()))
            return "EmptyCollectionBatchTest: PASS (schedule and metadata, untouched pixels/owners, ordinary events, control updates, operations, malformed/error/canvas rejection)"
        } finally { fixture.close() }
    }
    @Suppress("UNCHECKED_CAST") private fun runCacheBudget(context: Context) {
        val images = NativeImages(context)
        val cache = field(images, "cache") as LruCache<Any, NativeImages.Image>
        val constructor = Class.forName("com.exact.android.NativeImages\$Key")
            .getDeclaredConstructor(String::class.java, Integer.TYPE, Integer.TYPE, String::class.java)
            .apply { isAccessible = true }
        val leases = ArrayList<NativeImages.Image>()
        try {
            images.viewport(2560, 2560)
            for ((index, color) in listOf(Color.RED, Color.GREEN, Color.BLUE).withIndex()) {
                val bitmap = Bitmap.createBitmap(2048, 2048, Bitmap.Config.ARGB_8888)
                bitmap.eraseColor(color)
                val image = NativeImages.Image(bitmap, bitmap.width, bitmap.height)
                image.retain(); leases.add(image)
                cache.put(constructor.newInstance("cache-lease-$index", 2048, 2048, "fill"), image)
            }
            check(cache.size() == 48 * 1024 * 1024) { "viewport budget did not retain three images" }
            // A smaller owning viewport evicts the oldest cache lease; its
            // visible ImageView lease must still keep the exact pixels alive.
            images.viewport(0, 0)
            check(cache.size() == 32 * 1024 * 1024)
            check(!leases[0].bitmap.isRecycled && leases[0].bitmap.getPixel(0, 0) == Color.RED)
            val oldest = leases.removeAt(0); oldest.release()
            check(oldest.bitmap.isRecycled) { "evicted image survived its last owner" }
            images.trim()
            check(cache.size() == 0) { "background trim retained cached pixels" }
            leases.forEachIndexed { index, image ->
                image.retain()
                cache.put(constructor.newInstance("close-lease-$index", 2048, 2048, "fill"), image)
            }
            images.close()
            check(cache.size() == 0) { "closed cache retained images" }
            for ((image, color) in leases.zip(listOf(Color.GREEN, Color.BLUE))) {
                check(!image.bitmap.isRecycled && image.bitmap.getPixel(0, 0) == color)
            }
            while (leases.isNotEmpty()) {
                val image = leases.removeAt(0); image.release()
                check(image.bitmap.isRecycled) { "closed cache retained pixels after the last view" }
            }
        } finally { images.close(); leases.forEach { it.release() } }
    }
    @Suppress("UNCHECKED_CAST") fun runSessionVisibility(view: ExactView): String {
        val presenter = ExactView::class.java.getDeclaredField("presenter")
            .apply { isAccessible = true }.get(view) as Presenter
        val images = field(presenter, "images") as NativeImages
        val cache = field(images, "cache") as LruCache<Any, NativeImages.Image>
        val constructor = Class.forName("com.exact.android.NativeImages\$Key")
            .getDeclaredConstructor(String::class.java, Integer.TYPE, Integer.TYPE, String::class.java)
            .apply { isAccessible = true }
        val bitmap = Bitmap.createBitmap(2, 2, Bitmap.Config.ARGB_8888).apply { eraseColor(Color.RED) }
        val image = NativeImages.Image(bitmap, 2, 2)
        image.retain()
        try {
            cache.put(constructor.newInstance("session-visibility-lease", 2, 2, "fill"), image)
            view.setSessionVisible(false)
            check(cache.size() == 0 && !bitmap.isRecycled && bitmap.getPixel(0, 0) == Color.RED)
        } finally { view.setSessionVisible(true); image.release() }
        check(bitmap.isRecycled)
        return "SessionImageVisibilityTest: PASS (hidden session drops cache while preserving active pixels)"
    }
    fun run(context: Context): String {
        check(Looper.myLooper() == Looper.getMainLooper())
        runCacheBudget(context)
        val renewed = Fixture(context); val fresh = Fixture(context); val flat = Fixture(context)
        try {
            renewed.populate(); fresh.populate("new-button"); flat.populate(flat = true)
            val p = renewed.presenter
            val button = p.actionView("old-button") as Presenter.Box
            val owner = p.intrinsicOwner(3)
            val oldContact = checkNotNull(button.touchDispatch)
            val textSource = renewed.source()
            val beforeMetrics = List(3) { renewed.text.measure(textRequest(), textRequest().limit()).getFloat(it * 4) }
            p.present(3, 1, 11.0, 9.0, 0.0, 0.0); p.present(3, 2, .8, 0.0, 0.0, 0.0)
            p.present(3, 3, 17.0, 0.0, 0.0, 0.0); p.present(3, 4, .25, 0.0, 0.0, 0.0); p.finish()
            button.isPressed = true; button.isHovered = true; button.isSelected = true; button.isActivated = true
            button.translationX = 13f; button.rotationX = 11f
            renewed.commit(renew(1, 2, 3), props(3, JSONObject().put("testId", "new-button")))
            check(p.actionView("new-button") === button && p.actionView("old-button") == null) { "carrier/testId renewal failed" }
            check(owner !== p.intrinsicOwner(3) && textSource === renewed.source()) { "owner or text metric lease failed" }
            check(List(3) { renewed.text.measure(textRequest(), textRequest().limit()).getFloat(it * 4) } == beforeMetrics)
            check(button.tag == "new-button" && button.contentDescription == "Authored label" && button.isEnabled)
            check(button.animationMatrix == null && button.alpha == 1f && button.translationX == 0f && button.rotationX == 0f)
            check(!button.isPressed && !button.isHovered && !button.isSelected && !button.isActivated)
            check(renewed.pixels().contentEquals(fresh.pixels())) { "fresh vs renewed authored text/style pixels differ" }
            check(p.activate("new-button") && renewed.events == listOf(0)) { "unchanged press handler was lost or duplicated" }
            // The kernel measures before publishing renew. Do not erase that
            // newly prepared Source just because its logical owner changed.
            val changed = textRequest("Next measured row")
            renewed.text.measure(changed, changed.limit())
            val preparedSource = renewed.source()
            fresh.text.measure(textRequest("Next measured row"), textRequest("Next measured row").limit())
            renewed.commit(renew(2), props(2, JSONObject().put("text", "Next measured row")))
            fresh.commit(props(2, JSONObject().put("text", "Next measured row")))
            check(preparedSource === renewed.source() && renewed.text.paragraphText(2) == "Next measured row")
            check(renewed.pixels().contentEquals(fresh.pixels())) { "premeasured changed text differs from fresh row" }
            // Binary PAINT does not rewrite the immutable cold style JSON.
            // Renew-only must preserve its current authored color channels.
            val paint = ByteBuffer.allocate(16).order(ByteOrder.LITTLE_ENDIAN)
                .putInt(Color.YELLOW).putInt(Color.YELLOW).putInt(Color.RED).putInt(Color.RED)
            paint.flip(); p.paint(2, 3, paint); p.finish()
            fresh.commit(JSONObject().put("op", "style").put("id", 2).put("style", JSONObject()
                .put("background_color", colors(Color.YELLOW)).put("text_color", colors(Color.RED)).put("text_decoration_line", "underline")))
            repeat(2) {
                renewed.commit(renew(2))
                check(renewed.pixels().contentEquals(fresh.pixels())) { "renew-only restored stale cold paint" }
            }
            val collections = field(p, "collections")!!
            @Suppress("UNCHECKED_CAST") val rowGeometry = field(collections, "rowGeometry") as (Int) -> NativeCollections.RowBox?
            val row = checkNotNull(rowGeometry(1)); val geometry = checkNotNull(p.geometry("row"))
            check(row.main == 197.29f.toDouble() && row.cross == 241.17f.toDouble()) { "fractional row dimensions were rounded" }
            check(row.start == geometry.frame.top / context.resources.displayMetrics.density.toDouble())
            val down = MotionEvent.obtain(0, 0, MotionEvent.ACTION_DOWN, 1f, 1f, 0)
            try {
                oldContact(down)
                renewed.commit(renew(3)) // Old contact must not pin the new incarnation.
                checkNotNull(button.touchDispatch)(down)
                fails("live touch owner was renewable") { renewed.commit(renew(3)) }
            } finally { down.recycle() }
            val fp = flat.presenter
            val provider = checkNotNull(fp.accessibility("text"))
            check(provider.first.performAction(2, AccessibilityNodeInfo.ACTION_ACCESSIBILITY_FOCUS, null))
            check(provider.first.findFocus(AccessibilityNodeInfo.FOCUS_ACCESSIBILITY) != null)
            flat.commit(renew(2), props(2, JSONObject().put("testId", "new-text").put("accessibilityLabel", "Next owner")))
            check(provider.first.findFocus(AccessibilityNodeInfo.FOCUS_ACCESSIBILITY) == null) { "virtual focus survived incarnation" }
            val info = checkNotNull(checkNotNull(fp.accessibility("new-text")).first.createAccessibilityNodeInfo(2))
            check(info.text == "Same authored text" && info.contentDescription == "Next owner")
            info.recycle()
            // Preflight is atomic: a later forbidden carrier cannot retire an earlier safe one.
            fresh.commit(create(5, "scroll", JSONObject()))
            val untouched = fresh.presenter.intrinsicOwner(3)
            fails("forbidden scroll renewed") { fresh.commit(renew(3, 5)) }
            check(fresh.presenter.intrinsicOwner(3) === untouched)
            fails("duplicate renewal accepted") { fresh.commit(renew(3, 3)) }
            for (kind in listOf("input", "textarea", "native", "control", "list", "scroll", "web", "canvas", "video", "svg"))
                fails("stateful $kind admitted") { NativeRenewal.requireSupported(kind, false, false) }
            fails("navigation carrier admitted") { NativeRenewal.requireSupported("view", false, true) }
            return "NativeRenewalTest: PASS (cache resize/trim/close pixel leases, fresh pixels, unchanged/premeasured content metrics, hot paint, authored props/style/handlers, SDK state, testId, owner/touch, virtual accessibility, atomic refusal, fractional RowBox)"
        } finally { renewed.close(); fresh.close(); flat.close() }
    }
    /** Allow real posted SDK focus/click and cached image callbacks to run. */
    fun runAsync(context: Context, complete: (String) -> Unit) {
        check(Looper.myLooper() == Looper.getMainLooper())
        val handler = Handler(Looper.getMainLooper())
        val fixture = Fixture(context); var fresh: Fixture? = null
        val a = Bitmap.createBitmap(7, 5, Bitmap.Config.ARGB_8888).apply { eraseColor(Color.RED) }
        val b = Bitmap.createBitmap(9, 13, Bitmap.Config.ARGB_8888).apply { eraseColor(Color.BLUE) }
        var finished = false
        fun finish(result: String) {
            if (finished) return
            finished = true; fixture.close(); fresh?.close(); complete(result)
        }
        fun phase(action: () -> Unit) {
            if (finished) return
            try { action() } catch (failure: Throwable) { finish("NativeRenewalTest async: FAIL (${failure.stackTraceToString()})") }
        }
        handler.postDelayed({ finish("NativeRenewalTest async: FAIL (callbacks did not settle)") }, 8000)
        phase {
            fixture.seed("renew-a.png", a); fixture.seed("renew-b.png", b); fixture.populate(image = "renew-a.png")
            val p = fixture.presenter
            val button = checkNotNull(p.actionView("old-button"))
            button.isFocusableInTouchMode = true
            p.root.requestFocus()
            var focusEvents = 0
            button.setOnFocusChangeListener { _, focused -> if (focused) focusEvents++ }
            handler.post { phase {
                check((fixture.imageView().drawable as BitmapDrawable).bitmap === a)
                fixture.pending.drain(p::intrinsicOwner)
                val initialOwner = p.intrinsicOwner(4)
                fixture.commit(renew(4))
                check((fixture.imageView().drawable as BitmapDrawable).bitmap === a) { "same-source pixels blinked" }
                check(initialOwner !== p.intrinsicOwner(4))
                val request = fixture.imageRequest(); val stale = checkNotNull(request.deliver)
                val oldOwner = checkNotNull(p.intrinsicOwner(4))
                fixture.pending.put(4, oldOwner, 7f, 5f)
                fixture.commit(JSONObject().put("op", "command").put("name", "focus").put("args", JSONArray(listOf(3))))
                // SDK PerformClick is posted by ACTION_UP and must be cancelled.
                // A real focus-taking click must remain pinned. Make only
                // this synthetic SDK click non-focus-taking; the old posted
                // focus remains pending and will be checked after renewal.
                button.isFocusableInTouchMode = false
                val now = SystemClock.uptimeMillis()
                for (action in intArrayOf(MotionEvent.ACTION_DOWN, MotionEvent.ACTION_UP)) {
                    val event = MotionEvent.obtain(now, now, action, 1f, 1f, 0)
                    try { button.onTouchEvent(event) } finally { event.recycle() }
                }
                button.isFocusableInTouchMode = true
                p.root.requestFocus()
                check(!button.hasFocus()) { "synthetic input acquired a live focus pin" }
                fixture.commit(renew(3, 4), props(3, JSONObject().put("testId", "new-button")), props(4, JSONObject().put("imageSource", "renew-b.png")))
                check(request.cancelled && request.deliver == null && fixture.pending.drain(p::intrinsicOwner) == null)
                stale(NativeImages.Image(a, 7, 5)) // A late old callback must fail even if invoked after cancellation.
                check(fixture.imageView().drawable == null && fixture.pending.drain(p::intrinsicOwner) == null)
                handler.post { phase {
                    check(!button.hasFocus() && focusEvents == 0) { "old posted focus reached renewed owner" }
                    check(fixture.events.isEmpty()) { "old posted SDK click reached renewed owner" }
                    check((fixture.imageView().drawable as BitmapDrawable).bitmap === b)
                    val natural = checkNotNull(fixture.pending.drain(p::intrinsicOwner))
                    val sizes = ByteBuffer.wrap(natural).order(ByteOrder.LITTLE_ENDIAN)
                    check(sizes.int == 4 && sizes.float == 9f && sizes.float == 13f && !sizes.hasRemaining())
                    fresh = Fixture(context).also { it.seed("renew-b.png", b); it.populate("new-button", "renew-b.png") }
                    val ownerB = checkNotNull(p.intrinsicOwner(4))
                    fixture.pending.put(4, ownerB, 9f, 13f)
                    fixture.commit(renew(4))
                    check((fixture.imageView().drawable as BitmapDrawable).bitmap === b)
                    check(fixture.pending.drain(p::intrinsicOwner) == null)
                    handler.post { phase {
                        check(fixture.pending.drain(p::intrinsicOwner) != null) { "cached size was not re-reported for new owner" }
                        check(fixture.pixels().contentEquals(checkNotNull(fresh).pixels())) { "changed-source fresh/renewed image pixels differ" }
                        finish("NativeRenewalTest async: PASS (real Looper focus/click cancellation, same-source pixels, changed/cached images, stale callbacks/intrinsics, fresh image pixels)")
                    } }
                } }
            } }
        }
    }
}
