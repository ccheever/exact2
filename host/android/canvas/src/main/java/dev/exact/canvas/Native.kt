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
        boldText: Boolean, highContrast: Boolean, touch: Boolean, touchSlop: Float,
    )
    /** The device's colours ([Palette], LLP 1115.001 D4): Material 3's colour roles as the reader resolved them for
     * this device, twelve under the light scheme and then the same twelve under the dark one, ARGB. Before [start],
     * from any thread, so the host boots resolving every colour role by them, and again on the host's thread whenever
     * one changes. */
    @JvmStatic external fun colors(roles: IntArray)
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
    /** A driver from outside scrolls the feed by [dy] px ([ExactActivity.drive]). A finger's scroll is the three
     * calls below. */
    @JvmStatic external fun scroll(dy: Float)
    /** What is under a point of the view (pixels), for the reader to decide what a drag from there is (LLP 1115.001
     * D7.3) and whose a touch there is (D8): the axes exact2's own gestures there claim (bit 0 sideways, bit 1 up
     * and down), a count, the hosted text field the touch is for (0: none; [Fields]), and eleven numbers a scroller,
     * innermost first: its id, flags (bit 0 it takes a sideways drag from here, bit 1 one up or down, bit 2 its
     * rows are a node the reader can move), its revision, then as float bits its offset (x, y), its farthest offset
     * (x, y) and the box it shows in (x, y, width, height). exact2 tells the reader of each of them from then on
     * ([scrollers]). */
    @JvmStatic external fun under(x: Float, y: Float): IntArray
    /** What exact2 tells the reader of the text fields it hosts (LLP 1115.001 D8), when that is not what this last
     * returned (else null): a count, then for each field, in tree order: its id, its generation (moved on when its
     * node became another row's item: the view starts over), flags (bit 0 it was painted and has a place, bit 1 it
     * takes no touch and no focus) and the scroller whose rows it was painted among (0: none); as float bits its
     * content box and the clip it shows through (x, y, width, height each; a negative clip width: none); the count
     * of the reader's edits that reached exact2 and the value the app shows as of it (a string: its bytes' count,
     * then UTF-8 padded to whole words); its look, eighteen words (`Presenter::reader_fields` lists them), and three
     * strings: its placeholder, its `autocomplete` and its font's file. A field it leaves out is gone. */
    @JvmStatic external fun fields(): IntArray?
    /** Hosted field [id], known at [generation], took the focus (a finger tapped it) or lost it. On the host's
     * thread, as every `field` call is; what one says of a generation exact2 has moved on from is dropped. */
    @JvmStatic external fun fieldFocused(id: Int, generation: Int, focused: Boolean)
    /** Hosted field [id] holds [value] after the person's edit number [seq], its selection at [start] to [end]
     * (UTF-16 units). Composing text is part of the value: exact2 fires `input` as the text composes. */
    @JvmStatic external fun fieldEdited(id: Int, generation: Int, seq: Int, value: String, start: Int, end: Int)
    /** Hosted field [id] moved its selection with no edit. */
    @JvmStatic external fun fieldSelected(id: Int, generation: Int, start: Int, end: Int)
    /** Hosted field [id] had its action key pressed, or Enter or Tab on a hardware keyboard. [action] is what the
     * key said it does: 0 Enter with no action, 2 done, 3 go, 4 next, 5 previous, 6 search, 7 send, 16 Tab, 17
     * Shift+Tab. exact2 does that: Enter's `key` handlers, `change` and `submit`, then the move or the leave. */
    @JvmStatic external fun fieldAction(id: Int, generation: Int, action: Int)
    /** The reader moved scroller [id] by ([dx], [dy]) px, knowing revision [rev] of it. exact2 clamps the move to
     * the scroller's travel, and drops one sent under a revision the app's own scroll has moved on from. */
    @JvmStatic external fun scrolled(id: Int, dx: Float, dy: Float, rev: Int)
    /** What exact2 tells the reader of the scrollers it knows, when that is not what this last returned (else
     * null): nine numbers each: its id, flags (as [under]'s, for any drag), its revision, then as float bits where
     * it is (x, y), the offset its rows were last painted at (x, y) and its farthest offset (x, y). The revision
     * moves on when the app scrolled it (a `scrollTop` write, a reveal): the reader then takes this offset and ends
     * its fling. A scroller the reader knew that is not among them is gone. */
    @JvmStatic external fun scrollers(): IntArray?
    /** A touch (0 down, 1 up, 2 move, 3 cancel) at the view's pixels, and what touched: the event's tool type
     * ([android.view.MotionEvent.getToolType]). A finger is reported to the app as a touch that hovers nothing and
     * leaves no focus ring, a mouse as a mouse (LLP 1115.001 D7.4). Action 4 is no new touch: the one that is down
     * has been held still for the system's long-press time ([android.view.ViewConfiguration.getLongPressTimeout]).
     * What names a context menu there opens it (D10), and that touch then presses nothing when it lifts. */
    @JvmStatic external fun touch(action: Int, x: Float, y: Float, tool: Int)
    /** What happened in the window the reader made for surface [serial] of the top layer ([Surfaces], LLP 1115.001
     * D10), on the host's thread: a touch ([what] 0 down, 1 up, 2 move, 3 cancel) at ([x], [y]), pixels from the
     * window's own top left; the system dismissed the window (8: a tap outside a menu, Back, Escape); the reader
     * could not make it (9); a key (16 Enter, 17 Tab, 18 Shift+Tab, 19 to 22 the arrows up, down, left and right, 23
     * Space). What is said of a surface that is no longer the one open is dropped. */
    @JvmStatic external fun surface(serial: Int, what: Int, x: Float, y: Float)
    /** What exact2 tells the reader of its bars ([Bars], LLP 1115.001 D11), when that is not what this last returned
     * (else null): a route's header as Material's top app bar and the navigation root's tablist as its navigation
     * bar, each painted by exact2 and shown over the page. A count, then for each bar which it is (1 the top app
     * bar, 2 the navigation bar), its frame in the window (x, y, width, height, pixels), a count of words and that
     * many: its picture, an op stream of its own, whole. A bar a statement leaves out is gone. Taken with each
     * stream, so a bar and the page under it change in one frame. */
    @JvmStatic external fun bars(): IntArray?
    /** What happened in bar [which] (1 the top app bar, 2 the navigation bar), on the host's thread: a touch ([what]
     * 0 down, 1 up, 2 move, 3 cancel) at ([x], [y]), pixels from the bar's own top left. exact2 knows what is
     * there: the Up arrow goes Back, a header's button and a destination are pressed. */
    @JvmStatic external fun bar(which: Int, what: Int, x: Float, y: Float)
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
    /** A key at the focused node when no hosted field holds the focus: a character (or '\n' for Enter), or
     * Backspace. A hosted field ([Fields]) takes its keys itself. */
    @JvmStatic external fun key(code: Int, backspace: Boolean)
    /** System Back (LLP 1115.001 D6): whether the app took it. From its navigation root's selected route it goes to
     * the Back control the root names, else to the root's `navigate` handler, else it is the runner's own `back`;
     * false when there is nowhere to go back to, and the activity finishes. The reader takes Back only while the
     * host says it has somewhere to go (a request, [requests]). */
    @JvmStatic external fun back(): Boolean
    /** What exact2 asks of the platform since this last returned any (LLP 1115.001 D5), or null when it asks
     * nothing: the one channel out, which [Requests] reads and acts on, on the main thread. exact2 queues a request
     * and makes the first of [fds] readable, so the reader comes for it whether or not a frame follows. The words:
     * for each request its kind, its id, the count of words that follow, and those words. */
    @JvmStatic external fun requests(): IntArray?
    /** The answer to request [id] (the share sheet's: `shared` or `dismissed`, or why it could not be shown), or null
     * when it was cancelled. On the host's thread. */
    @JvmStatic external fun answer(id: Int, answer: String?)
    /** One request of the agent API (LLP 1012), a JSON line as UTF-8, and its answer the same way: the Linux
     * host's own handler (`host/linux/src/canvas/agent.rs`), on the host's thread. From the first one the host's turns
     * run at the driver's clock, which only a `clock` request moves. [AgentDrive] calls it for a driver over adb. */
    @JvmStatic external fun agent(line: ByteArray): ByteArray
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
    /** The feed: the scroller a finger last moved up or down, or a driver's [scroll] found (its group's id in the
     * stream); 0 before either. */
    @JvmStatic external fun feed(): Int
}
