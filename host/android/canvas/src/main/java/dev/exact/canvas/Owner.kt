package dev.exact.canvas

import android.os.Handler
import android.os.HandlerThread
import android.os.Looper
import android.os.MessageQueue
import android.os.ParcelFileDescriptor
import android.view.Choreographer
import java.nio.ByteOrder
import java.nio.IntBuffer
import java.util.concurrent.ConcurrentLinkedQueue
import java.util.concurrent.FutureTask
import java.util.concurrent.atomic.AtomicBoolean

/**
 * exact2 on a thread of its own (LLP 1072, on Android). Every call into the Rust host runs on the `exact` thread, which
 * owns it from boot on: its looper takes the executor's replies and decoded pictures, its Choreographer the turns an
 * animation wants. Each new op stream is copied and published; the main thread only ingests and replays them. A scroll
 * translates the feed's group on the main thread at once (as a compositor scrolls) and reaches exact2 after, so a long
 * collection pass delays rows by a frame or two but never holds a frame (owner=0: exact2 on the main thread).
 */
internal class Owner(private val hwbuf: Boolean, private val fast: Boolean, priority: Int, intent: (String) -> String?, private val published: () -> Unit) {
    /** boost=0: input does not raise the clocks, to compare. Otherwise the exact and main threads are an ADPF hint
     * session (made at the first input), told of a workload increase as each press, release or key arrives: an idle
     * phone meets a tap at its lowest clocks (a Pixel 10 Pro XL's exact thread at 400-650 MHz: the release's commit and
     * paint took 5-10 ms and its response missed the frame after the tap; with the hint, 1.8-2 GHz and 2-3 ms). */
    private val boost = intent("boost") != "0"
    private var session: Boolean? = null

    /** A press, release or key arrived: the threads that answer it clocked up now. (A host library without these
     * calls, an older exact2, goes without.) */
    fun input() {
        if (!boost || session == false) return
        try {
            val made = session ?: Native.hintSession(intArrayOf(thread.threadId, android.os.Process.myPid()), 4_000_000L).also { session = it }
            if (made) Native.inputHint()
        } catch (e: UnsatisfiedLinkError) { session = false }
    }

    /** mainfast=0: the main thread wakes for a release where the scheduler puts it, to compare. Otherwise it is kept to
     * the fast cores while a press is held: on a phone with little cores (a Pixel 10a) it woke for the release on one,
     * so the exact thread was not colocated, started 1.3-2.9 ms late on an idle core and ran at 357 MHz until the hint
     * reached the governor; taps answered a frame late 14 of 66 times, 7 of 66 with this. (The hint again at each warm
     * poke did not help; a clock floor of the threads' own, uclamp.min, is refused to an app.) */
    private val mainFast = intent("mainfast") != "0"
    private var isHeld = false

    /** A press began ([on]) or ended (released, cancelled, or held past the warm hold). */
    fun held(on: Boolean) {
        if (on == isHeld) return
        isHeld = on
        if (mainFast) try { Native.fastThread(android.os.Process.myPid(), on) } catch (e: UnsatisfiedLinkError) {}
    }

    /** place=0: the exact thread wakes where the scheduler puts it, to compare. Otherwise, while the main thread waits for
     * an answer, the exact thread runs on the main thread's (awake) core: woken on an idle core it started ~1 ms after
     * the post, on the main thread's ~0.4 ms. It widens its mask again ([Native.fastCores]) as it starts. */
    private var colocate = intent("place") != "0"

    private fun colocated(waitMs: Long): Boolean {
        if (!colocate || !fast || waitMs <= 0) return false
        return try { Native.colocate(thread.threadId) } catch (e: UnsatisfiedLinkError) { colocate = false; false }
    }

    /** A published stream: [ops] (a copy, the reader's), whether it only moves the last paint, the last scroll it
     * includes, and the feed's group (0 before the first scroll). */
    class Pub(val ops: IntBuffer, val move: Boolean, val seq: Int, val feed: Int)

    /** At the main thread's own priority, not above it ([priority]): its work is ahead of the frame the main thread
     * draws, never instead of it (THREAD_PRIORITY_DISPLAY let it preempt the main thread on a phone with few big cores:
     * heavy 12k/24k dropped to 113/108 fps). */
    private val thread = HandlerThread("exact", priority).apply { start() }
    private val handler = Handler(thread.looper)
    private val main = Handler(Looper.getMainLooper())
    /** Every paint in order (each may define and free pictures and rows); moves are coalesced by the reader. */
    val queue = ConcurrentLinkedQueue<Pub>()
    private val notifying = AtomicBoolean(false)
    /** Pictures the exact thread made for streams it published (copying exact2's pixels into a GPU buffer), so the
     * main thread's ingest only takes them. */
    val made = java.util.concurrent.ConcurrentHashMap<Int, android.graphics.Bitmap>()
    /** Whether an editable field holds the focus (0, 1 an input, 3 a textarea), as of the last stream. */
    @Volatile var editing = 0
        private set
    /** Called on the main thread when [editing] changed. */
    var editingChanged: () -> Unit = {}

    // The exact thread's own state.
    private var seq = 0
    private var steps = 0
    private var turnPosted = false
    private var choreographer: Choreographer? = null
    private val prefetch = true

    // Scrolls the main thread sent that the exact thread has not taken: summed, so a busy thread catches up at once.
    private val lock = Any()
    private var owedDy = 0f
    private var owedSeq = 0
    private var stepPosted = false
    /** lead=1: the velocity the window leads by (px/s, positive toward the end; otherwise none, a viewport each side).
     * Fast travel ([LEAD_ON] dp/s and on, until it slows below [LEAD_OFF]) leads by all exact2 allows (two viewports
     * more): a lead that followed the measured speed moved the window's far edge with each frame's jitter, mounting and
     * retiring a row every report (crypto at 1k: 3x the collection work). Off by default: on a Pixel 10 Pro XL the
     * longer window made every report dearer (xheavy 24k: 18-19% of the view blank against 12-14% without, CPU +4%;
     * crypto +12%), and rows near the view are what blanks (ExactView's near rows). */
    @Volatile private var velocity = 0f
    private var measured = 0f
    private var lastScrollAt = 0L
    private val lead = intent("lead") == "1"
    private val density = android.content.res.Resources.getSystem().displayMetrics.density

    init {
        // At once, while the activity goes on setting up: the thread's Choreographer, its cores, and exact2's library
        // (loaded by Native's first call), so the boot posted after starts with them done.
        handler.post {
            choreographer = Choreographer.getInstance()
            // Off the little cores (fast=0: wherever the scheduler puts it): the thread sleeps between bursts (a tap's
            // commit, a frame), so the scheduler wakes it where an idle thread goes, and a tap's work there took two
            // to three times as long. Neither a higher priority nor an ADPF session kept it off them.
            if (fast) Native.fastCores()
        }
    }

    /** Boot at the size the view is expected to have; [done] on the main thread with whether it booted. */
    fun boot(width: Int, height: Int, scale: Float, dir: String, live: Boolean, done: (Boolean) -> Unit) {
        handler.post {
            android.os.Trace.beginSection("exact boot")
            try { Native.hardwarePictures(hwbuf) } catch (e: UnsatisfiedLinkError) { }
            val ok = Native.start(width, height, scale, dir, live)
            android.os.Trace.endSection()
            if (ok) {
                val queue = thread.looper.queue
                for (fd in Native.fds()) {
                    if (fd < 0) continue
                    val pfd = ParcelFileDescriptor.fromFd(fd)
                    queue.addOnFileDescriptorEventListener(pfd.fileDescriptor, MessageQueue.OnFileDescriptorEventListener.EVENT_INPUT) { _, _ ->
                        if (Native.poll()) produce() else schedule()
                        MessageQueue.OnFileDescriptorEventListener.EVENT_INPUT
                    }
                }
                produce()
            }
            main.post { done(ok) }
        }
    }

    fun resize(width: Int, height: Int) = call { Native.resize(width, height) }

    /** A scroll of [dy] px, the main thread's [seq]th. */
    fun scroll(dy: Float, seq: Int) {
        val now = System.nanoTime()
        val dt = (now - lastScrollAt) / 1e9f
        lastScrollAt = now
        // Steps a frame apart: their mean over the last few frames (a pause starts it again).
        measured = if (dt > 0.1f || dt <= 0f) 0f else measured * 0.6f + dy / dt * 0.4f
        if (lead) {
            val dp = Math.abs(measured) / density
            val on = if (velocity != 0f) dp >= LEAD_OFF && Math.signum(measured) == Math.signum(velocity) else dp >= LEAD_ON
            velocity = if (on) Math.signum(measured) * LEAD_PX else 0f
        }
        synchronized(lock) {
            owedDy += dy; owedSeq = seq
            if (stepPosted) return
            stepPosted = true
        }
        handler.post(step)
    }

    private val step = Runnable {
        val dy: Float; val s: Int
        synchronized(lock) { dy = owedDy; s = owedSeq; owedDy = 0f; stepPosted = false }
        android.os.Trace.beginSection("exact scroll")
        Native.scroll(dy)
        android.os.Trace.endSection()
        seq = s
        produce()
        // Every third step: the window's lead mounts, off the main thread.
        if (prefetch && ++steps % 3 == 0) { handler.removeCallbacks(refiner); handler.post(refiner) }
        if (led) { handler.removeCallbacks(settle); handler.postDelayed(settle, SETTLE_MS) }
    }

    /** The last refine led (exact2 keeps that velocity for its passes until a refine sets another). */
    private var led = false

    private val refiner: Runnable = Runnable {
        android.os.Trace.beginSection("exact prefetch")
        val v = velocity
        led = v != 0f
        val r = Native.refine(-1, v)
        if (r and 1 != 0) produce()
        // A pass exact2 cut short (while the view moves fast it builds what fits a frame's share): the rest after
        // the steps queued meanwhile, each pass painting what it built.
        if (r and 2 != 0) { handler.removeCallbacks(refiner); handler.post(refiner) }
        android.os.Trace.endSection()
    }

    /** Scrolling stopped while the window led: it goes back to a viewport each side. */
    private val settle = Runnable {
        if (led) { led = false; if (Native.refine(-1, 0f) and 1 != 0) produce() }
    }

    /** A touch: answered within [waitMs] if exact2 is free (as LLP 1072's events are synchronous, so the response is
     * in this frame), else published when it is. Whether it was answered. */
    fun touch(action: Int, x: Float, y: Float, waitMs: Long): Boolean = answer(waitMs) {
        Native.touch(action, x, y)
        if (action == 0 && ahead) { handler.removeCallbacks(aheadSlice); handler.post(aheadSlice) }
    }

    /** ahead=0: off. What the press of the touch that is down would open is laid out ahead of its release (exact2's
     * `ahead`: its text shaped, in a scratch tree; nothing shown or committed), a slice of at most [AHEAD_US] of
     * shaping at a time. Each slice is posted behind whatever this thread has queued, so the touch's own frame and
     * anything that came meanwhile go first; exact2 forgets the rest when the touch ends or anything commits. The
     * first mail the daily benchmark opens was ready 9 ms after its release, 3 ms of that its layout; 6 ms with this. */
    private val ahead = intent("ahead") != "0"
    private val aheadSlice: Runnable = object : Runnable {
        override fun run() {
            val more = try { Native.ahead(AHEAD_US) } catch (e: UnsatisfiedLinkError) { false }
            if (more) handler.post(this)
        }
    }

    /** Run [f] on the exact thread and publish; wait up to [waitMs] for it. Whether it finished in time. */
    fun answer(waitMs: Long, f: () -> Unit): Boolean {
        val pinned = colocated(waitMs)
        // The waiter stays on this core for the wait (waitpin=0: wherever the scheduler wakes it, to compare): the
        // exact thread is answering here, and the main thread woken for the answer on another, idle core began its
        // ingest ~1.1 ms after the answer was ready (every tap; an answer ready at 4.6 ms was drawn from 7.2 ms).
        val main = android.os.Process.myPid()
        val stays = pinned && waitPin && try { Native.colocate(main) } catch (e: UnsatisfiedLinkError) { false }
        val task = FutureTask { if (pinned) Native.fastCores(); f(); produce() }
        handler.post(task)
        return try { task.get(waitMs, java.util.concurrent.TimeUnit.MILLISECONDS); true } catch (e: java.util.concurrent.TimeoutException) { false } finally {
            if (stays) try { Native.fastThread(main, isHeld && mainFast) } catch (e: UnsatisfiedLinkError) {}
        }
    }
    private val waitPin = intent("waitpin") != "0"

    /** Run [f] on the exact thread, then publish what it changed. */
    fun call(f: () -> Unit) {
        handler.post { f(); produce() }
    }

    /** Run [f] on the exact thread and wait for it (Back, a surface's window): rare, and the answer is needed now. */
    fun <T> sync(f: () -> T): T {
        if (Looper.myLooper() == thread.looper) return f()
        val task = FutureTask { val r = f(); produce(); r }
        handler.post(task)
        return task.get()
    }

    /** exact2's next stream, if it has one: copied, queued, and the main thread told. */
    private fun produce() {
        android.os.Trace.beginSection("exact native frame")
        val fresh = Native.frame()
        android.os.Trace.endSection()
        if (fresh != null) {
            // A heap copy: no native allocation or Cleaner per stream, at up to a stream a frame.
            val words = IntArray(fresh.capacity() / 4)
            fresh.order(ByteOrder.LITTLE_ENDIAN).asIntBuffer().get(words)
            val ints = IntBuffer.wrap(words)
            // The pictures this stream announces, made here before the main thread needs them.
            val pics = Native.pendingPictures()
            if (pics.isNotEmpty()) {
                android.os.Trace.beginSection("exact pictures")
                for (k in pics.indices step 3) if (!made.containsKey(pics[k])) made[pics[k]] = makePicture(pics[k], pics[k + 1], pics[k + 2], hwbuf)
                android.os.Trace.endSection()
            }
            queue.add(Pub(ints, ints.limit() > 0 && ints.get(0) == 22, seq, Native.feed()))
            if (!notifying.getAndSet(true)) main.post { notifying.set(false); published() }
        }
        val now = Native.editing()
        if (now != editing) { editing = now; main.post { editingChanged() } }
        schedule()
    }

    private val turn = Choreographer.FrameCallback { turnPosted = false; produce() }
    /** Timers, and the paint a scroll owes once it pauses (only then the collection pass it left behind). */
    private val timer = Runnable {
        if (prefetch && Native.owed() && Native.refine(-1, if (led) velocity else 0f) and 2 != 0) handler.post(refiner)
        produce()
    }

    private fun schedule() {
        if (Native.animating() && !turnPosted) { turnPosted = true; choreographer?.postFrameCallback(turn) }
        var due = Native.nextDue()
        // A moved paint is owed a paint once scrolling pauses: each step pushes this back.
        if (Native.owed() && (due < 0 || due > 16)) due = 16.0
        handler.removeCallbacks(timer)
        if (due >= 0) handler.postDelayed(timer, due.toLong().coerceAtLeast(0))
    }
}

/** dp/s from which a scroll leads, and below which it stops leading; the velocity a lead passes (px/s: past any
 * viewport's cap, two viewports at a quarter second of travel). */
private const val LEAD_ON = 8000f
private const val LEAD_OFF = 4000f
private const val LEAD_PX = 1e6f
/** ms without a scroll step after which a leading window stops leading. */
private const val SETTLE_MS = 100L
/** A slice of laying a press's screen out ahead: the microseconds it may shape for. */
private const val AHEAD_US = 1500

/** Calls the reader makes outside [ExactView]'s own (keys, Back, GPU surfaces): to the [Owner] when there is one. */
internal object Exact {
    var owner: Owner? = null

    /** Main thread, when an answer was published in time (the reader draws it this frame). */
    var answered: () -> Unit = {}

    fun key(code: Int, backspace: Boolean) {
        val o = owner
        if (o != null) { o.input(); if (o.answer(KEY_WAIT_MS) { Native.key(code, backspace) }) answered() } else Native.key(code, backspace)
    }

    /** How long a press or release waits for exact2's answer before the frame goes without it: what is left of a
     * 120 Hz frame after input (a longer wait holds the frame; 16 ms cost 9-15 janky frames per daily scenario). */
    const val WAIT_MS = 5L
    /** A key's wait: typing sends them in bursts. */
    const val KEY_WAIT_MS = 2L

    fun editing(): Int = owner?.editing ?: Native.editing()

    fun back(): Boolean = owner?.sync { Native.back() } ?: Native.back()

    /** Give a GPU canvas its window: on the exact thread, without waiting for it; [done] there with the handle. */
    fun attachSurface(view: Int, surface: android.view.Surface, width: Int, height: Int, done: (Long) -> Unit) {
        val o = owner
        if (o != null) o.call { done(Native.attachSurface(view, surface, width, height)) } else done(Native.attachSurface(view, surface, width, height))
    }

    /** A GPU canvas that renders into buffers the reader draws: on the exact thread, not waited for. */
    fun attachBuffers(view: Int, width: Int, height: Int) {
        val o = owner
        if (o != null) o.call { Native.attachBuffers(view, width, height) } else Native.attachBuffers(view, width, height)
    }

    /** A buffered canvas goes, its buffers with it: on the exact thread, in order with the canvases attached after it,
     * and not waited for (detachwait=1: waited for, to compare). The main thread waited inside its ingest for whatever
     * the exact thread was in the middle of: 10-24 ms behind a collection pass in an xheavy fling, and the next frame
     * began that late (most of that list's janky frames). The buffers themselves are counted references: the reader's
     * bitmaps of them go when it drops them, exact2's when the canvas goes there. */
    var detachWait = false
    fun detachBuffers(view: Int) {
        val o = owner
        val f = { Native.detachSurface(view, 0L) }
        if (o == null) f() else if (detachWait) o.sync(f) else o.call(f)
    }

    /** Take a GPU canvas's window back, waiting for it; [window] (read on the exact thread) is the handle, 0 for none. */
    fun detachSurface(view: Int, window: () -> Long) {
        val o = owner
        val f = { val w = window(); if (w != 0L) Native.detachSurface(view, w) }
        if (o != null) o.sync(f) else f()
    }
}
