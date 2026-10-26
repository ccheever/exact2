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
        boldText: Boolean, highContrast: Boolean, touch: Boolean, tv: Boolean, touchSlop: Float, dates: String,
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
    @JvmStatic external fun acquired(running: Boolean)
    @JvmStatic external fun displayFrame()
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
     * ([scrollers]). After the scrollers: the native module button under the point (0: none), then its painted
     * box x, y, width and height as float bits, in pixels, its allocation slot and generation, and how many long
     * presses a node has taken so far (wrapping). */
    @JvmStatic external fun under(x: Float, y: Float): IntArray
    /** What exact2 tells the reader of the text fields it hosts (LLP 1115.001 D8), when that is not what this last
     * returned (else null): a count, then for each field, in tree order: its id, its generation (moved on when its
     * node became another row's item: the view starts over), flags (bit 0 it was painted and has a place, bit 1 it
     * takes no touch and no focus) and the scroller whose rows it was painted among (0: none); as float bits its
     * content box and the clip it shows through (x, y, width, height each; a negative clip width: none); the count
     * of the reader's edits that reached exact2 and the value the app shows as of it (a string: its bytes' count,
     * then UTF-8 padded to whole words); its look, eighteen words (`Presenter::reader_fields` lists them), and four
     * strings: its placeholder, its `autocomplete`, its font's file and its axes. A field it leaves out is gone. */
    @JvmStatic external fun fields(): IntArray?
    /** Hosted field [id], known at [generation], took the focus (a finger tapped it) or lost it. On the host's
     * thread, as every `field` call is; what one says of a generation exact2 has moved on from is dropped. */
    @JvmStatic external fun fieldFocused(id: Int, generation: Int, focused: Boolean)
    /** Hosted field [id], known at [generation], had real input that may change nothing exact2 hears of: a finger
     * down in it, a key down. A state held on it (LLP 1122 D8) lets go. */
    @JvmStatic external fun fieldInput(id: Int, generation: Int)
    /** Hosted field [id] holds [value] after the person's edit number [seq], its selection at [start] to [end]
     * (UTF-16 units). Composing text is part of the value: exact2 fires `input` as the text composes. */
    @JvmStatic external fun fieldEdited(id: Int, generation: Int, seq: Int, value: String, start: Int, end: Int)
    /** Hosted field [id] began to compose, holding [value], or its composition ended, committed or cancelled: told
     * before the edit that began or ended it. While it composes exact2 holds a value the app writes and tells the
     * field its own text; once it ends, the value the app then shows (LLP 1115.000 §5, ruling 6). */
    @JvmStatic external fun fieldComposing(id: Int, generation: Int, composing: Boolean, value: String)
    /** Hosted field [id] moved its selection with no edit. */
    @JvmStatic external fun fieldSelected(id: Int, generation: Int, start: Int, end: Int)
    /** Hosted field [id] had its action key pressed, or Enter or Tab on a hardware keyboard. [action] is what the
     * key said it does: 0 Enter with no action, 2 done, 3 go, 4 next, 5 previous, 6 search, 7 send, 16 Tab, 17
     * Shift+Tab. exact2 does that: Enter's `key` handlers, `change` and `submit`, then the move or the leave. */
    @JvmStatic external fun fieldAction(id: Int, generation: Int, action: Int)
    /** A module event, checked against the allocation generation carried by its painted view. */
    @JvmStatic external fun moduleEvent(id: Int, slot: Int, generation: Int, kind: String, payload: String)
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
    /** What exact2 tells the reader of the accessibility tree ([Access], LLP 1115.001 D13), when that is not what
     * this last returned (else null). Called only while an accessibility service is on; exact2 builds nothing
     * otherwise. [again]: the reader holds nothing of the tree (it was made again, or a service was just turned on)
     * and is told it whole. The words: the statement's number; the surface whose window the nodes are in (its
     * serial; 0: the page's window); 1 when the tree is told whole (the only changes then listed speak); the
     * identity of the route that shows (0: none) and its title, a string (its bytes' count, then UTF-8 padded to
     * whole words); a count of changes and four words each (a node's identity, 0 for the view itself; what changed:
     * bit 0 its subtree, bit 1 its text, bit 2 its state, bit 3 it scrolled, bit 4 it is a live region that
     * speaks; and as float bits how far a scroller moved, x and y, in pixels); a count of nodes and for each, in
     * the order a person reads: its identity, its parent's (0: the view), its view, its role and its flags
     * ([Access]'s constants), its box (x, y, width, height, pixels: in the window, or in its own window for a
     * surface), as float bits its range's least, greatest and present value (a scroller's offset and its farthest),
     * its place in its set from 1 and the set's size, and three strings: its name, its value and its description. */
    @JvmStatic external fun access(again: Boolean): IntArray?
    /** An accessibility service acts on node [id] of the tree last told, on the host's thread: [action] 1 press
     * (the presenter's own press at the node, as a driver's `tap`), 2 hold (its context menu), 3 scroll a scroller
     * on or step a range up, 4 back or down, 5 set a range to [value]. Whether anything took it. */
    @JvmStatic external fun accessAct(id: Int, action: Int, value: Float): Boolean
    /** No accessibility service is on any more: exact2 forgets the tree it told and builds none until [access]. */
    @JvmStatic external fun accessOff()
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
    /** -1 attachment refused, 0 waiting, 1 first window frame submitted to the compositor. Any thread. */
    @JvmStatic external fun canvasReady(view: Int): Int
    /** That frame's buffer, or null. */
    @JvmStatic external fun canvasBuffer(view: Int): android.hardware.HardwareBuffer?
    /** A key at the focused node when no hosted field holds the focus: a character (or '\n' for Enter), or
     * Backspace. A hosted field ([Fields]) takes its keys itself. */
    @JvmStatic external fun key(code: Int, backspace: Boolean)
    /** A gamepad's state for the canvas a key would reach (LLP 1127.003 D2): [values] its 17 standard buttons, then
     * its 4 axes; [connected] false when it went away. */
    @JvmStatic external fun pad(index: Int, connected: Boolean, values: FloatArray)
    /** A remote's key, down or up, by the web's `code` and `key` (LLP 1115.001.003 D3): a D-pad arrow, or Select. */
    @JvmStatic external fun namedKey(code: String, key: String, down: Boolean, repeat: Boolean)
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
    /** A topic an extension announced ([ExtensionContext.changed]): its readers are asked again. On the host's thread. */
    @JvmStatic external fun changed(topic: String)
    /** The extensions the APK carries, one a line ([Extensions]); under the agent, told so unless the drive names them
     * (`--bind`). On the host's thread. */
    @JvmStatic external fun bindExtensions(names: String)
    /** How a presence check ended ([Presence]), on the store's completion path: never journaled. On the host's thread. */
    @JvmStatic external fun presence(answer: String)
    /** A package's picture for request [id] (LLP 1133.019 D11 item 1): [bitmap], software `ARGB_8888`, or why there is
     * none ([error]) when it is null. On the host's thread. */
    @JvmStatic external fun picture(id: Int, bitmap: android.graphics.Bitmap?, error: String?)
    /** Picture [source] changed, so it is asked for again, or was [revoked]: every box showing it is emptied. On the
     * host's thread. */
    @JvmStatic external fun pictureChanged(source: String, revoked: Boolean)
    /** The activity stopped (true) or started (false): the page's `visibilityState`. On the host's thread. */
    @JvmStatic external fun hidden(hidden: Boolean)
    /** What [Orientation] asked of the activity for the session's orientation lock (LLP 1133.029 §8), JSON, whether
     * its resolution found a conflict, and a line for the journal or null. On the host's thread. */
    @JvmStatic external fun orientationApplied(applied: String, conflict: Boolean, line: String?)
    /** The session's clock, ms, as of the host's last turn and moved on by the wall's outside the agent
     * ([ExtensionContext.now]). Any thread. */
    @JvmStatic external fun now(): Double

    /** The file an `app:/data|cache|tmp/…` path names, or null (`ExtensionContext.file`). */
    @JvmStatic external fun appFile(path: String): String?
    /** Whether extension [name] is told it runs under the agent ([ExtensionContext.agent]). Any thread. */
    @JvmStatic external fun underAgent(name: String): Boolean
    /** Whether the page places a canvas of surface [name], as of the host's last turn ([ExtensionSurfaces]). Any
     * thread. */
    @JvmStatic external fun surfaceHas(name: String): Boolean
    /** A package's sample for surface [surface]'s canvas input (LLP 1133.013 D6), or with [ended] its end: kept until
     * the host's next display frame delivers it to the canvas. [values] x, y, z and w; [at] the session clock's ms.
     * Any thread; it waits for nothing. Whether none waited before it: the host is then woken for a frame. */
    @JvmStatic external fun sensorInput(surface: String, sensor: String, values: DoubleArray, dropped: Int, at: Double, ended: Boolean): Boolean
    /** What the app's Kotlin hatches ask, for node [node] or 0 for their module (LLP 1115.001.000 D4; the kinds are
     * `hatches::reader::act`'s): its answer, a ticket for a registration, -1 when refused. */
    @JvmStatic external fun hatchAct(node: Int, kind: Int, text: String?): Long
    /** One request of the agent API (LLP 1012), a JSON line as UTF-8, and its answer the same way: the Linux
     * host's own handler (`host/linux/src/canvas/agent.rs`), on the host's thread. From the first one the host's turns
     * run at the driver's clock, which only a `clock` request moves. [AgentDrive] calls it for a driver over adb. */
    @JvmStatic external fun agent(line: ByteArray): ByteArray
    /** What an [ExactTamber] asks (LLP 1121 D12), on the host's thread: `{"view": id or 0, "names": [..]}`, answered
     * `{"inputs": "<u64>", "colors": {name: ARGB or null}}`, each colour as exact2 paints it at that view (its pinned
     * modes, its scheme), null for a name the design lacks; `inputs` moves when anything the answer read does. A
     * view that is no live node, or a name that is no string, is answered `{"error": ...}`. */
    @JvmStatic external fun tamber(request: String): String
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
