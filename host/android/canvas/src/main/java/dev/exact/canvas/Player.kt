package dev.exact.canvas

import android.graphics.Bitmap
import android.graphics.Canvas
import android.graphics.Matrix
import android.graphics.Paint
import android.graphics.Path
import android.graphics.RectF
import android.graphics.fonts.Font
import java.nio.IntBuffer

/**
 * Whether a stroked polyline is drawn as lines and discs ([Player.strokePolyline]): where HWUI rasterizes paths on the
 * CPU (PowerVR's Vulkan: no multisampled path rendering). Elsewhere it stays a path, which the GPU strokes for less
 * CPU than the lines take (Mali, crypto's fling: lines 22 MB less and 10% more CPU). poly=1|0 to compare.
 */
/** Small paths kept as pictures ([Player.icon]): a symbol's fill or stroke, the same in every row. On PowerVR HWUI has
 * no GPU path for them: each was stroked and filled into a mask on RenderThread and its workers, for every row that
 * came (easy on a Pixel 10 Pro XL at 12k dp/s: 30 ms/s of hwuiTask the Views app, which draws its vector drawables
 * from a bitmap, does not have). icons=0|1 to compare. */
internal object Icons {
    @JvmField var on = Polylines.on
    const val MAX = 160
    const val CAP = 256
    data class Key(val a: Long, val b: Long, val n: Int)
    class Entry(val bmp: Bitmap, val dx: Float, val dy: Float)
    val map = java.util.concurrent.ConcurrentHashMap<Key, Entry>()
}

internal object Polylines {
    @JvmField var mesh = true
    @JvmField var on = try {
        Class.forName("android.os.SystemProperties").getMethod("get", String::class.java).invoke(null, "ro.hardware.vulkan") == "powervr"
    } catch (e: Exception) { false }
}

/** Picture [id] (announced by IMAGE_DEF, [w]×[h]) as a bitmap: with [hwbuf], straight into a GPU buffer wrapped as a
 * hardware bitmap (as an image loader's decode-to-hardware bitmap is: no heap copy, no upload on the RenderThread). */
internal fun makePicture(id: Int, w: Int, h: Int, hwbuf: Boolean): Bitmap {
    if (hwbuf) {
        // The picture was decoded into a GPU buffer: wrap that one (no copy at all).
        val own = try { Native.imageBuffer(id) } catch (e: UnsatisfiedLinkError) { null }
        if (own != null) {
            val bmp = Bitmap.wrapHardwareBuffer(own, android.graphics.ColorSpace.get(android.graphics.ColorSpace.Named.SRGB))
            own.close()
            if (bmp != null) return bmp
        }
        val hb = android.hardware.HardwareBuffer.create(w, h, android.hardware.HardwareBuffer.RGBA_8888, 1,
            android.hardware.HardwareBuffer.USAGE_GPU_SAMPLED_IMAGE or android.hardware.HardwareBuffer.USAGE_CPU_WRITE_OFTEN)
        val ok = try { Native.copyImageToBuffer(id, hb) } catch (e: UnsatisfiedLinkError) { false }
        val bmp = if (ok) Bitmap.wrapHardwareBuffer(hb, android.graphics.ColorSpace.get(android.graphics.ColorSpace.Named.SRGB)) else null
        hb.close()
        if (bmp != null) return bmp
    }
    val bmp = Bitmap.createBitmap(w, h, Bitmap.Config.ARGB_8888)
    Native.copyImage(id, bmp)
    bmp.prepareToDraw()
    return bmp
}

/**
 * Replays an op stream (exact2's host/linux/src/canvas.rs) onto a Canvas. Its scratch objects are its own: one
 * Player a thread. [image] and [row] resolve ids; [group] gives a scroller's node (main thread only).
 */
internal class Player(private val fonts: Map<Int, Font>) {
    var image: (Int) -> Bitmap? = { null }
    /** A platform element's node (op 28, [Natives]). */
    var native: (Int) -> android.graphics.RenderNode? = { null }
    /** A picture's animated drawable (op 26), drawn in its still's place; HWUI plays it on the RenderThread. */
    var animated: (Int) -> android.graphics.drawable.Drawable? = { null }
    private val bounds = android.graphics.Rect()
    var row: (Int) -> android.graphics.RenderNode? = { null }
    var slot: (Int) -> android.graphics.RenderNode? = { null }
    /** A node exact2's reader animates (op 29, [Layers]). */
    var layer: (Int) -> android.graphics.RenderNode? = { null }
    var group: ((Int) -> android.graphics.RenderNode)? = null
    /** What is drawn in place of a scroller's own node, for one whose edge the reader stretches ([Scrolling]). */
    var holder: ((Int) -> android.graphics.RenderNode?)? = null
    private var grouped = 0
    var width = 0
    var height = 0
    private val fill = Paint(Paint.ANTI_ALIAS_FLAG)
    private val glyphPaint = Paint(Paint.ANTI_ALIAS_FLAG)
    private val imagePaint = Paint(Paint.FILTER_BITMAP_FLAG or Paint.ANTI_ALIAS_FLAG)
    private val stroke = Paint(Paint.ANTI_ALIAS_FLAG).apply { style = Paint.Style.STROKE }
    private val src = RectF()
    private val dstRect = RectF()
    private val matrix = Matrix()
    private val values = FloatArray(9)
    private val rect = RectF()
    private val radii = FloatArray(8)
    private val inner = RectF()
    private val innerRadii = FloatArray(8)
    private val strip = RectF()
    private val path = Path()
    private var glyphIds = IntArray(256)
    private var positions = FloatArray(512)

    /** Device pixels the stream's matrices are moved by (a backdrop's node is placed at its bounds). */
    private var shiftX = 0f
    private var shiftY = 0f
    /** The stream's Canvas matrix, including clip and opacity restores. */
    private val current = Matrix()
    private val savedMatrices = ArrayList<Matrix>()
    private var matrixDepth = 0
    private fun saveMatrix() {
        if (matrixDepth == savedMatrices.size) savedMatrices.add(Matrix())
        savedMatrices[matrixDepth++].set(current)
    }
    private fun restoreMatrix() { current.set(savedMatrices[--matrixDepth]) }
    private val backdropBounds = RectF()

    /** [anim] over [bounds]: drawn at the origin, under a translation, with bounds of its size alone. Android 10's
     * `AnimatedImageDrawable` drew a strip of its frames, or nothing, at bounds away from the origin. */
    private fun drawAnimated(c: Canvas, anim: android.graphics.drawable.Drawable) {
        c.save(); c.translate(bounds.left.toFloat(), bounds.top.toFloat())
        anim.setBounds(0, 0, bounds.width(), bounds.height()); anim.draw(c)
        c.restore()
    }

    /**
     * A material's backdrop (op 27 at [at]): what this replay drew from [from] up to it, again, into a node at the
     * blur's device bounds with a blur effect, drawn within the rounded rect. HWUI keeps the blurred layer until the
     * row is recorded again.
     */
    /** Op 27 (one blur) or 42 (ordered functions: kind 0 blur, 1 saturation), over what [b] drew from [from] to [at]. */
    private fun backdrop(c: Canvas, b: IntBuffer, from: Int, at: Int) {
        // No backdrop filter before RenderEffect (Android 12): what is behind shows unfiltered.
        if (android.os.Build.VERSION.SDK_INT < 31) return
        val ordered = b.get(at) == 42
        val count = if (ordered) b.get(at + 1) else 1
        readRRect(b, if (ordered) at + 2 + 2 * count else at + 2)
        val scale = Math.hypot(values[0].toDouble(), values[3].toDouble()).toFloat()
        var effect: android.graphics.RenderEffect? = null
        var sigmaPx = 0f
        for (k in 0 until count) {
            val kind = if (ordered) b.get(at + 2 + 2 * k) else 0
            val amount = if (ordered) f(b, at + 3 + 2 * k) else f(b, at + 1)
            val next = if (kind == 0) {
                val s = amount * scale
                if (s <= 0f) continue
                sigmaPx = Math.hypot(sigmaPx.toDouble(), s.toDouble()).toFloat()
                // Skia's blur takes a radius: sigma = 0.57735 r + 0.5.
                val radius = ((s - 0.5f) / 0.57735f).coerceAtLeast(0.5f)
                android.graphics.RenderEffect.createBlurEffect(radius, radius, android.graphics.Shader.TileMode.CLAMP)
            } else {
                val m = android.graphics.ColorMatrix(); m.setSaturation(amount)
                android.graphics.RenderEffect.createColorFilterEffect(android.graphics.ColorMatrixColorFilter(m))
            }
            effect = effect?.let { android.graphics.RenderEffect.createChainEffect(next, it) } ?: next
        }
        if (effect == null) return
        val here = Matrix(current)
        here.mapRect(backdropBounds, rect)
        val pad = 3f * sigmaPx
        val l = Math.floor((backdropBounds.left - pad).toDouble()).toFloat(); val t = Math.floor((backdropBounds.top - pad).toDouble()).toFloat()
        val node = android.graphics.RenderNode("exact backdrop")
        node.setPosition(l.toInt(), t.toInt(), Math.ceil((backdropBounds.right + pad).toDouble()).toInt(), Math.ceil((backdropBounds.bottom + pad).toDouble()).toInt())
        node.setClipToBounds(true)
        node.setRenderEffect(effect)
        val savedRadii = radii.copyOf(); val savedRect = RectF(rect)
        val sx = shiftX; val sy = shiftY
        shiftX = sx + l; shiftY = sy + t
        val inner = node.beginRecording()
        replay(inner, b, from, at)
        node.endRecording()
        shiftX = sx; shiftY = sy
        current.set(here)
        // The replay moved this canvas's matrix on its own canvas only; this one is as the op found it.
        c.save()
        c.setMatrix(here)
        path.rewind(); path.addRoundRect(savedRect, savedRadii, Path.Direction.CW); c.clipPath(path)
        // Back to this canvas's own coordinates, where the node sits at (l, t).
        val inv = Matrix(); here.invert(inv)
        c.concat(inv)
        c.drawRenderNode(node)
        c.restore()
    }

    /** A freed picture: nothing of it is kept here. */
    fun forget(bmp: Bitmap) {}

    /**
     * An opacity group (op 7) as cheaply as it draws the same. A layer saved with no bounds is as large as the clip
     * around it, whatever it draws: a full-screen scrim at 40 % (one rect) held two window-sized offscreen surfaces,
     * 18 MB, for as long as it showed. One fill takes the opacity in its colour ([tint]); anything else is saved over
     * the bounds of what it draws when [Bounds] can tell them.
     */
    private var tint = 1f
    private var tintEnd = -1
    private val walker = Structure()
    private val measurer = Bounds()
    private val layerRect = RectF()
    private val layerMatrix = Matrix()

    /** The RESTORE that closes the group whose content starts at [from], or -1. */
    private fun layerEnd(b: IntBuffer, from: Int, n: Int): Int {
        var depth = 0
        var i = from
        while (i < n) {
            when (b.get(i)) {
                4, 5, 7 -> depth++
                6 -> { if (depth == 0) return i; depth-- }
                20, 21 -> return -1
            }
            i = walker.end(b, i)
        }
        return -1
    }

    /** Whether [from, to) is one rect fill (op 2) and matrices. */
    private fun singleFill(b: IntBuffer, from: Int, to: Int): Boolean {
        var fills = 0
        var i = from
        while (i < to) {
            when (b.get(i)) { 1 -> {}; 2 -> fills++; else -> return false }
            i = walker.end(b, i)
        }
        return fills == 1
    }

    /** Whether [from, to) is one filled shape (a rect, op 2, or a path, op 3) and matrices: one draw, which cannot
     * overlap itself. */
    fun singleShape(b: IntBuffer, from: Int, to: Int): Boolean {
        var fills = 0
        var i = from
        while (i < to) {
            when (b.get(i)) { 1 -> {}; 2, 3 -> fills++; else -> return false }
            i = walker.end(b, i)
        }
        return fills == 1
    }

    /** What [from, to) draws, in the canvas's coordinates now (for saveLayer), or null when unknown. */
    private fun layerBounds(b: IntBuffer, from: Int, to: Int): RectF? {
        if (to < 0) return null
        // The stream's matrices are absolute device pixels, moved by the shift: measured there, mapped back.
        layerMatrix.set(current); layerMatrix.postTranslate(shiftX, shiftY)
        if (!measurer.measure(b, from, to, layerRect, layerMatrix) || layerRect.isEmpty) return null
        layerRect.offset(-shiftX, -shiftY)
        if (!current.invert(layerMatrix)) return null
        layerMatrix.mapRect(layerRect)
        layerRect.inset(-1f, -1f)
        return layerRect
    }

    /** [replay] into a node whose origin is at ([dx], [dy]) in the stream's pixels. */
    fun replayAt(canvas: Canvas, b: IntBuffer, from: Int, n: Int, dx: Float, dy: Float) {
        val sx = shiftX; val sy = shiftY
        shiftX = sx + dx; shiftY = sy + dy
        try { replay(canvas, b, from, n) } finally { shiftX = sx; shiftY = sy }
    }

    private fun f(b: IntBuffer, i: Int) = java.lang.Float.intBitsToFloat(b.get(i))

    /** The last DASH: its intervals (path units) and phase; null when solid. */
    private var dash: FloatArray? = null
    /** The last SHADER, for the next shape op; null when it paints its colour. */
    private var shader: android.graphics.Shader? = null
    /** BLUR (op 44): the sigma the next GLYPHS draws through, and the last mask made for one. */
    private var glyphBlur = 0f
    private var blurMask: Pair<Float, android.graphics.BlurMaskFilter>? = null
    private fun blurFilter(sigma: Float): android.graphics.MaskFilter? {
        if (sigma <= 0f) return null
        blurMask?.let { (s, m) -> if (s == sigma) return m }
        // BlurMaskFilter takes a radius; Skia's sigma is 0.57735 of it, plus half a pixel.
        val m = android.graphics.BlurMaskFilter(Math.max(0.01f, (sigma - 0.5f) / 0.57735f), android.graphics.BlurMaskFilter.Blur.NORMAL)
        blurMask = sigma to m
        return m
    }
    private var dashPeriod = 0f
    private var dashPhase = 0f
    private val measure = android.graphics.PathMeasure()
    private val segment = Path()
    private val spans = FloatArray(8)
    /** What [readPath] last read: its contours and whether one closes. */
    private var pathMoves = 0
    private var pathCloses = false
    /** A layer's path, read once (set by the layer's recording, see [Layers]); null elsewhere. */
    var memo: PathMemo? = null

    /**
     * Op 13 under a DASH: the path's on-spans, drawn as the path's own segments without a path effect (as a View
     * drawing a draw-in with PathMeasure.getSegment does), so RenderThread strokes a plain path and nothing is dashed
     * again every frame; the whole path when one span covers it. A path of several or closed contours, a zero-length
     * dash, or more than four spans keeps the DashPathEffect (Skia's dashing).
     */
    private fun strokeDashed(c: Canvas, b: IntBuffer, pathAt: Int) {
        val d = dash!!
        val single = pathMoves == 1 && !pathCloses
        var length = -1f
        var m = measure
        if (single) {
            val memo = memo
            if (memo != null && memo.length >= 0f) { m = memo.measure; length = memo.length }
            else {
                if (memo != null) m = memo.measure
                m.setPath(path, false)
                length = m.length
                if (memo != null) memo.length = length
            }
        }
        var count = 0
        if (length > 0f && dashPeriod > 0f && d.none { it <= 0f }) {
            var s = -(((dashPhase % dashPeriod) + dashPeriod) % dashPeriod)
            var k = 0
            var steps = 0
            while (s < length && count >= 0) {
                val e = s + d[k]
                if (k % 2 == 0 && e > 0f) {
                    if (count == 4) count = -1 else { spans[2 * count] = Math.max(s, 0f); spans[2 * count + 1] = Math.min(e, length); count++ }
                }
                s = e; k = (k + 1) % d.size
                if (++steps > 64) count = -1
            }
        } else count = -1
        if (count < 0) {
            stroke.pathEffect = android.graphics.DashPathEffect(d, dashPhase)
            c.drawPath(path, stroke)
            stroke.pathEffect = null
        } else if (count == 1 && spans[0] <= length * 0.001f && spans[1] >= length * 0.999f) {
            if (!strokePolyline(c, b, pathAt)) c.drawPath(path, stroke)
        } else if (count > 0 && !strokePolyline(c, b, pathAt, count)) {
            segment.rewind()
            for (k in 0 until count) m.getSegment(spans[2 * k], spans[2 * k + 1], segment, true)
            c.drawPath(segment, stroke)
        }
    }

    /** The last path [readPath] read, when it was one circle (a move, four cubics, maybe a close): its centre and
     * radius, the angle its contour starts at (degrees, as Canvas measures them) and its direction (1: clockwise on
     * the canvas, as Canvas's sweeps are). Drawn as an oval or arcs, Skia's analytic circle and arc ops, where the path
     * would be tessellated (or a dashed stroke rasterized in software) again at every frame HWUI draws. */
    private var circle = false
    private var circleClosed = false
    private var circleX = 0f
    private var circleY = 0f
    private var circleR = 0f
    private var circleStart = 0f
    private var circleDir = 1f
    private val oval = RectF()

    private fun findCircle(b: IntBuffer, at: Int) {
        circle = false
        val n = b.get(at)
        if (n != 5 && n != 6) return
        var i = at + 1
        if (b.get(i) != 0) return
        val x0 = f(b, i + 1); val y0 = f(b, i + 2)
        i += 3
        // The on-curve points, then each cubic's midpoint.
        val px = FloatArray(5); val py = FloatArray(5); val mx = FloatArray(4); val my = FloatArray(4)
        px[0] = x0; py[0] = y0
        for (k in 0 until 4) {
            if (b.get(i) != 2) return
            val x1 = f(b, i + 1); val y1 = f(b, i + 2); val x2 = f(b, i + 3); val y2 = f(b, i + 4)
            val x3 = f(b, i + 5); val y3 = f(b, i + 6)
            mx[k] = (px[k] + 3 * x1 + 3 * x2 + x3) / 8f; my[k] = (py[k] + 3 * y1 + 3 * y2 + y3) / 8f
            px[k + 1] = x3; py[k + 1] = y3
            i += 7
        }
        val closed = n == 6
        if (closed && b.get(i) != 3) return
        val cx = (px[0] + px[1] + px[2] + px[3]) / 4f; val cy = (py[0] + py[1] + py[2] + py[3]) / 4f
        val r = Math.hypot((px[0] - cx).toDouble(), (py[0] - cy).toDouble()).toFloat()
        if (!(r > 0f)) return
        val tol = r * 0.002f + 0.01f
        fun on(x: Float, y: Float) = Math.abs(Math.hypot((x - cx).toDouble(), (y - cy).toDouble()).toFloat() - r) <= tol
        for (k in 0 until 5) if (!on(px[k], py[k])) return
        for (k in 0 until 4) if (!on(mx[k], my[k])) return
        if (Math.abs(px[4] - px[0]) > tol || Math.abs(py[4] - py[0]) > tol) return
        // A quarter turn each, the same way round.
        var turn = 0f
        for (k in 0 until 4) {
            val cross = (px[k] - cx) * (py[k + 1] - cy) - (py[k] - cy) * (px[k + 1] - cx)
            val dot = (px[k] - cx) * (px[k + 1] - cx) + (py[k] - cy) * (py[k + 1] - cy)
            if (Math.abs(dot) > r * r * 0.01f || Math.abs(Math.abs(cross) - r * r) > r * r * 0.01f) return
            val dir = Math.signum(cross)
            if (turn != 0f && dir != turn) return
            turn = dir
        }
        circle = true
        circleClosed = closed
        circleX = cx; circleY = cy; circleR = r
        circleStart = Math.toDegrees(Math.atan2((py[0] - cy).toDouble(), (px[0] - cx).toDouble())).toFloat()
        circleDir = turn
    }

    /** The last DASH along the circle [findCircle] found, as arcs (Skia's dash restarts at the contour's start and
     * does not wrap; neither do these). Whether it could: false leaves the dashed path to be drawn. */
    private fun dashedCircle(c: Canvas, p: Paint): Boolean {
        val d = dash ?: return false
        if (dashPeriod <= 0f) return false
        val length = (2.0 * Math.PI * circleR).toFloat()
        oval.set(circleX - circleR, circleY - circleR, circleX + circleR, circleY + circleR)
        val effect = p.pathEffect
        p.pathEffect = null
        var drawn = 0
        val first = Math.floor((dashPhase / dashPeriod).toDouble()).toLong()
        val last = Math.floor(((dashPhase + length) / dashPeriod).toDouble()).toLong()
        if (last - first > 64) { p.pathEffect = effect; return false }
        for (m in first..last) {
            var a = m * dashPeriod
            var k = 0
            while (k + 1 < d.size) {
                val s = maxOf(a, dashPhase) - dashPhase
                val e = minOf(a + d[k], dashPhase + length) - dashPhase
                if (e - s > length * 1e-5f) {
                    c.drawArc(oval, circleStart + circleDir * s / length * 360f, circleDir * (e - s) / length * 360f, false, p)
                    drawn++
                }
                a += d[k] + d[k + 1]
                k += 2
            }
        }
        p.pathEffect = effect
        return true
    }

    private var segments = FloatArray(256)
    private var cut = FloatArray(256)
    private val disc = Paint(Paint.ANTI_ALIAS_FLAG)

    /**
     * The path at [at] stroked as what it is when it is one polyline with round joins in an opaque colour (a chart's
     * line): its segments as lines and a disc at each joint (and at each end under round caps), which HWUI draws as
     * rects and circles on the GPU; with [count] > 0, only its [spans] (lengths along it: a draw-in's part). As a
     * path, a GPU without multisampling (PowerVR) gets its coverage from a mask rasterized on the CPU, kept only
     * while more than half of the path is inside the frame's damage and never for a part that changes each frame: a
     * line whose end something animates over (a pulsing dot) was rasterized again at every frame, each time into a
     * new texture (crypto at rest on a Pixel 10 Pro XL: 220 ms/s on HWUI's workers and ~19 MB of GPU memory; after a
     * fling, whose rows each draw in, 250 -> 129 MB and 11% less CPU). Only there ([Polylines]). False when the path
     * is anything else, or is left a path: the caller strokes it.
     */
    private fun strokePolyline(c: Canvas, b: IntBuffer, at: Int, count: Int = 0): Boolean {
        if (!Polylines.on) return false
        val n = b.get(at)
        if (n < 2 || n > 1024 || stroke.strokeJoin != Paint.Join.ROUND || stroke.strokeCap == Paint.Cap.SQUARE) return false
        if (stroke.alpha != 255 || stroke.strokeWidth <= 0f) return false
        if (segments.size < 4 * (n - 1)) { segments = FloatArray(4 * (n - 1)); cut = FloatArray(4 * (n - 1)) }
        val s = segments
        var i = at + 1; var k = 0; var x = 0f; var y = 0f
        for (v in 0 until n) {
            if (b.get(i) != (if (v == 0) 0 else 1)) return false
            val nx = f(b, i + 1); val ny = f(b, i + 2)
            if (v > 0) { s[k] = x; s[k + 1] = y; s[k + 2] = nx; s[k + 3] = ny; k += 4 }
            x = nx; y = ny; i += 3
        }
        if (count == 0) { strokeRun(c, s, k); return true }
        val o = cut
        for (q in 0 until count) {
            val from = spans[2 * q]; val to = spans[2 * q + 1]
            var along = 0f; var m = 0; var j = 0
            while (j < k && along < to) {
                val dx = s[j + 2] - s[j]; val dy = s[j + 3] - s[j + 1]
                val len = Math.hypot(dx.toDouble(), dy.toDouble()).toFloat()
                val a = Math.max(from, along); val e = Math.min(to, along + len)
                if (e > a && len > 0f) {
                    val t0 = (a - along) / len; val t1 = (e - along) / len
                    o[m] = s[j] + dx * t0; o[m + 1] = s[j + 1] + dy * t0; o[m + 2] = s[j] + dx * t1; o[m + 3] = s[j + 1] + dy * t1
                    m += 4
                }
                along += len; j += 4
            }
            if (m > 0) strokeRun(c, o, m)
        }
        return true
    }

    /** [k] floats of [s] (segments end to end) under [stroke]: butt lines, a disc at each joint and round end. */
    private fun strokeRun(c: Canvas, s: FloatArray, k: Int) {
        val cap = stroke.strokeCap
        // The segments as one feathered triangle list on PowerVR's renderer ([meshLines]; mesh=0: Skia's
        // antialiased lines, to compare): 48 of those a sparkline cost 11 mW of GPU on a Pixel 10 Pro XL at every
        // fling speed (crypto, 57 -> 46 mW at 1k dp/s), where the triangles cost what a path's cached mask does.
        if (Polylines.mesh) meshLines(c, s, k)
        else { stroke.strokeCap = Paint.Cap.BUTT; c.drawLines(s, 0, k, stroke); stroke.strokeCap = cap }
        // The joints (and round ends) as round points in one call: each is a disc on the GPU, as a circle is, and a
        // sparkline's 49 were 49 calls a recording, every frame a draw-in plays (dots=0: a circle each, to compare).
        val r = stroke.strokeWidth / 2f
        if (!dotsAtOnce) {
            disc.color = stroke.color
            if (cap == Paint.Cap.ROUND) c.drawCircle(s[0], s[1], r, disc)
            var j = 2
            while (j < k) { if (j + 2 < k || cap == Paint.Cap.ROUND) c.drawCircle(s[j], s[j + 1], r, disc); j += 4 }
            return
        }
        if (dots.size < k / 2 + 2) dots = FloatArray(k / 2 + 2)
        val d = dots
        var m = 0
        if (cap == Paint.Cap.ROUND) { d[0] = s[0]; d[1] = s[1]; m = 2 }
        var j = 2
        while (j < k) { if (j + 2 < k || cap == Paint.Cap.ROUND) { d[m] = s[j]; d[m + 1] = s[j + 1]; m += 2 }; j += 4 }
        if (m == 0) return
        dot.color = stroke.color; dot.strokeWidth = stroke.strokeWidth
        c.drawPoints(d, 0, m, dot)
    }
    private val iconBounds = RectF()
    private val iconValues = FloatArray(9)
    private val iconMatrix = Matrix()

    /** [path] (the stream's words [from, to)) under [paint], as a picture of it made once and kept ([Icons]), where
     * HWUI would rasterize the path on the CPU ([Polylines]): a small path under a plain scale, as a row's symbols
     * are, the same in every row. False when it is not that: the caller draws the path. */
    private fun icon(c: Canvas, b: IntBuffer, from: Int, to: Int, paint: Paint): Boolean {
        if (!Icons.on || memo != null) return false
        current.getValues(iconValues)
        val v = iconValues
        val sx = v[0]; val sy = v[4]
        if (v[1] != 0f || v[3] != 0f || sx <= 0f || sy <= 0f) return false
        path.computeBounds(iconBounds, true)
        val grow = if (paint.style == Paint.Style.STROKE) paint.strokeWidth * (if (paint.strokeJoin == Paint.Join.MITER) 2f else 0.5f) else 0f
        val l = Math.floor(((iconBounds.left - grow) * sx + v[2]).toDouble()).toFloat() - 1f
        val t = Math.floor(((iconBounds.top - grow) * sy + v[5]).toDouble()).toFloat() - 1f
        val w = Math.ceil(((iconBounds.right + grow) * sx + v[2]).toDouble()).toInt() + 1 - l.toInt()
        val h = Math.ceil(((iconBounds.bottom + grow) * sy + v[5]).toDouble()).toInt() + 1 - t.toInt()
        if (w <= 0 || h <= 0 || w > Icons.MAX || h > Icons.MAX) return false
        var h1 = 1125899906842597L; var h2 = 0x9E3779B97F4A7C15uL.toLong()
        fun mix(x: Int) { h1 = 31 * h1 + x; h2 = (h2 xor x.toLong()) * 0x100000001b3L }
        for (k in from until to) mix(b.get(k))
        mix(paint.color); mix(sx.toRawBits()); mix(sy.toRawBits()); mix(path.fillType.ordinal)
        if (paint.style == Paint.Style.STROKE) {
            mix(paint.strokeWidth.toRawBits()); mix(paint.strokeCap.ordinal); mix(paint.strokeJoin.ordinal); mix(7)
        }
        // The fraction of a pixel it sits at is part of the picture.
        mix(((v[2] - Math.floor(v[2].toDouble()).toFloat()) * 8f).toInt()); mix(((v[5] - Math.floor(v[5].toDouble()).toFloat()) * 8f).toInt())
        val key = Icons.Key(h1, h2, to - from)
        var e = Icons.map[key]
        if (e == null) {
            if (Icons.map.size >= Icons.CAP) return false
            val bmp = Bitmap.createBitmap(w, h, Bitmap.Config.ARGB_8888)
            val cv = Canvas(bmp)
            cv.translate(-l, -t)
            cv.concat(current)
            cv.drawPath(path, paint)
            e = Icons.Entry(bmp, l - v[2], t - v[5])
            Icons.map[key] = e
        }
        c.save()
        iconMatrix.setTranslate(Math.round(v[2] + e.dx).toFloat(), Math.round(v[5] + e.dy).toFloat())
        c.setMatrix(iconMatrix)
        c.drawBitmap(e.bmp, 0f, 0f, null)
        c.restore()
        return true
    }
    private var mesh = FloatArray(0)
    private var meshColors = IntArray(0)
    private val meshPaint = Paint().apply { color = -1 }
    private val meshValues = FloatArray(9)

    /** [k] floats of [s] (segments end to end) as one triangle list: per segment a solid core and, either side, a
     * pixel-wide strip fading to nothing (the edge's antialiasing, as vertex alpha). */
    private fun meshLines(c: Canvas, s: FloatArray, k: Int) {
        current.getValues(meshValues)
        val scale = Math.max(1e-3f, Math.hypot(meshValues[0].toDouble(), meshValues[3].toDouble()).toFloat())
        val half = 0.5f / scale
        val hw = stroke.strokeWidth / 2f
        val inner = Math.max(0f, hw - half); val outer = hw + half
        val n = k / 4 * 18
        if (mesh.size < n * 2) { mesh = FloatArray(n * 2); meshColors = IntArray(n) }
        val solid = stroke.color; val clear = solid and 0x00ffffff
        var q = 0; var m = 0; var v = 0
        fun put(x: Float, y: Float, col: Int) { mesh[m] = x; mesh[m + 1] = y; m += 2; meshColors[v++] = col }
        while (q < k) {
            val x0 = s[q]; val y0 = s[q + 1]; val x1 = s[q + 2]; val y1 = s[q + 3]
            val dx = x1 - x0; val dy = y1 - y0
            val len = Math.hypot(dx.toDouble(), dy.toDouble()).toFloat()
            if (len > 0f) {
                val ux = -dy / len; val uy = dx / len
                val ix = ux * inner; val iy = uy * inner; val ox = ux * outer; val oy = uy * outer
                // Core.
                put(x0 + ix, y0 + iy, solid); put(x0 - ix, y0 - iy, solid); put(x1 + ix, y1 + iy, solid)
                put(x0 - ix, y0 - iy, solid); put(x1 - ix, y1 - iy, solid); put(x1 + ix, y1 + iy, solid)
                // The two edges.
                put(x0 + ox, y0 + oy, clear); put(x0 + ix, y0 + iy, solid); put(x1 + ox, y1 + oy, clear)
                put(x0 + ix, y0 + iy, solid); put(x1 + ix, y1 + iy, solid); put(x1 + ox, y1 + oy, clear)
                put(x0 - ix, y0 - iy, solid); put(x0 - ox, y0 - oy, clear); put(x1 - ix, y1 - iy, solid)
                put(x0 - ox, y0 - oy, clear); put(x1 - ox, y1 - oy, clear); put(x1 - ix, y1 - iy, solid)
            }
            q += 4
        }
        if (m > 0) c.drawVertices(Canvas.VertexMode.TRIANGLES, m, mesh, 0, null, 0, meshColors, 0, null, 0, 0, meshPaint)
    }
    private var dots = FloatArray(0)
    private val dot = Paint(Paint.ANTI_ALIAS_FLAG).apply { style = Paint.Style.STROKE; strokeCap = Paint.Cap.ROUND }
    var dotsAtOnce = true

    private fun readPath(b: IntBuffer, at: Int, evenOdd: Boolean): Int {
        val memo = memo
        if (memo != null && memo.at == at) {
            path.set(memo.path)
            pathMoves = memo.moves; pathCloses = memo.closes
            circle = false
            return memo.end
        }
        findCircle(b, at)
        path.rewind()
        path.fillType = if (evenOdd) Path.FillType.EVEN_ODD else Path.FillType.WINDING
        var i = at + 1
        var moves = 0
        var closes = false
        repeat(b.get(at)) {
            when (b.get(i)) {
                0 -> { path.moveTo(f(b, i + 1), f(b, i + 2)); i += 3; moves++ }
                1 -> { path.lineTo(f(b, i + 1), f(b, i + 2)); i += 3 }
                2 -> { path.cubicTo(f(b, i + 1), f(b, i + 2), f(b, i + 3), f(b, i + 4), f(b, i + 5), f(b, i + 6)); i += 7 }
                else -> { path.close(); i += 1; closes = true }
            }
        }
        pathMoves = moves; pathCloses = closes
        if (memo != null) {
            memo.path.set(path); memo.at = at; memo.end = i; memo.moves = moves; memo.closes = closes; memo.length = -1f
        }
        return i
    }

    private val shadowPaint = Paint(Paint.ANTI_ALIAS_FLAG or Paint.FILTER_BITMAP_FLAG)
    private val blurs = HashMap<Float, android.graphics.BlurMaskFilter>()

    /** Op 40: an outer box-shadow, the rounded rect blurred by sigma (Skia's analytic blur under the matrix),
     * drawn only outside the border box. */
    private fun shadow(c: Canvas, b: IntBuffer, i: Int) {
        shadowPaint.color = b.get(i + 1)
        val sigma = f(b, i + 2)
        // Skia's radius → sigma is 0.57735·r + 0.5.
        shadowPaint.maskFilter = if (sigma > 0.5f) blurs.getOrPut(sigma) {
            android.graphics.BlurMaskFilter((sigma - 0.5f) / 0.57735f, android.graphics.BlurMaskFilter.Blur.NORMAL)
        } else null
        c.save()
        if (readRRect(b, i + 15)) c.clipOutRect(rect) else { path.rewind(); path.addRoundRect(rect, radii, Path.Direction.CW); c.clipOutPath(path) }
        val square = readRRect(b, i + 3)
        // Under a scale and translation: the mask made once on the CPU ([Shadows]), not a GPU blur.
        if (values[1] == 0f && values[3] == 0f && values[0] > 0f && Math.abs(values[0] - values[4]) < 1e-3f && sigma > 0.5f / values[0]) {
            val mf = shadowPaint.maskFilter
            shadowPaint.maskFilter = null
            val drawn = Shadows.draw(c, shadowPaint, rect, radii, sigma, values[0])
            shadowPaint.maskFilter = mf
            if (drawn) { c.restore(); return }
        }
        if (square) c.drawRect(rect, shadowPaint)
        else if (uniform()) c.drawRoundRect(rect, radii[0], radii[1], shadowPaint)
        else { path.rewind(); path.addRoundRect(rect, radii, Path.Direction.CW); c.drawPath(path, shadowPaint) }
        c.restore()
    }

    /** Whether the radii just read are one (rx, ry) at every corner. */
    private fun uniform(): Boolean {
        for (k in 2 until 8 step 2) if (radii[k] != radii[0] || radii[k + 1] != radii[1]) return false
        return true
    }

    /** x y w h then eight radii at `at`; whether every radius is zero. */
    private fun readRRect(b: IntBuffer, at: Int): Boolean {
        rect.set(f(b, at), f(b, at + 1), f(b, at) + f(b, at + 2), f(b, at + 1) + f(b, at + 3))
        var square = true
        for (k in 0 until 8) { radii[k] = f(b, at + 4 + k); if (radii[k] != 0f) square = false }
        return square
    }

    /** A border ring: square, as up to four rects; rounded, as a double round rect (both analytic in HWUI). */
    private fun ring(canvas: Canvas, b: IntBuffer, i: Int) {
        fill.color = b.get(i + 1)
        val outerSquare = readRRect(b, i + 2)
        var innerSquare = true
        inner.set(f(b, i + 14), f(b, i + 15), f(b, i + 14) + f(b, i + 16), f(b, i + 15) + f(b, i + 17))
        for (k in 0 until 8) { innerRadii[k] = f(b, i + 18 + k); if (innerRadii[k] != 0f) innerSquare = false }
        if (inner.width() <= 0f || inner.height() <= 0f) {
            if (outerSquare) canvas.drawRect(rect, fill) else {
                path.rewind(); path.addRoundRect(rect, radii, Path.Direction.CW); canvas.drawPath(path, fill)
            }
        } else if (outerSquare && innerSquare) {
            strip.set(rect.left, rect.top, rect.right, inner.top); if (strip.height() > 0f) canvas.drawRect(strip, fill)
            strip.set(rect.left, inner.bottom, rect.right, rect.bottom); if (strip.height() > 0f) canvas.drawRect(strip, fill)
            strip.set(rect.left, inner.top, inner.left, inner.bottom); if (strip.width() > 0f) canvas.drawRect(strip, fill)
            strip.set(inner.right, inner.top, rect.right, inner.bottom); if (strip.width() > 0f) canvas.drawRect(strip, fill)
        } else {
            canvas.drawDoubleRoundRect(rect, radii, inner, innerRadii, fill)
        }
    }

    fun replay(canvas: Canvas, b: IntBuffer, from: Int, n: Int) {
        tint = 1f; tintEnd = -1
        val base = canvas.save()
        val matrixBase = matrixDepth
        // Each recording starts locally; stream matrices supply the device coordinates.
        current.reset(); canvas.setMatrix(current)
        var c: Canvas = canvas
        var outer: Canvas? = null
        var node: android.graphics.RenderNode? = null
        var i = from
        while (i < n) {
            when (b.get(i)) {
                1 -> {
                    values[0] = f(b, i + 1); values[3] = f(b, i + 2); values[1] = f(b, i + 3); values[4] = f(b, i + 4)
                    values[2] = f(b, i + 5); values[5] = f(b, i + 6); values[6] = 0f; values[7] = 0f; values[8] = 1f
                    matrix.setValues(values)
                    if (shiftX != 0f || shiftY != 0f) matrix.postTranslate(-shiftX, -shiftY)
                    current.set(matrix)
                    c.setMatrix(matrix)
                    i += 7
                }
                27 -> { backdrop(c, b, from, i); i += 14 }
                42 -> { backdrop(c, b, from, i); i += 14 + 2 * b.get(i + 1) }
                40 -> { shadow(c, b, i); i += 27 }
                28 -> {
                    native(b.get(i + 1))?.let { node ->
                        val square = readRRect(b, i + 3)
                        c.save()
                        if (square) c.clipRect(rect) else { path.rewind(); path.addRoundRect(rect, radii, Path.Direction.CW); c.clipPath(path) }
                        current.mapRect(backdropBounds, rect)
                        val m = Matrix(); m.setTranslate(Math.round(backdropBounds.left).toFloat(), Math.round(backdropBounds.top).toFloat())
                        c.setMatrix(m)
                        c.drawRenderNode(node)
                        c.restore()
                    }
                    i += 16 + (b.get(i + 15) + 3) / 4
                }
                2 -> {
                    fill.color = b.get(i + 1)
                    if (tint < 1f) fill.alpha = Math.round(fill.alpha * tint)
                    fill.shader = shader
                    if (readRRect(b, i + 2)) c.drawRect(rect, fill)
                    else if (uniform()) c.drawRoundRect(rect, radii[0], radii[1], fill)
                    else { path.rewind(); path.addRoundRect(rect, radii, Path.Direction.CW); c.drawPath(path, fill) }
                    fill.shader = null; shader = null
                    i += 14
                }
                3 -> {
                    fill.color = b.get(i + 1)
                    fill.shader = shader
                    val pathAt3 = i + 2
                    i = readPath(b, i + 3, b.get(i + 2) == 1)
                    if (circle) {
                        oval.set(circleX - circleR, circleY - circleR, circleX + circleR, circleY + circleR)
                        c.drawOval(oval, fill)
                    } else if (shader != null || !icon(c, b, pathAt3, i, fill)) c.drawPath(path, fill)
                    fill.shader = null; shader = null
                }
                4 -> {
                    saveMatrix()
                    c.save()
                    if (readRRect(b, i + 1)) c.clipRect(rect) else {
                        path.rewind(); path.addRoundRect(rect, radii, Path.Direction.CW); c.clipPath(path)
                    }
                    i += 13
                }
                5 -> {
                    saveMatrix()
                    c.save()
                    i = readPath(b, i + 2, b.get(i + 1) == 1)
                    c.clipPath(path)
                }
                6 -> {
                    if (i == tintEnd) { tint = 1f; tintEnd = -1 }
                    c.restore(); restoreMatrix(); i += 1
                }
                7 -> {
                    // saveLayer bounds use the current Canvas space, including earlier restores.
                    saveMatrix()
                    val alpha = f(b, i + 1)
                    val end = layerEnd(b, i + 2, n)
                    if (end > 0 && singleFill(b, i + 2, end)) {
                        // One fill cannot overlap itself: its colour carries the opacity, with no offscreen surface.
                        c.save(); tint = alpha; tintEnd = end
                    } else {
                        c.saveLayerAlpha(layerBounds(b, i + 2, end), (alpha * 255).toInt())
                    }
                    i += 2
                }
                8 -> {
                    rect.set(f(b, i + 2), f(b, i + 3), f(b, i + 2) + f(b, i + 4), f(b, i + 3) + f(b, i + 5))
                    val anim = animated(b.get(i + 1))
                    if (anim != null) {
                        rect.round(bounds); drawAnimated(c, anim)
                    } else image(b.get(i + 1))?.let { bmp -> c.drawBitmap(bmp, null, rect, imagePaint) }
                    i += 6
                }
                26 -> i += 3 + (b.get(i + 2) + 3) / 4
                9 -> {
                    val font = fonts[b.get(i + 1)]
                    val count = b.get(i + 5)
                    if (font != null && count > 0) {
                        glyphPaint.textSize = f(b, i + 2)
                        glyphPaint.color = b.get(i + 3)
                        glyphPaint.textSkewX = f(b, i + 4)
                        glyphPaint.maskFilter = blurFilter(glyphBlur); glyphBlur = 0f
                        if (glyphIds.size < count) { glyphIds = IntArray(count * 2); positions = FloatArray(count * 4) }
                        var at = i + 6
                        for (g in 0 until count) {
                            glyphIds[g] = b.get(at); positions[2 * g] = f(b, at + 1); positions[2 * g + 1] = f(b, at + 2); at += 3
                        }
                        if (android.os.Build.VERSION.SDK_INT >= 31) c.drawGlyphs(glyphIds, 0, positions, 0, count, font, glyphPaint)
                        else LegacyGlyphs.draw(c, glyphIds, positions, count, font, glyphPaint)
                    }
                    i += 6 + 3 * count
                }
                10 -> i += 5 + (b.get(i + 4) + 3) / 4
                41 -> i = fontAxesEnd(b, i)
                11 -> i += 4
                12 -> i += 2
                14 -> {
                    // A picture drawn within a (rounded) rect.
                    val anim = animated(b.get(i + 1))
                    if (anim != null) {
                        rect.set(f(b, i + 6), f(b, i + 7), f(b, i + 6) + f(b, i + 8), f(b, i + 7) + f(b, i + 9))
                        for (k in 0 until 8) radii[k] = f(b, i + 10 + k)
                        c.save()
                        path.rewind(); path.addRoundRect(rect, radii, Path.Direction.CW); c.clipPath(path)
                        dstRect.set(f(b, i + 2), f(b, i + 3), f(b, i + 2) + f(b, i + 4), f(b, i + 3) + f(b, i + 5))
                        dstRect.round(bounds); drawAnimated(c, anim)
                        c.restore()
                    } else image(b.get(i + 1))?.let { bmp ->
                        // Drawn under a clip, not as a BitmapShader's fill: a shader keeps its bitmap's pixels (for a
                        // hardware bitmap, the GPU buffer) until the garbage collector frees the Java Shader, long
                        // after the picture is recycled (a photo fling held twice its live pictures).
                        dstRect.set(f(b, i + 2), f(b, i + 3), f(b, i + 2) + f(b, i + 4), f(b, i + 3) + f(b, i + 5))
                        rect.set(f(b, i + 6), f(b, i + 7), f(b, i + 6) + f(b, i + 8), f(b, i + 7) + f(b, i + 9))
                        var square = true
                        for (k in 0 until 8) { radii[k] = f(b, i + 10 + k); if (radii[k] != 0f) square = false }
                        c.save()
                        when {
                            square -> c.clipRect(rect)
                            else -> { path.rewind(); path.addRoundRect(rect, radii, Path.Direction.CW); c.clipPath(path) }
                        }
                        c.drawBitmap(bmp, null, dstRect, imagePaint)
                        c.restore()
                    }
                    i += 18
                }
                13 -> {
                    stroke.color = b.get(i + 1)
                    stroke.shader = shader
                    stroke.strokeWidth = f(b, i + 2)
                    stroke.strokeCap = when (b.get(i + 3)) { 1 -> Paint.Cap.ROUND; 2 -> Paint.Cap.SQUARE; else -> Paint.Cap.BUTT }
                    stroke.strokeJoin = when (b.get(i + 4)) { 1 -> Paint.Join.ROUND; 2 -> Paint.Join.BEVEL; else -> Paint.Join.MITER }
                    val pathAt = i + 5
                    i = readPath(b, pathAt, false)
                    // A circle as an oval or (dashed) arcs, Skia's analytic ops; another dashed stroke as its
                    // path's on-segments ([strokeDashed]); neither is rasterized again at every frame HWUI draws.
                    if (dash != null && circle && dashedCircle(c, stroke)) {
                    } else if (dash != null) strokeDashed(c, b, pathAt)
                    else if (circle && (circleClosed || stroke.strokeCap != Paint.Cap.SQUARE)) {
                        oval.set(circleX - circleR, circleY - circleR, circleX + circleR, circleY + circleR)
                        c.drawOval(oval, stroke)
                    } else if (!strokePolyline(c, b, pathAt) && (shader != null || !icon(c, b, pathAt, i, stroke))) c.drawPath(path, stroke)
                    dash = null; stroke.shader = null; shader = null
                }
                43 -> { shader = Shaders.read(b, i); i += 2 + b.get(i + 1) }
                44 -> { glyphBlur = f(b, i + 1); i += 2 }
                25 -> {
                    val n = b.get(i + 2)
                    val d = FloatArray(n) { f(b, i + 3 + it) }
                    dash = if (n >= 2 && n % 2 == 0 && d.sum() > 0f) d else null
                    dashPeriod = d.sum(); dashPhase = f(b, i + 1)
                    i += 3 + n
                }
                15 -> i += 6 + b.get(i + 5)
                16 -> i += 1
                17 -> {
                    row(b.get(i + 1))?.let { r ->
                        c.save()
                        matrix.setTranslate(f(b, i + 2) - shiftX, f(b, i + 3) - shiftY)
                        c.setMatrix(matrix)
                        c.drawRenderNode(r)
                        c.restore()
                    }
                    i += 4
                }
                18 -> i += 2
                19 -> { ring(c, b, i); i += 26 }
                23 -> {
                    // A picture's slot: its own node, so a picture arriving re-records only that.
                    slot(b.get(i + 1))?.let { s -> c.save(); matrix.setTranslate(-shiftX, -shiftY); c.setMatrix(matrix); c.drawRenderNode(s); c.restore() }
                    i += 2
                }
                24 -> i += 3 + b.get(i + 2)
                29 -> {
                    // A layer the reader moves and fades: drawn in the stream's coordinates, as a slot is.
                    layer(b.get(i + 1))?.let { s -> c.save(); matrix.setTranslate(-shiftX, -shiftY); c.setMatrix(matrix); c.drawRenderNode(s); c.restore() }
                    i += 2
                }
                30, 31 -> i += 3 + b.get(i + 2)
                32 -> i += 2
                33 -> i += 3
                20 -> {
                    // A scroller's rows: their own node, so a move only translates it.
                    val g = group!!(b.get(i + 1))
                    g.setPosition(0, 0, width, height)
                    node = g
                    grouped = b.get(i + 1)
                    outer = c
                    saveMatrix()
                    c = g.beginRecording()
                    current.reset(); c.setMatrix(current)
                    i += 2
                }
                21 -> {
                    node?.let { g ->
                        g.endRecording()
                        c = outer!!
                        restoreMatrix()
                        c.save(); matrix.setTranslate(-shiftX, -shiftY); c.setMatrix(matrix); c.drawRenderNode(holder?.invoke(grouped) ?: g); c.restore()
                    }
                    node = null
                    i += 1
                }
                else -> break
            }
        }
        canvas.restoreToCount(base)
        matrixDepth = matrixBase
    }
}

/** A layer's one path as [Player] read it (at word [at] of the layer's ops), so a dash phase recorded again each frame
 * neither reads nor measures it again. */
internal class PathMemo {
    val path = Path()
    val measure = android.graphics.PathMeasure()
    var at = -1
    var end = 0
    var moves = 0
    var closes = false
    var length = -1f
}
