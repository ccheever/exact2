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
internal class Owner(private val hwbuf: Boolean, private val fast: Boolean, priority: Int, intent: (String) -> String?) {
    /** Called on the main thread when a stream was published: the view that reads this owner now. The owner outlives
     * an activity (its thread holds the host, which is the process's), so each activity's view takes it in turn. */
    @Volatile var published: () -> Unit = {}

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
     * includes, and the feed's group (0 before the first scroll). [scrollers]: what exact2 tells the reader of its
     * scrollers as of this stream ([Native.scrollers]), null when that is what it last told; with no [ops], that
     * statement alone. [fields]: what exact2 tells of the text fields the reader hosts as of this stream
     * ([Native.fields], LLP 1115.001 D8), null when that is what it last told. [bars]: what it tells of its bars as
     * of this stream ([Native.bars], D11), null likewise. [access]: what it tells of the accessibility tree
     * ([Native.access], D13), null likewise, and always while no accessibility service is on. */
    class Pub(
        val ops: IntBuffer?, val move: Boolean, val seq: Int, val feed: Int, val scrollers: IntArray?, val fields: IntArray?,
        val bars: IntArray?, val access: IntArray?,
    )

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
    /** Called on the main thread with what exact2 asks of the platform ([Native.requests], LLP 1115.001 D5): the
     * view's activity that reads this owner now acts on them ([Requests]). */
    @Volatile var asked: (IntArray) -> Unit = {}

    // The exact thread's own state.
    private var seq = 0
    private var steps = 0
    private var turnPosted = false
    /** The last turn took longer than a display frame: the next one is run at once, not at the next one. */
    private var late = false
    private var choreographer: Choreographer? = null
    private val prefetch = true

    // Scrolls the main thread sent that the exact thread has not taken: summed, so a busy thread catches up at once.
    private val lock = Any()
    private var owedDy = 0f
    private var owedSeq = 0
    private var stepPosted = false
    /** A finger's moves the exact thread has not taken, summed by scroller: how far, and the revision known. */
    private class Moved(var dx: Float, var dy: Float, var rev: Int)
    private val owedMoves = LinkedHashMap<Int, Moved>()
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

    /** Whether the host's wake descriptors are watched: once, however many readers boot it. */
    private var watching = false

    /** Whether an accessibility service is on, and whether the reader holds nothing of the tree yet (LLP 1115.001
     * D13): the exact thread's own. exact2 is asked for the tree with each stream only while one is on. */
    private var accessible = false
    private var accessAgain = false

    /** An accessibility service came on ([on]), or the last one went: the reader is told the tree whole, or exact2
     * forgets it. */
    fun access(on: Boolean) {
        handler.post {
            if (on) { accessible = true; accessAgain = true; if (watching) produce() }
            else if (accessible) { accessible = false; try { Native.accessOff() } catch (e: UnsatisfiedLinkError) {} }
        }
    }

    /** Boot at the size the view is expected to have; [done] on the main thread with whether it booted. An activity
     * made again in this process (LLP 1115.001 D15.1) calls this for its own view: [Native.start] then adopts the host
     * that runs on this thread, which paints everything again for the new reader, and starts no second one. */
    fun boot(width: Int, height: Int, scale: Float, dir: String, live: Boolean, done: (Boolean) -> Unit) {
        handler.post {
            android.os.Trace.beginSection("exact boot")
            try { Native.hardwarePictures(hwbuf) } catch (e: UnsatisfiedLinkError) { }
            val ok = Native.start(width, height, scale, dir, live)
            android.os.Trace.endSection()
            if (ok) {
                // What was published or scrolled for the reader before this one is not this one's.
                queue.clear(); made.clear(); seq = 0
                accessAgain = true
                synchronized(lock) { owedDy = 0f; owedSeq = 0 }
                if (!watching) {
                    watching = true
                    val wakes = thread.looper.queue
                    for (fd in Native.fds()) {
                        if (fd < 0) continue
                        val pfd = ParcelFileDescriptor.fromFd(fd)
                        wakes.addOnFileDescriptorEventListener(pfd.fileDescriptor, MessageQueue.OnFileDescriptorEventListener.EVENT_INPUT) { _, _ ->
                            // A reply, a picture, or a request exact2 queued with no frame to follow it.
                            if (Native.poll()) produce() else { requests(); schedule() }
                            MessageQueue.OnFileDescriptorEventListener.EVENT_INPUT
                        }
                    }
                }
                produce()
            }
            main.post { done(ok) }
        }
    }

    /** The device's settings ([Device]): told before [boot] so the host boots with them (the library loads here, not on
     * the main thread), and whenever they change, when the host takes them and publishes what they changed. */
    fun configure(device: Device) {
        handler.post { device.tell(); if (watching) produce() }
    }

    /** The device's colours ([Palette]): told before [boot] so the host's first frame is in them, and whenever they
     * change, when the host paints the page again. */
    fun colors(roles: IntArray) {
        handler.post { Native.colors(roles); if (watching) produce() }
    }

    /** What exact2 asks of the platform since it was last asked ([Native.requests]): handed to the main thread, which
     * acts on it. After every turn, and when exact2 wakes this thread for a request no frame follows (a haptic
     * changes no pixel); also while the activity is stopped, when no stream is made. */
    private fun requests() {
        Native.requests()?.let { r -> main.post { asked(r) } }
    }

    /** The window ([Native.window]): told before [boot] so the host is laid out in it from its first frame, and
     * whenever its size, the display's density or an inset changes, when the host lays the page out again. */
    fun window(width: Int, height: Int, scale: Float, insets: IntArray) {
        handler.post { Native.window(width, height, scale, insets); if (watching) produce() }
    }

    /** Called on the main thread with what the host says to draw around the page ([Native.chrome]) when that
     * changed: asked after each stream, so it is drawn with the frame it belongs to. */
    @Volatile var chrome: (IntArray) -> Unit = {}

    /** Whether the activity is started. While it is stopped the host does no frame work (LLP 1115.001 D15.2): no
     * turn at a vsync, no timer, no stream. Replies are still drained as they arrive ([Native.poll]), so what they
     * change is in the frame drawn when the activity starts again. */
    private var running = true

    /** The activity stopped ([on] false) or started again. */
    fun running(on: Boolean) {
        handler.post {
            if (on == running) return@post
            running = on
            if (on) { produce(); return@post }
            choreographer?.removeFrameCallback(turn); turnPosted = false
            handler.removeCallbacks(again)
            handler.removeCallbacks(timer); handler.removeCallbacks(refiner); handler.removeCallbacks(settle)
        }
    }

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

    /** The reader moved scroller [id] by ([dx], [dy]) px, knowing revision [rev] of it ([Scrolling]); the main
     * thread's [seq]th scroll. Summed like a driver's, so a busy thread catches up at once. */
    fun scrolled(id: Int, dx: Float, dy: Float, rev: Int, seq: Int) {
        synchronized(lock) {
            val m = owedMoves.getOrPut(id) { Moved(0f, 0f, rev) }
            // What was sent under an older revision is dropped by exact2 anyway.
            if (m.rev != rev) { m.dx = 0f; m.dy = 0f; m.rev = rev }
            m.dx += dx; m.dy += dy
            owedSeq = seq
            if (stepPosted) return
            stepPosted = true
        }
        handler.post(step)
    }

    /** What is under a point of the view ([Native.under]), asked on the exact thread behind the touch that landed
     * there. [done] on the main thread with the answer, after the statement of the scrollers it names is published,
     * so the reader knows where each is before it decides anything. */
    fun under(x: Float, y: Float, done: (IntArray) -> Unit) {
        handler.post { val u = Native.under(x, y); produce(); main.post { done(u) } }
    }

    private val step = Runnable {
        val dy: Float; val s: Int
        var ids: IntArray? = null; var moves: Array<Moved>? = null
        synchronized(lock) {
            dy = owedDy; s = owedSeq; owedDy = 0f; stepPosted = false
            if (owedMoves.isNotEmpty()) { ids = owedMoves.keys.toIntArray(); moves = owedMoves.values.toTypedArray(); owedMoves.clear() }
        }
        android.os.Trace.beginSection("exact scroll")
        val fingers = moves
        if (fingers != null) for (k in fingers.indices) Native.scrolled(ids!![k], fingers[k].dx, fingers[k].dy, fingers[k].rev)
        if (fingers == null || dy != 0f) Native.scroll(dy)
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
    fun touch(action: Int, x: Float, y: Float, tool: Int, waitMs: Long): Boolean = answer(waitMs) {
        Native.touch(action, x, y, tool)
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
        if (!running) { requests(); return }
        android.os.Trace.beginSection("exact native frame")
        val started = System.nanoTime()
        val fresh = Native.frame()
        late = System.nanoTime() - started > frameNanos
        android.os.Trace.endSection()
        val told = Native.scrollers()
        val fields = Native.fields()
        val bars = Native.bars()
        // A host library without the call (an older exact2) is not asked again.
        val access = if (accessible) try { Native.access(accessAgain).also { accessAgain = false } } catch (e: UnsatisfiedLinkError) { accessible = false; null } else null
        if (fresh == null && (told != null || fields != null || bars != null || access != null)) {
            queue.add(Pub(null, true, seq, 0, told, fields, bars, access))
            if (!notifying.getAndSet(true)) main.post { notifying.set(false); published() }
        }
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
            queue.add(Pub(ints, ints.limit() > 0 && ints.get(0) == 22, seq, Native.feed(), told, fields, bars, access))
            if (!notifying.getAndSet(true)) main.post { notifying.set(false); published() }
        }
        Native.chrome()?.let { c -> main.post { chrome(c) } }
        requests()
        schedule()
    }

    private val turn = Choreographer.FrameCallback { t ->
        // The display's frame: the shortest gap between consecutive vsyncs seen.
        if (vsync > 0 && t - vsync > 2_000_000) frameNanos = minOf(frameNanos, t - vsync)
        vsync = t
        turnPosted = false; produce()
    }
    private var vsync = 0L
    private var frameNanos = 1_000_000_000L / 60
    /** A turn that overran its display frame (a GPU canvas whose world ran two ticks in it, 256,000 cubes on a
     * Pixel 5) waits for no vsync: Choreographer would skip the one it missed, and every frame after would take
     * two. The canvas's buffers hold at most three frames on the GPU, and while they do a turn only runs its ticks. */
    private val again = Runnable { produce() }
    /** Timers, and the paint a scroll owes once it pauses (only then the collection pass it left behind). */
    private val timer = Runnable {
        if (prefetch && Native.owed() && Native.refine(-1, if (led) velocity else 0f) and 2 != 0) handler.post(refiner)
        produce()
    }

    private fun schedule() {
        if (!running) return
        if (Native.animating() && late) { late = false; handler.post(again) }
        else if (Native.animating() && !turnPosted) { turnPosted = true; choreographer?.postFrameCallback(turn) }
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
    /** The process's owner, from the first activity that made one on: an activity made again takes it. */
    var owner: Owner? = null
    /** Whether an activity has set this process up for a host (its environment is written, its host may run). */
    var live = false
    /** The activity whose view reads the host now. */
    var reader: java.lang.ref.WeakReference<ExactActivity>? = null

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

    fun back(): Boolean = owner?.sync { Native.back() } ?: Native.back()

    /** A driver's request ([Native.agent], [AgentDrive]): answered on exact2's thread, which then publishes what it
     * painted; waited for, since the driver's next request follows its answer. Carried only where exact2 has a
     * thread of its own (not with owner=0, the benchmark's compare switch). */
    fun agent(line: String): String {
        val o = owner ?: return "{\"error\":\"the reader carries a driver only with exact2 on its own thread (owner=0 is set)\"}"
        return o.sync { String(Native.agent(line.toByteArray(Charsets.UTF_8)), Charsets.UTF_8) }
    }

    /** Main thread, where exact2 runs there too (owner=0) and a hosted field told it something: the reader draws
     * what that changed. */
    var turned: () -> Unit = {}

    /** What a hosted text field tells exact2 ([Fields], LLP 1115.001 D8), on exact2's thread and in order. */
    private fun field(f: () -> Unit) {
        val o = owner
        if (o != null) { o.input(); o.call(f) } else { f(); turned() }
    }

    fun fieldFocused(id: Int, generation: Int, focused: Boolean) = field { Native.fieldFocused(id, generation, focused) }
    fun fieldEdited(id: Int, generation: Int, seq: Int, value: String, start: Int, end: Int) =
        field { Native.fieldEdited(id, generation, seq, value, start, end) }
    fun fieldComposing(id: Int, generation: Int, composing: Boolean, value: String) =
        field { Native.fieldComposing(id, generation, composing, value) }
    fun fieldSelected(id: Int, generation: Int, start: Int, end: Int) = field { Native.fieldSelected(id, generation, start, end) }
    fun fieldAction(id: Int, generation: Int, action: Int) = field { Native.fieldAction(id, generation, action) }

    /** What happened in the window of surface [serial] of the top layer ([Surfaces], [Native.surface], LLP 1115.001
     * D10): told on exact2's thread, in order. exact2 answers with the surface painted again or closed (a request,
     * [Requests]) and with whatever a chosen row's handler changed in the page (a stream). */
    fun surface(serial: Int, what: Int, x: Float, y: Float) {
        val o = owner
        if (o != null) { if (what != 2) o.input(); o.call { Native.surface(serial, what, x, y) } }
        else { Native.surface(serial, what, x, y); turned() }
    }

    /** What happened in bar [which] ([Bars], [Native.bar], LLP 1115.001 D11): told on exact2's thread, in order.
     * exact2 answers with the bar painted again (a target lit) and with whatever the press changed in the page. */
    fun bar(which: Int, what: Int, x: Float, y: Float) {
        val o = owner
        if (o != null) { if (what != 2) o.input(); o.call { Native.bar(which, what, x, y) } }
        else { Native.bar(which, what, x, y); turned() }
    }

    /** What an accessibility service asks of node [id] of the tree exact2 last told ([Access], [Native.accessAct],
     * LLP 1115.001 D13): on exact2's thread, and waited for, since the service is answered with whether it was
     * done. exact2 then publishes what it changed. */
    fun accessAct(id: Int, action: Int, value: Float): Boolean {
        val o = owner
        return if (o != null) { o.input(); o.sync { Native.accessAct(id, action, value) } } else Native.accessAct(id, action, value).also { turned() }
    }

    /** What a hatch's handle asks of node [node] ([ExactElement]): on exact2's thread, waited for, as an
     * accessibility service's act is; exact2 runs it between its turns and publishes what it changed. */
    fun hatchAct(node: Int, kind: Int, text: String?): Boolean {
        val o = owner
        return if (o != null) { o.input(); o.sync { Native.hatchAct(node, kind, text) } } else Native.hatchAct(node, kind, text).also { turned() }
    }

    /** When the last touch lifted, or was held long enough to be a long press ([android.os.SystemClock.uptimeMillis]):
     * what a surface's first frame is timed from (`x_log_surfaces=1`). */
    var touched = 0L

    /** The answer to a request exact2 made ([Native.answer]), or its cancellation (null): on exact2's thread. */
    fun answer(id: Int, answer: String?) {
        val o = owner
        if (o != null) o.call { Native.answer(id, answer) } else Native.answer(id, answer)
    }

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
