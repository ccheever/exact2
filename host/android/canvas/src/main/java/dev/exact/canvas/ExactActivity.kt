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
    /** The row pitch `start=N` scrolls by (0: the app's plan scrolls to it itself). */
    protected open val startPitchDp = 0f
    /** The app has text fields: keys and the software keyboard reach it ([Keys]), and system Back presses its
     * `id="back"` control (the app's library exports Native.key, editing and back). */
    protected open val input = false
    /** The view stops above the navigation bar (the content area), not only below the status bar. */
    protected open val insetBottom = false
    /**
     * The seam a driver attaches at ([Drive]); the default attaches none. Called once, in [onCreate], when the host
     * has begun to boot and before the view is made: a driver sets the window up and reads its own launch extras
     * here. [scroll] moves the app's feed by a distance in pixels ([ExactView.scrollBy]) and may be called once
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
        // owner=0: exact2 on the main thread (booting on its own thread first unless asyncboot=0). Otherwise exact2
        // lives on a thread of its own from boot on ([Owner]), started here so it loads exact2's library while the
        // assets are checked.
        val owned = intent.getStringExtra("owner") != "0"
        val owner: Owner? = if (owned) {
            // exactprio=display: exact2's thread above the main thread (THREAD_PRIORITY_DISPLAY), to compare.
            val priority = if (intent.getStringExtra("exactprio") == "display") android.os.Process.THREAD_PRIORITY_DISPLAY
                else android.os.Process.THREAD_PRIORITY_DEFAULT
            // The process's one owner: its thread holds the host, which an activity made again adopts.
            (Exact.owner ?: Owner(intent.getStringExtra("hwbuf") != "0", intent.getStringExtra("fast") != "0", priority, intent::getStringExtra).also { Exact.owner = it })
                .also { it.published = { view.onPublished() } }
        } else null
        val storage = createDeviceProtectedStorageContext().filesDir
        installAssets(File(storage, "app"))
        // asyncboot=0: boot when the view is laid out, on the main thread, to compare. Otherwise now, at the size the
        // view will have (the window less the bars this activity pads by), on a thread that also loads the library:
        // exact2 loads, boots and paints its first frame while Android sets the activity up, and the view takes it
        // when it is laid out.
        var booting: Pair<Int, Int>? = null
        if (owner != null) {
            val wm = windowManager.currentWindowMetrics
            val bars = wm.windowInsets.getInsets(android.view.WindowInsets.Type.systemBars())
            val size = wm.bounds.width() to (wm.bounds.height() - bars.top - if (insetBottom) bars.bottom else 0)
            val live = intent.getStringExtra("live").let { it == "1" || it == "true" }
            owner.boot(size.first, size.second, resources.displayMetrics.density, storage.absolutePath, live) { ok -> view.ownerStarted(ok) }
            booting = size
        } else if (intent.getStringExtra("asyncboot") != "0") {
            val wm = windowManager.currentWindowMetrics
            val bars = wm.windowInsets.getInsets(android.view.WindowInsets.Type.systemBars())
            val size = wm.bounds.width() to (wm.bounds.height() - bars.top - if (insetBottom) bars.bottom else 0)
            val live = intent.getStringExtra("live").let { it == "1" || it == "true" }
            val density = resources.displayMetrics.density
            try { Native.hardwarePictures(intent.getStringExtra("hwbuf") != "0") } catch (e: UnsatisfiedLinkError) { }
            Thread({ Native.startAsync(size.first, size.second, density, storage.absolutePath, live) }, "exact-load")
                .apply { priority = Thread.MAX_PRIORITY }.start()
            booting = size
        }
        driver = drive { dy -> view.scrollBy(dy) }
        view = ExactView(this, intent.getStringExtra("live").let { it == "1" || it == "true" }) {
            run {
                // `start=N` for an app whose rows have one pitch: scrolled there before the driver starts.
                val start = intent.getStringExtra("start")?.toIntOrNull() ?: 0
                if (start > 0 && startPitchDp > 0f) view.scrollBy(start * startPitchDp * resources.displayMetrics.density)
                driver?.contentReady()
            }
        }
        view.booting = booting
        view.owner = owner
        owner?.editingChanged = { view.keys?.focusChanged() }
        Exact.answered = { view.onPublished() }
        if (input) {
            view.keys = Keys(view)
            onBackInvokedDispatcher.registerOnBackInvokedCallback(android.window.OnBackInvokedDispatcher.PRIORITY_DEFAULT) {
                if (Exact.back()) view.invalidate() else finish()
            }
        }
        val root = FrameLayout(this).apply { setBackgroundColor(Color.WHITE) }
        val elements = Natives(this, File(storage, "app"), modules, video)
        natives = elements
        view.natives = elements
        root.addView(elements, FrameLayout.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.MATCH_PARENT))
        root.addView(view, FrameLayout.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.MATCH_PARENT))
        val ticker = if (intent.getStringExtra("tag") == "1") TagView(this) { view.tagState() } else View(this)
        elements.frame = { ticker.invalidate() }
        if (ticker is TagView) {
            val d = resources.displayMetrics.density
            root.addView(ticker, FrameLayout.LayoutParams((12 * TagView.BIT_DP * d).toInt(), (TagView.BIT_DP * d).toInt()).apply {
                leftMargin = (TagView.X_DP * d).toInt(); topMargin = (TagView.Y_DP * d).toInt()
            })
        } else root.addView(ticker, FrameLayout.LayoutParams(1, 1))
        view.ticker = ticker
        root.setOnApplyWindowInsetsListener { v, insets ->
            val bars = insets.getInsets(android.view.WindowInsets.Type.systemBars())
            v.setPadding(0, bars.top, 0, if (insetBottom) bars.bottom else 0); insets
        }
        setContentView(root)
    }

    /** The display's density changed in place (LLP 1115.001 D15.1): the manifest declares it, with the size, night
     * mode, font scale, locale and keyboard changes, so none of them makes the activity again. A size arrives through
     * the view's own [ExactView.onSizeChanged]. */
    override fun onConfigurationChanged(config: android.content.res.Configuration) {
        super.onConfigurationChanged(config)
        view.densityChanged()
    }

    /** The host does its frame work only while the activity is started (LLP 1115.001 D15.2). */
    override fun onStart() {
        super.onStart()
        view.running(true)
    }

    override fun onStop() {
        view.running(false)
        super.onStop()
    }

    override fun onDestroy() {
        release()
        super.onDestroy()
    }

    /** This activity's reader lets go of the host: its hosted views and their surfaces go, and the owner stops
     * telling its view. Once: at [onDestroy], or sooner when the activity that replaces it is made first. */
    private fun release() {
        natives?.destroyAll()
        natives = null
        if (Exact.reader?.get() !== this) return
        Exact.reader = null
        Exact.owner?.let { it.published = {}; it.editingChanged = {} }
        Exact.answered = {}
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
