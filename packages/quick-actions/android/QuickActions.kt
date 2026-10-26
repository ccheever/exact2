// @exact/quick-actions on Android (LLP 1133.023 D3, D11, D17): the app's list as the launcher activity's dynamic
// shortcuts (ShortcutManager.setDynamicShortcuts), after the manifest's static ones (`shortcuts.xml`, which the APK's
// build writes), and a pin request through the launcher's own dialog. Each shortcut starts the launcher activity with
// its location as the intent's data, which the reader takes as a link's (D15 item 3).
//
// `setDynamicShortcuts` answers false when the call is rate-limited
// (https://developer.android.com/reference/android/content/pm/ShortcutManager), so the list is kept and set again the
// next time the app comes to the front.
package exact.quickactions

import android.content.Intent
import android.content.pm.ShortcutInfo
import android.content.pm.ShortcutManager
import android.net.Uri
import dev.exact.canvas.ExactExtension
import dev.exact.canvas.ExtensionContext
import org.json.JSONArray
import org.json.JSONObject

class QuickActionsModule(context: ExtensionContext) : ExactExtension(context) {
    private val app get() = context.application
    private val manager get() = app.getSystemService(ShortcutManager::class.java)
    /** A list a rate limit refused, set again at the next foreground. */
    private var pending: List<ShortcutInfo>? = null

    override fun later(name: String, args: JSONObject, reply: (JSONObject) -> Unit) {
        when (name) {
            "setAppShortcuts" -> {
                val items = args.optJSONArray("shortcuts") ?: JSONArray()
                val infos = (0 until items.length()).map { i -> info("app-$i", items.optJSONObject(i) ?: JSONObject(), i) }
                if (infos.any { it == null }) return reply(JSONObject().put("error", "TypeError"))
                // Static and dynamic shortcuts share the activity's count.
                if (infos.size > manager.maxShortcutCountPerActivity - manager.manifestShortcuts.size)
                    return reply(JSONObject().put("error", "QuotaExceededError"))
                set(infos.filterNotNull())
                reply(JSONObject())
            }
            "clearAppShortcuts" -> {
                pending = null
                manager.removeAllDynamicShortcuts()
                reply(JSONObject())
            }
            "requestPinShortcut" -> {
                val shortcut = args.optJSONObject("shortcut") ?: JSONObject()
                val info = info("pin-" + shortcut.optString("url"), shortcut, 0) ?: return reply(JSONObject().put("error", "TypeError"))
                val requested = manager.isRequestPinShortcutSupported && manager.requestPinShortcut(info, null)
                reply(JSONObject().put("state", if (requested) "requested" else "unsupported"))
            }
            else -> reply(JSONObject().put("error", "NotSupportedError"))
        }
    }

    override fun resumed() {
        pending?.let { set(it) }
    }

    private fun set(infos: List<ShortcutInfo>) {
        pending = if (manager.setDynamicShortcuts(infos)) null else infos
    }

    /** A shortcut item as the launcher's: its name and short name, its rank, and an intent opening its location. A
     * location that is not an app path, or a missing name, is none (D3). */
    private fun info(id: String, item: JSONObject, index: Int): ShortcutInfo? {
        val name = item.optString("name")
        val url = item.optString("url")
        if (name.isEmpty() || !url.startsWith("/") || url.startsWith("//")) return null
        val launcher = app.packageManager.getLaunchIntentForPackage(app.packageName)?.component ?: return null
        val intent = Intent(Intent.ACTION_VIEW, Uri.parse(url)).setComponent(launcher)
        val rank = item.optJSONObject("android")?.optDouble("rank")?.takeIf { !it.isNaN() }?.toInt() ?: index
        return ShortcutInfo.Builder(app, id)
            .setShortLabel(item.optString("short_name").ifEmpty { name })
            .setLongLabel(name)
            .setRank(rank)
            .setIntent(intent)
            .build()
    }
}
