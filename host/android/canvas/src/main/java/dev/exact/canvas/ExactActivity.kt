package dev.exact.canvas

import android.app.Activity
import android.content.Context
import android.graphics.Color
import android.os.Bundle
import android.view.View
import android.view.ViewGroup
import android.widget.FrameLayout
import java.io.File

/**
 * What drives an app from outside the reader: a benchmark's scroll and tap driver, a test's. The reader has none of
 * its own and waits on none. An activity attaches one by overriding [ExactActivity.drive].
 */
fun interface Drive {
    /** The app's first content has been drawn: called once, on the main thread. */
    fun contentReady()
}

/**
 * One exact2 app on the Canvas host. Launch extras: `freeze=1` (BENCH_FREEZE), `start=N` (BENCH_START_INDEX),
 * `kinds=…` (BENCH_KINDS); `scenario` reaches the data source as BENCH_SCENARIO, `live` as BENCH_LIVE.
 */
open class ExactActivity : Activity() {
    /** The app's native modules (LLP 1024), by name. */
    protected open val modules: Map<String, NativeModule> = emptyMap()
    /** The app's `video` player ([VideoPlayer]); none for an app without video. */
    protected open val video: ((Context) -> VideoPlayer)? = null
    private var natives: Natives? = null
    /** The app's text fields, each the platform's own ([Fields], LLP 1115.001 D8). */
    private var fields: Fields? = null
    /** The row pitch `start=N` scrolls by (0: the app's plan scrolls to it itself). */
    protected open val startPitchDp = 0f
    /** The app has text fields: keys and the software keyboard reach it ([Keys]). */
    protected open val input = false
    /**
     * The seam a driver attaches at ([Drive]); the default attaches none. Called once, as the activity is made, when
     * the host has begun to boot and before the view is made: a driver sets the window up and reads its own launch
     * extras here. [scroll] moves the app's feed by a distance in pixels ([ExactView.scrollBy]) and may be called once
     * [onCreate] has returned.
     */
    protected open fun drive(scroll: (Float) -> Unit): Drive? = null
    private var driver: Drive? = null
    private lateinit var view: ExactView

    override fun onCreate(state: Bundle?) {
        // An activity made again in a process whose host runs (LLP 1115.001 D15.1: a configuration change this
        // activity does not declare, or one the system destroyed in the background) starts no second host: its view
        // adopts the one that runs ([Owner.boot], [Native.start]). The environment below is read once, as the library
        // loads and the host boots, and other threads read it from then on, so it is written only the first time.
        val again = Exact.live
        Exact.live = true
        // An activity this one replaces that is not destroyed yet (it finished; its onDestroy comes late) lets go of
        // the host now, before this one's view takes it.
        Exact.reader?.get()?.takeIf { it !== this }?.release()
        Exact.reader = java.lang.ref.WeakReference(this)
        // Before anything loads exact2's library: mimalloc reads its options as it loads. Freed memory goes back at
        // once (decoded pictures otherwise stay held).
        if (!again) android.system.Os.setenv("MIMALLOC_PURGE_DELAY", "0", false)
        super.onCreate(state)
        // The starting window goes the moment the first frame is drawn, not after its exit fade (splashfade=1: the
        // platform's fade, to compare): the first frame is what shows.
        if (intent.getStringExtra("splashfade") != "1") splashScreen.setOnExitAnimationListener { it.remove() }
        if (!again) environment()
        edgeToEdge()
        // exact2's thread, made before the assets are checked: it loads exact2's library meanwhile.
        owner()
        installAssets(File(storage, "app"))
        // What the host asks of the platform (LLP 1115.001 D5), acted on here, on the main thread.
        requests = Requests(this).also { it.back = { goes -> takeBack(goes) } }
        // No background of its own: the window's shows behind the page, the launch theme's surface until the host says
        // the page's ([chrome]).
        root = Root(this)
        // The top layer's windows (LLP 1115.001 D10) are this activity's, and outlive its reader.
        requests.surfaces = Surfaces(this, root).also { it.covering = { on -> fields?.covered(on) } }
        read()
        // The raw insets go to the host whenever they change, and the window's size when it does; nothing is padded
        // here (D3.3).
        root.setOnApplyWindowInsetsListener { _, insets -> window(insets); insets }
        root.addOnLayoutChangeListener { _, l, t, r, b, ol, ot, or, ob ->
            if (r - l != or - ol || b - t != ob - ot) root.rootWindowInsets?.let { window(it) }
        }
        setContentView(root)
        accessibility.addAccessibilityStateChangeListener(accessWatch)
    }

    /** Whether an accessibility service is on (LLP 1115.001 D13). The reader answers one through its root
     * ([Root], [Access]) and asks exact2 for the tree only while one is on: with none, exact2 is never asked and
     * builds nothing. */
    private val accessibility by lazy { getSystemService(android.view.accessibility.AccessibilityManager::class.java) }
    private val accessWatch = android.view.accessibility.AccessibilityManager.AccessibilityStateChangeListener { on -> accessible(on) }

    /** A service came on, or the last one went; or this activity's reader was made ([read]) and is told which. */
    private fun accessible(on: Boolean) {
        if (!reads() || !::view.isInitialized) return
        if (on == root.accessible && !on) return
        root.accessible = on
        if (!on) { root.access.clear(); requests.surfaces?.access(null) }
        val owner = view.owner
        if (owner != null) owner.access(on) else view.accessible(on)
    }

    /** exact2's statement of the accessibility tree ([Native.access]): the page's goes to the root, a surface's to
     * that surface's window, and the page under an open surface then has nothing that can be reached. */
    private fun accessTold(words: IntArray) {
        val told = Access.Told(words)
        if (told.surface == 0) { requests.surfaces?.access(null); root.access.told(told) }
        else { root.access.clear(); requests.surfaces?.access(told) }
    }

    private lateinit var requests: Requests

    /** System Back goes where the app's routes send it ([Native.back], LLP 1115.001 D6). The callback is registered
     * only while the host says Back has somewhere to go in the app ([Requests.back]): at the first screen there is
     * none, so Back is the system's own, which leaves the app with its own animation. A Back that arrives and the
     * app does not take (it had somewhere to go a moment ago) finishes the activity, as it always did. */
    private val backCallback = android.window.OnBackInvokedCallback { if (Exact.back()) view.invalidate() else finish() }
    private var takesBack = false
    private fun takeBack(on: Boolean) {
        if (on == takesBack) return
        takesBack = on
        if (on) onBackInvokedDispatcher.registerOnBackInvokedCallback(android.window.OnBackInvokedDispatcher.PRIORITY_DEFAULT, backCallback)
        else onBackInvokedDispatcher.unregisterOnBackInvokedCallback(backCallback)
    }

    /** Where exact2 keeps the app's files, and the APK's assets are copied: the app's own files directory, which
     * the system keeps encrypted until the phone is first unlocked, as any app's is. */
    private val storage: File by lazy { filesDir }
    /** A driver was asked for, once ([drive]), and told of the first content once, whatever view draws it again. */
    private var driven = false
    private var ready = false

    /** exact2's own thread ([Owner]), the process's one: its thread holds the host, which a reader made again adopts.
     * Null with owner=0: exact2 on the main thread (booting on its own thread first unless asyncboot=0), to compare. */
    private fun owner(): Owner? {
        if (intent.getStringExtra("owner") == "0") return null
        // exactprio=display: exact2's thread above the main thread (THREAD_PRIORITY_DISPLAY), to compare.
        val priority = if (intent.getStringExtra("exactprio") == "display") android.os.Process.THREAD_PRIORITY_DISPLAY
            else android.os.Process.THREAD_PRIORITY_DEFAULT
        return Exact.owner ?: Owner(intent.getStringExtra("hwbuf") != "0", intent.getStringExtra("fast") != "0", priority, intent::getStringExtra).also { Exact.owner = it }
    }

    /**
     * Make this activity's reader, the view that replays the host's streams with the hosted views under it, and give
     * it the host: booted now, or adopted where one runs already ([Native.start]). Called when the activity is made,
     * and again each time it starts after a stop (LLP 1115.001 D15.2): Android discards a stopped window's display
     * lists, the rows and pictures the reader kept among them, so the reader that comes back holds nothing the host
     * sent it. A new one adopts the host, which paints everything again for it; the app is as it was.
     */
    private fun read() {
        natives?.destroyAll()
        fields?.leaving()
        root.removeAllViews()
        // Before the host boots: its first layout measures the app's text fields in a field of this activity's
        // theme ([Fields.measure], LLP 1115.001 D8).
        Fields.context = this
        val owner = owner()?.also { it.published = { view.onPublished() } }
        // The device's settings, told before the host boots: its first frame is laid out and painted with them. Its
        // colours too (D4): read again by each reader, since a wallpaper changed while the last one was stopped is
        // another palette.
        val device = Device.read(this).also { told = it }
        val colors = Palette.read(this).also { toldColors = it }
        // The window as it is now, with the system's raw insets (D3.3): told before the host boots too, so the page is
        // laid out where it will be shown. The view's own size is never told: the presenter places the page.
        val metrics = windowManager.currentWindowMetrics
        val size = metrics.bounds.width() to metrics.bounds.height()
        val density = resources.displayMetrics.density
        val edges = edges(root.rootWindowInsets ?: metrics.windowInsets)
        toldWindow = Triple(size, density, edges.toList())
        // asyncboot=0: boot when the view is laid out, on the main thread, to compare. Otherwise now, on a thread that
        // also loads the library: exact2 loads, boots and paints its first frame while Android sets the activity up,
        // and the view takes it when it is laid out.
        var booting = false
        val live = intent.getStringExtra("live").let { it == "1" || it == "true" }
        if (owner != null) {
            owner.chrome = { c -> chrome(c) }
            owner.asked = { r -> requests.perform(r) }
            owner.configure(device)
            owner.colors(colors)
            owner.window(size.first, size.second, density, edges)
            owner.boot(size.first, size.second, density, storage.absolutePath, live) { ok -> view.ownerStarted(ok) }
            booting = true
        } else if (intent.getStringExtra("asyncboot") != "0") {
            try { Native.hardwarePictures(intent.getStringExtra("hwbuf") != "0") } catch (e: UnsatisfiedLinkError) { }
            Thread({
                device.tell(); Native.colors(colors); Native.window(size.first, size.second, density, edges)
                Native.startAsync(size.first, size.second, density, storage.absolutePath, live)
            }, "exact-load").apply { priority = Thread.MAX_PRIORITY }.start()
            booting = true
        } else { device.tell(); Native.colors(colors); Native.window(size.first, size.second, density, edges) }
        if (!driven) { driven = true; driver = drive { dy -> view.scrollBy(dy) } }
        view = ExactView(this, live) {
            if (!ready) {
                ready = true
                // `start=N` for an app whose rows have one pitch: scrolled there before the driver starts.
                val start = intent.getStringExtra("start")?.toIntOrNull() ?: 0
                if (start > 0 && startPitchDp > 0f) view.scrollBy(start * startPitchDp * resources.displayMetrics.density)
                driver?.contentReady()
            }
        }
        view.booting = booting
        view.window = size
        view.chrome = { c -> chrome(c) }
        view.asked = { r -> requests.perform(r) }
        view.owner = owner
        requests.view = view
        Exact.answered = { view.onPublished() }
        Exact.turned = { view.turned() }
        if (input) view.keys = Keys(view)
        val elements = Natives(this, File(storage, "app"), modules, video)
        natives = elements
        view.natives = elements
        // The app's text fields (LLP 1115.001 D8) outlive a reader made again: a field keeps its view, and with it
        // its selection and its undo, while its node lives. Under the view, which takes every touch and hands a
        // field its own ([Fields.touch]).
        val hosted = fields ?: Fields(this).also { fields = it }
        hosted.adopted(view)
        // A surface of the top layer that is open over this reader's page keeps its fields from the focus.
        hosted.covered(requests.surfaces?.open == true)
        view.fields = hosted
        // What the reader answers accessibility services with (D13): the root answers for the page, the bars and
        // the fields; the page's view and the layers are pictures and say nothing themselves. Hosted elements (a
        // web view, a video) report themselves, after exact2's nodes.
        view.importantForAccessibility = View.IMPORTANT_FOR_ACCESSIBILITY_NO
        view.access = { words -> accessTold(words) }
        root.access.fields = hosted
        root.access.others = { if (elements.childCount > 0) listOf<View>(elements) else emptyList() }
        root.addView(elements, FrameLayout.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.MATCH_PARENT))
        root.addView(hosted, FrameLayout.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.MATCH_PARENT))
        root.addView(view, FrameLayout.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.MATCH_PARENT))
        val ticker = if (intent.getStringExtra("tag") == "1") TagView(this) { view.tagState() } else View(this)
        elements.frame = { ticker.invalidate() }
        hosted.frame = { ticker.invalidate() }
        if (ticker is TagView) {
            val d = resources.displayMetrics.density
            root.addView(ticker, FrameLayout.LayoutParams((12 * TagView.BIT_DP * d).toInt(), (TagView.BIT_DP * d).toInt()).apply {
                leftMargin = (TagView.X_DP * d).toInt(); topMargin = (TagView.Y_DP * d).toInt()
            })
        } else root.addView(ticker, FrameLayout.LayoutParams(1, 1))
        view.ticker = ticker
        // The app's bars (LLP 1115.001 D11), over the page and everything it hosts. The host tells this reader its
        // bars anew, as it paints everything again for it.
        view.bars = Bars(this, root)
        accessible(accessibility.isEnabled)
    }

    private lateinit var root: Root

    /** The app is edge to edge (LLP 1115.001 D3.4): the window reaches under transparent system bars and into the
     * display's cutout, and the software keyboard is an inset the host is told, not a pan of the window. Android 15
     * does most of this to every app that targets it; the calls make it so from Android 12 on. */
    @Suppress("DEPRECATION")
    private fun edgeToEdge() {
        window.setDecorFitsSystemWindows(false)
        window.statusBarColor = Color.TRANSPARENT
        window.navigationBarColor = Color.TRANSPARENT
        window.isStatusBarContrastEnforced = false
        window.isNavigationBarContrastEnforced = false
        window.attributes = window.attributes.apply {
            layoutInDisplayCutoutMode = android.view.WindowManager.LayoutParams.LAYOUT_IN_DISPLAY_CUTOUT_MODE_ALWAYS
        }
        window.setSoftInputMode(android.view.WindowManager.LayoutParams.SOFT_INPUT_ADJUST_RESIZE)
    }

    /** The system's raw insets as the host takes them ([Native.window]): top, right, bottom, left for the system
     * bars, the display cutout, the software keyboard and the system's gestures, in pixels. */
    private fun edges(insets: android.view.WindowInsets): IntArray {
        val out = IntArray(16)
        val kinds = intArrayOf(
            android.view.WindowInsets.Type.systemBars(), android.view.WindowInsets.Type.displayCutout(),
            android.view.WindowInsets.Type.ime(), android.view.WindowInsets.Type.systemGestures(),
        )
        for ((k, kind) in kinds.withIndex()) {
            val i = insets.getInsets(kind)
            out[k * 4] = i.top; out[k * 4 + 1] = i.right; out[k * 4 + 2] = i.bottom; out[k * 4 + 3] = i.left
        }
        return out
    }

    /** The window's size, density and insets as the host was last told them. */
    private var toldWindow: Triple<Pair<Int, Int>, Float, List<Int>>? = null

    /** Tell the host the window, whole, when its size, the display's density or any inset is not what it was last
     * told (LLP 1115.001 D3.3). The host answers with where the page is ([chrome]). */
    private fun window(insets: android.view.WindowInsets) {
        if (!reads()) return
        val size = if (root.width > 0 && root.height > 0) root.width to root.height
            else windowManager.currentWindowMetrics.bounds.let { it.width() to it.height() }
        val density = resources.displayMetrics.density
        val edges = edges(insets)
        val now = Triple(size, density, edges.toList())
        if (now == toldWindow) return
        toldWindow = now
        val owner = view.owner
        if (owner != null) owner.window(size.first, size.second, density, edges)
        else { Native.window(size.first, size.second, density, edges); view.invalidate() }
    }

    /** What the host says to draw around the page ([Native.chrome]): the page's place in the window (x, y, width and
     * height in pixels), where the view and the hosted views under it are put; the colour behind the page and in the
     * edges it keeps out of, which is the window's background; and whether its scheme is dark, which the bars' icons
     * follow (light icons over a dark page). */
    private fun chrome(c: IntArray) {
        for (child in listOfNotNull<View>(view, natives ?: return, fields)) {
            child.layoutParams = FrameLayout.LayoutParams(c[2], c[3]).apply {
                gravity = android.view.Gravity.TOP or android.view.Gravity.LEFT
                leftMargin = c[0]; topMargin = c[1]
            }
        }
        window.setBackgroundDrawable(android.graphics.drawable.ColorDrawable(c[4]))
        val light = android.view.WindowInsetsController.APPEARANCE_LIGHT_STATUS_BARS or
            android.view.WindowInsetsController.APPEARANCE_LIGHT_NAVIGATION_BARS
        window.insetsController?.setSystemBarsAppearance(if (c[5] != 0) 0 else light, light)
    }

    /** The device's settings as the host was last told them. */
    private var told: Device? = null

    /** The device's colours as the host was last told them ([Palette]). */
    private var toldColors: IntArray? = null

    /** Read the device's settings and its colours and tell the host each, whole, when any of it is not what it was
     * last told (LLP 1115.001 D3.1, D4). */
    private fun configure() {
        if (!reads()) return
        val owner = view.owner
        val now = Device.read(this)
        if (now != told) {
            told = now
            if (owner != null) owner.configure(now) else { now.tell(); view.invalidate() }
        }
        val colors = Palette.read(this)
        if (!colors.contentEquals(toldColors)) {
            toldColors = colors
            if (owner != null) owner.colors(colors) else { Native.colors(colors); view.invalidate() }
        }
    }

    /** The two settings the app is told that are no part of a [android.content.res.Configuration]: "remove
     * animations" and the 12 or 24-hour clock. Watched while the activity is started; read again when it starts. */
    private val settings = object : android.database.ContentObserver(android.os.Handler(android.os.Looper.getMainLooper())) {
        override fun onChange(selfChange: Boolean) = configure()
    }

    /** A configuration changed in place (LLP 1115.001 D15.1): the manifest declares the size, night mode, font scale,
     * bold text, locale, density and keyboard changes, so none of them makes the activity again. The settings go to
     * the host whole ([configure]), and the window with its density ([window]); a new size or new insets arrive
     * through the root's own listeners as well. */
    override fun onConfigurationChanged(config: android.content.res.Configuration) {
        super.onConfigurationChanged(config)
        configure()
        root.rootWindowInsets?.let { window(it) }
    }

    /** The activity was stopped since its reader was made: Android discarded what that reader kept. */
    private var stopped = false

    /** The host does its frame work only while the activity is started (LLP 1115.001 D15.2), and a start after a
     * stop makes the reader again ([read]). */
    override fun onStart() {
        super.onStart()
        if (stopped) { stopped = false; read() }
        contentResolver.registerContentObserver(android.provider.Settings.Global.getUriFor(android.provider.Settings.Global.ANIMATOR_DURATION_SCALE), false, settings)
        contentResolver.registerContentObserver(android.provider.Settings.System.getUriFor(android.provider.Settings.System.TIME_12_24), false, settings)
        // What changed while the activity was stopped and is no configuration change.
        configure()
        view.running(true)
    }

    override fun onStop() {
        stopped = true
        view.running(false, reads())
        contentResolver.unregisterContentObserver(settings)
        super.onStop()
    }

    /** In front again: a share sheet that was up and chose nothing was dismissed ([Requests.resumed]). */
    override fun onResume() {
        super.onResume()
        requests.resumed()
    }

    override fun onDestroy() {
        accessibility.removeAccessibilityStateChangeListener(accessWatch)
        requests.destroy()
        release()
        super.onDestroy()
    }

    /** Whether this activity's view is the one that reads the host. An activity that was replaced before it stopped
     * (Android starts its successor first) is not, and must tell the host nothing: its window, its settings and its
     * stop are no longer the app's. */
    private fun reads() = Exact.reader?.get() === this

    /** This activity's reader lets go of the host: its hosted views and their surfaces go, and the owner stops
     * telling its view. Once: at [onDestroy], or sooner when the activity that replaces it is made first. */
    private fun release() {
        requests.surfaces?.destroy()
        natives?.destroyAll()
        natives = null
        fields?.leaving()
        if (Fields.context === this) Fields.context = null
        if (!reads()) return
        Exact.reader = null
        Exact.owner?.let { it.published = {}; it.asked = {}; it.chrome = {} }
        Exact.answered = {}
        Exact.turned = {}
    }

    /** The launch extras that reach exact2 as its environment, written before its library loads. */
    private fun environment() {
        // rows=0: no kept rows, every frame walks every row (exact2's EXACT_ROWS), to compare.
        if (intent.getStringExtra("rows") == "0") android.system.Os.setenv("EXACT_ROWS", "0", true)
        // imgvp=N: keep N viewports of decoded pictures (exact2's EXACT_IMAGE_VIEWPORTS; 12 by default).
        // moves=N: frames that may move the last paint before one paints (exact2's EXACT_MOVES).
        intent.getStringExtra("moves")?.let { android.system.Os.setenv("EXACT_MOVES", it, true) }
        // idle=N: paints a picture goes undrawn before its Android copy is freed (exact2's EXACT_IDLE_FRAMES).
        intent.getStringExtra("idle")?.let { android.system.Os.setenv("EXACT_IDLE_FRAMES", it, true) }
        intent.getStringExtra("slots")?.let { android.system.Os.setenv("EXACT_SLOTS", it, true) }
        // lower=0: the engine samples every animation (no reader-played layers; exact2's EXACT_LOWER), to compare.
        intent.getStringExtra("lower")?.let { android.system.Os.setenv("EXACT_LOWER", it, true) }
        // malloc=system: Android's allocator for exact2's allocations, not mimalloc (read at the library's first allocation).
        intent.getStringExtra("malloc")?.let { android.system.Os.setenv("EXACT_MALLOC", it, true) }
        // mi_<option>=v: mimalloc's MIMALLOC_<OPTION> (read at its first allocation), to tune it.
        intent.extras?.keySet()?.filter { it.startsWith("mi_") }?.forEach { k ->
            intent.getStringExtra(k)?.let { android.system.Os.setenv("MIMALLOC_" + k.removePrefix("mi_").uppercase(), it, true) }
        }
        // x_<name>=v: exact2's EXACT_<NAME>, any of its switches, to compare.
        intent.extras?.keySet()?.filter { it.startsWith("x_") }?.forEach { k ->
            intent.getStringExtra(k)?.let { android.system.Os.setenv("EXACT_" + k.removePrefix("x_").uppercase(), it, true) }
        }
        // A move may carry layers' tracks ([applyMove]), so a row coming into view starts its animations without a
        // paint, and exact2 paints only by the frame new rows can show (movetracks=0: a paint within six moves).
        if (intent.getStringExtra("movetracks") != "0") android.system.Os.setenv("EXACT_MOVE_TRACKS", "1", true)
        // reuse=0: list rows built anew, not rebound (exact2's EXACT_ROW_REUSE, LLP 1078), to compare.
        intent.getStringExtra("reuse")?.let { android.system.Os.setenv("EXACT_ROW_REUSE", it, true) }
        Shadows.enabled = intent.getStringExtra("shadowmask") != "0"
        Exact.detachWait = intent.getStringExtra("detachwait") == "1"
        intent.getStringExtra("imgvp")?.let { android.system.Os.setenv("EXACT_IMAGE_VIEWPORTS", it, true) }
        // GPU modules (LLP 1009) load from the APK's extracted libraries, not beside the zygote's binary.
        android.system.Os.setenv("EXACT_NATIVE_LIBS", applicationInfo.nativeLibraryDir, true)
        intent.getStringExtra("poly")?.let { Polylines.on = it != "0" }
        intent.getStringExtra("mesh")?.let { Polylines.mesh = it != "0" }
        intent.getStringExtra("icons")?.let { Icons.on = it != "0" }
        intent.getStringExtra("scenario")?.let { android.system.Os.setenv("BENCH_SCENARIO", it, true) }
        android.system.Os.setenv("BENCH_FREEZE", if (intent.getStringExtra("freeze") == "1") "1" else "0", true)
        intent.getStringExtra("start")?.let { android.system.Os.setenv("BENCH_START_INDEX", it, true) }
        // kinds=17 (or a kind list): BENCH_KINDS, for an app whose source filters by it (as the Views app's `kinds`).
        intent.getStringExtra("kinds")?.let { android.system.Os.setenv("BENCH_KINDS", it, true) }
    }

    /** The APK's `app/` assets (the plan names `assets/…` under it), copied once per install: exact2 reads files. */
    private fun installAssets(to: File) {
        val stamp = packageManager.getPackageInfo(packageName, 0).lastUpdateTime.toString()
        val done = File(to, ".done")
        if (done.exists() && done.readText() == stamp) return
        fun copy(from: String, dir: File) {
            val names = assets.list(from) ?: return
            if (names.isEmpty()) {
                dir.parentFile?.mkdirs()
                assets.open(from).use { i -> dir.outputStream().use { o -> i.copyTo(o) } }
                return
            }
            dir.mkdirs()
            for (n in names) copy("$from/$n", File(dir, n))
        }
        if (assets.list("app").isNullOrEmpty()) return
        copy("app", to)
        done.writeText(stamp)
    }
}
