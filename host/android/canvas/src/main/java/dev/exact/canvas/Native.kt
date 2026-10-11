package dev.exact.canvas

import android.graphics.Bitmap
import java.nio.ByteBuffer

/** exact2 on the main thread: layout, shaping and the paint walk, recorded as Canvas ops. */
object Native {
    init { System.loadLibrary("exactcanvas") }
    /** Boot the app's host on the calling thread, in a window of this size in pixels ([window] told its insets);
     * whether it booted. Where a host already runs on this thread (the activity was made again in this process,
     * LLP 1115.001 D15.1) nothing boots: that host is adopted in this window, and paints everything again for the
     * reader that calls, which has none of the last one's rows, faces or pictures. */
    @JvmStatic external fun start(width: Int, height: Int, scale: Float, dir: String, live: Boolean): Boolean
    /** [start] on a thread of its own; [adopt] takes the host. Where a host already runs in this process nothing
     * boots, and [adopt] adopts that one as [start] does. */
    @JvmStatic external fun startAsync(width: Int, height: Int, scale: Float, dir: String, live: Boolean)
    /** Wait for [startAsync]'s host and run it on this thread from now on; whether it booted. */
    @JvmStatic external fun adopt(): Boolean
    /** The device's settings, whole ([Device], LLP 1115.001 D3.1): before [start], from any thread, so the host boots
     * with them, and again on the host's thread whenever one changes. */
    @JvmStatic external fun configure(
        night: Boolean, fontScale: Float, locale: String, rtl: Boolean, hour24: Boolean, noAnimations: Boolean,
        boldText: Boolean, highContrast: Boolean, touch: Boolean,
    )
    /** The window (LLP 1115.001 D3.3): its size in pixels, the display's pixels a point, and the system's raw insets
     * in pixels, sixteen numbers: top, right, bottom, left for the system bars, then the display cutout, the software
     * keyboard and the system's gestures. Before [start], from any thread, so the host boots laid out in it, and on
     * the host's thread again whenever any of it changes. The reader pads nothing: [chrome] says where the page is. */
    @JvmStatic external fun window(width: Int, height: Int, scale: Float, insets: IntArray)
    /** What the reader draws around the page, when that is not what this last returned (else null): where the page is
     * in the window (x, y, width, height, in pixels), the colour behind it and in the edges it keeps out of (ARGB),
     * and 1 when its scheme is dark (the bars' icons are then light). */
    @JvmStatic external fun chrome(): IntArray?
    @JvmStatic external fun frame(): ByteBuffer?
    @JvmStatic external fun poll(): Boolean
    @JvmStatic external fun animating(): Boolean
    @JvmStatic external fun owed(): Boolean
    /** The collection pass a scroll left; `limit` >= 0 slices it (rows past what shows per list), the scrolled list
     * leading toward `velocity` px/s. Bit 0: a paint is wanted; bit 1: rows are left for another slice. */
    @JvmStatic external fun refine(limit: Int, velocity: Float): Int
    @JvmStatic external fun nextDue(): Double
    @JvmStatic external fun fds(): IntArray
    @JvmStatic external fun scroll(dy: Float)
    @JvmStatic external fun touch(action: Int, x: Float, y: Float)
    /** One slice ([budgetUs] of shaping) of laying out what the touch that is down would open; whether more is left. */
    @JvmStatic external fun ahead(budgetUs: Int): Boolean
    @JvmStatic external fun copyImage(id: Int, bitmap: Bitmap): Boolean
    /** Picture [id] into a GPU buffer (one copy; no upload at first draw). */
    @JvmStatic external fun copyImageToBuffer(id: Int, buffer: android.hardware.HardwareBuffer): Boolean
    /** A GPU canvas's window (op 28 kind 3): the ANativeWindow, to pass to [detachSurface], or 0. */
    @JvmStatic external fun attachSurface(view: Int, surface: android.view.Surface, width: Int, height: Int): Long
    @JvmStatic external fun detachSurface(view: Int, window: Long)
    /** A GPU canvas without a window: it renders into buffers this reader draws ([canvasFrame], [canvasBuffer]). */
    @JvmStatic external fun attachBuffers(view: Int, width: Int, height: Int)
    /** A buffered canvas's newest frame (any thread): generation (24 bits), slot (8), serial (32); -1 none yet, -2 no
     * buffers on this device. */
    @JvmStatic external fun canvasFrame(view: Int): Long
    /** That frame's buffer, or null. */
    @JvmStatic external fun canvasBuffer(view: Int): android.hardware.HardwareBuffer?
    /** A key at the focused field: a character (or '\n' for Enter), or Backspace. */
    @JvmStatic external fun key(code: Int, backspace: Boolean)
    /** Whether an editable field holds the focus: 0 none, 1 an input, 3 a textarea. */
    @JvmStatic external fun editing(): Int
    /** System Back (LLP 1115.001 D6): whether the app took it. From its navigation root's selected route it goes to
     * the Back control the root names, else to the root's `navigate` handler, else it is the runner's own `back`;
     * false when there is nowhere to go back to, and the activity finishes. */
    @JvmStatic external fun back(): Boolean
    /** Run the calling thread on the cores outside the slowest cluster; whether it was set. */
    @JvmStatic external fun fastCores(): Boolean
    /** An ADPF hint session over [tids] (this process's threads), [targetNs] their work's target. */
    @JvmStatic external fun hintSession(tids: IntArray, targetNs: Long): Boolean
    /** Input arrived: the [hintSession]'s threads clocked up for it; whether a hint was sent. */
    @JvmStatic external fun inputHint(): Boolean
    /** Run thread [tid] on the calling thread's core until it calls [fastCores]; whether it was set. */
    @JvmStatic external fun colocate(tid: Int): Boolean
    /** Thread [tid] on the fast cores only ([fast]), or anywhere again. */
    @JvmStatic external fun fastThread(tid: Int, fast: Boolean): Boolean
    /** Picture [id]'s own GPU buffer (decoded into it), or null when its pixels are on the heap. */
    @JvmStatic external fun imageBuffer(id: Int): android.hardware.HardwareBuffer?
    /** Decode pictures straight into GPU buffers ([imageBuffer]) from now on. */
    @JvmStatic external fun hardwarePictures(on: Boolean)
    /** Pictures announced and not fetched yet: id, width, height triples. */
    @JvmStatic external fun pendingPictures(): IntArray
    /** The scroller [scroll] moves (its group's id in the stream), 0 before the first scroll found one. */
    @JvmStatic external fun feed(): Int
}
