// @exact/local-network on Android (LLP 1133.036 D1-D4, D8, D13): `NsdManager`'s discovery and resolution. Under the
// agent the runner's declared substitute answers and nothing here runs.
//
// - One discovery per type, shared by the session's readers and stopped when the last releases it; a resolve per
//   (name, type) by `registerServiceInfoCallback` from API 34, which answers again on each change, and below it by
//   `resolveService`, which runs one at a time, so resolves queue.
// - A browse answers once its first service is found or a second after it started, then each change; instances are
//   merged across networks and ordered by name, case-insensitively (D2). Both pause in the background.
// - `host` is an address the platform resolved, IPv4 first, an IPv6 one in brackets otherwise (D4).
// - Before T extensions 7 the system does not manage multicast reception, so a browse holds a `MulticastLock`
//   (`CHANGE_WIFI_MULTICAST_STATE`, which `net.dns-sd` derives).
// - The host targets API 36, where an app holding `INTERNET` has the local network implicitly: the state is
//   `granted` and `android` is none. `ACCESS_LOCAL_NETWORK` and `android.chooseService` wait for API 37 (§15 Q6).
// - Tier 3 (D13): `LocalNetworkModule.nsdManager` is given the manager once, when the module is made.
package exact.localnetwork

import android.content.Context
import android.net.nsd.NsdManager
import android.net.nsd.NsdServiceInfo
import android.net.wifi.WifiManager
import android.os.Build
import android.os.Handler
import android.os.Looper
import android.os.ext.SdkExtensions
import dev.exact.canvas.ExactExtension
import dev.exact.canvas.ExtensionContext
import java.net.Inet4Address
import java.net.Inet6Address
import java.net.InetAddress
import java.util.concurrent.Executor
import org.json.JSONArray
import org.json.JSONObject

class LocalNetworkModule(context: ExtensionContext) : ExactExtension(context) {
    companion object {
        /** The app's access hatch (D13): given the manager once, when the module is made. */
        @JvmStatic var nsdManager: ((NsdManager) -> Unit)? = null
    }

    private val manager = (context.application.getSystemService(Context.NSD_SERVICE) as NsdManager).also { nsdManager?.invoke(it) }
    private val main = Handler(Looper.getMainLooper())
    private val onMain = Executor { main.post(it) }
    private val browses = HashMap<String, Browse>()
    private val resolves = HashMap<String, Resolve>()
    private var shown = true

    private fun error(code: String, message: String? = null) = JSONObject().put("error", code).also { if (message != null) it.put("message", message) }

    override fun later(name: String, args: JSONObject, reply: (JSONObject) -> Unit) {
        main.post {
            if (context.agent) return@post reply(error("NotSupportedError"))
            when (name) {
                "localNetworkStatus" -> reply(JSONObject().put("available", true).put("state", "granted").put("android", JSONObject.NULL))
                "browseServices" -> browses.getOrPut(args.optString("type")) { Browse(args.optString("type")) }.read(reply)
                "resolveService" -> {
                    val key = "${args.optString("type")} ${args.optString("name")}"
                    resolves.getOrPut(key) { Resolve(args.optString("name"), args.optString("type")) }.read(args.optDouble("timeout", 5000.0), reply)
                }
                else -> reply(error("NotSupportedError"))
            }
        }
    }

    override fun release(name: String) = main.post { leave { it.startsWith(name) } }.let {}
    override fun release(name: String, args: JSONObject) = main.post {
        leave { it == if (name == "browseServices") "browseServices ${args.optString("type")}" else "resolveService ${args.optString("type")} ${args.optString("name")}" }
    }.let {}

    private fun leave(wanted: (String) -> Boolean) {
        browses.entries.removeAll { (type, b) -> wanted("browseServices $type").also { if (it) b.stop() } }
        resolves.entries.removeAll { (key, r) -> wanted("resolveService $key").also { if (it) r.stop() } }
    }

    override fun foreground(shown: Boolean) {
        main.post {
            if (shown == this.shown) return@post
            this.shown = shown
            browses.values.forEach { it.pause(!shown) }
            resolves.values.forEach { it.pause(!shown) }
        }
    }

    override fun destroy(done: () -> Unit) {
        main.post { leave { true }; done() }
    }

    /** Whether the system manages multicast reception (T extensions 7), so no lock is needed. */
    private fun managed(): Boolean =
        Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU && SdkExtensions.getExtensionVersion(Build.VERSION_CODES.TIRAMISU) >= 7

    private inner class Browse(val type: String) {
        private val names = HashMap<String, Int>() // instance name -> networks reporting it
        private var settled = false
        private var failure: JSONObject? = null
        private val waiting = ArrayList<(JSONObject) -> Unit>()
        private var listener: NsdManager.DiscoveryListener? = null
        private var lock: WifiManager.MulticastLock? = null

        fun read(reply: (JSONObject) -> Unit) {
            failure?.let { return reply(it) }
            if (settled) return reply(answer())
            waiting.add(reply)
            start()
        }

        private fun answer(): JSONObject {
            val sorted = names.keys.sortedWith(String.CASE_INSENSITIVE_ORDER)
            return JSONObject().put("services", JSONArray(sorted.map { JSONObject().put("name", it).put("type", type) }))
        }

        private fun start() {
            if (listener != null || !shown) return
            if (!managed()) {
                val wifi = context.application.applicationContext.getSystemService(Context.WIFI_SERVICE) as WifiManager
                lock = wifi.createMulticastLock("exact.local-network").apply { setReferenceCounted(false); acquire() }
            }
            val l = object : NsdManager.DiscoveryListener {
                override fun onDiscoveryStarted(serviceType: String) {}
                override fun onDiscoveryStopped(serviceType: String) {}
                override fun onStartDiscoveryFailed(serviceType: String, code: Int) = main.post { fail(error("NetworkError", "discovery failed: $code")) }.let {}
                override fun onStopDiscoveryFailed(serviceType: String, code: Int) {}
                override fun onServiceFound(info: NsdServiceInfo) = main.post { moved(info.serviceName, +1) }.let {}
                override fun onServiceLost(info: NsdServiceInfo) = main.post { moved(info.serviceName, -1) }.let {}
            }
            listener = l
            manager.discoverServices(type, NsdManager.PROTOCOL_DNS_SD, l)
            main.postDelayed({ if (listener === l) settle() }, 1000)
        }

        private fun moved(name: String, by: Int) {
            val n = (names[name] ?: 0) + by
            if (n > 0) names[name] = n else names.remove(name)
            if (settled) context.changed("local-network.browseServices") else settle()
        }

        private fun settle() {
            settled = true
            val replies = ArrayList(waiting)
            waiting.clear()
            replies.forEach { it(answer()) }
        }

        private fun fail(e: JSONObject) {
            failure = e
            stop()
            val replies = ArrayList(waiting)
            waiting.clear()
            replies.forEach { it(e) }
            context.changed("local-network.browseServices")
        }

        fun pause(hidden: Boolean) {
            if (hidden) stop() else { settled = false; failure = null; names.clear(); context.changed("local-network.browseServices") }
        }

        fun stop() {
            listener?.let { runCatching { manager.stopServiceDiscovery(it) } }
            listener = null
            lock?.let { if (it.isHeld) it.release() }
            lock = null
        }
    }

    private inner class Resolve(val name: String, val type: String) {
        private var info: JSONObject? = null
        private val waiting = ArrayList<(JSONObject) -> Unit>()
        private var callback: NsdManager.ServiceInfoCallback? = null
        private var asked = false

        fun read(timeout: Double, reply: (JSONObject) -> Unit) {
            info?.let { return reply(it) }
            waiting.add(reply)
            start()
            main.postDelayed({ if (info == null) finish(error("NotFoundError")) }, timeout.toLong())
        }

        private fun start() {
            if (asked || !shown) return
            asked = true
            val wanted = NsdServiceInfo().apply { serviceName = name; serviceType = type }
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.UPSIDE_DOWN_CAKE) {
                val c = object : NsdManager.ServiceInfoCallback {
                    override fun onServiceInfoCallbackRegistrationFailed(code: Int) = main.post { finish(error("NetworkError", "resolve failed: $code")) }.let {}
                    override fun onServiceUpdated(found: NsdServiceInfo) = main.post { took(found, found.hostAddresses) }.let {}
                    override fun onServiceLost() = main.post { info = null; finish(error("NotFoundError")); context.changed("local-network.resolveService") }.let {}
                    override fun onServiceInfoCallbackUnregistered() {}
                }
                callback = c
                manager.registerServiceInfoCallback(wanted, onMain, c)
            } else Serial.ask(manager, wanted) { found, code -> main.post { if (found != null) @Suppress("DEPRECATION") took(found, listOfNotNull(found.host)) else finish(error("NotFoundError", "resolve failed: $code")) } }
        }

        private fun took(found: NsdServiceInfo, addresses: List<InetAddress>) {
            // A link-local IPv6 address carries a zone no URL can (D2).
            val usable = addresses.filterNot { it is Inet6Address && it.isLinkLocalAddress }
            if (usable.isEmpty()) return
            val v4 = usable.firstOrNull { it is Inet4Address }
            val host = v4?.hostAddress ?: "[${usable.first().hostAddress}]"
            val txt = found.attributes.entries
                .map { (k, v) -> k.lowercase() to v }
                .distinctBy { it.first }
                .sortedBy { it.first }
                .map { (k, v) -> JSONObject().put("key", k).put("value", v?.let { String(it, Charsets.UTF_8) } ?: JSONObject.NULL) }
            val hostName = if (Build.VERSION.SDK_INT >= 36) found.hostname else null
            val fresh = JSONObject().put("name", name).put("type", type).put("host", host).put("hostName", hostName ?: JSONObject.NULL)
                .put("port", found.port).put("addresses", JSONArray(usable.map { it.hostAddress })).put("txt", JSONArray(txt))
            val changed = info != null && info.toString() != fresh.toString()
            info = fresh
            if (waiting.isNotEmpty()) finish(fresh) else if (changed) context.changed("local-network.resolveService")
        }

        private fun finish(answer: JSONObject) {
            val replies = ArrayList(waiting)
            waiting.clear()
            replies.forEach { it(answer) }
            if (answer.has("error")) stop()
        }

        fun pause(hidden: Boolean) {
            if (hidden) stop() else { info = null; context.changed("local-network.resolveService") }
        }

        fun stop() {
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.UPSIDE_DOWN_CAKE) callback?.let { runCatching { manager.unregisterServiceInfoCallback(it) } }
            callback = null
            asked = false
        }
    }

    /** `resolveService` below API 34 handles one resolve at a time; a second fails `FAILURE_ALREADY_ACTIVE`. */
    private object Serial {
        private val queue = ArrayDeque<Pair<NsdServiceInfo, (NsdServiceInfo?, Int) -> Unit>>()
        private var busy = false
        private lateinit var nsd: NsdManager

        fun ask(manager: NsdManager, info: NsdServiceInfo, done: (NsdServiceInfo?, Int) -> Unit) {
            nsd = manager
            synchronized(this) { queue.addLast(info to done) }
            next()
        }

        private fun next() {
            val (info, done) = synchronized(this) {
                if (busy || queue.isEmpty()) return
                busy = true
                queue.removeFirst()
            }
            @Suppress("DEPRECATION")
            nsd.resolveService(info, object : NsdManager.ResolveListener {
                override fun onResolveFailed(info: NsdServiceInfo, code: Int) = finished { done(null, code) }
                override fun onServiceResolved(info: NsdServiceInfo) = finished { done(info, 0) }
            })
        }

        private fun finished(then: () -> Unit) {
            synchronized(this) { busy = false }
            then()
            next()
        }
    }
}
