// @exact/in-app-browser on Android (LLP 1133.024 D11–D13): a Custom Tab from androidx.browser, over the app when a
// browser provides Custom Tabs, else the default browser.
//
// - The embedder decides first ([ExtensionContext.leave], D22 item 7): a refusal answers `NotAllowedError`.
// - It answers `opened` at launch: the platform reports a tab's navigation and hiding only to a bound
//   `CustomTabsSession`, which the module does not keep (§18).
// - `overApp` is whether a Custom Tabs provider resolves, read again at each resume (D11); the package's `queries`
//   entry makes the providers visible under Android 11's package visibility.
// - Under the agent nothing opens: the `browser` ticket's tap is the answer. The journal names the URL's origin.
// - Tier 3 (D13): [InAppBrowserModule.customTabsBuilder] is handed the builder after the module's words are set and
//   before `build()`; the launch stays the module's.
package exact.inappbrowser

import android.net.Uri
import androidx.browser.customtabs.CustomTabsClient
import androidx.browser.customtabs.CustomTabsIntent
import dev.exact.canvas.ExactExtension
import dev.exact.canvas.ExtensionContext
import org.json.JSONObject

class InAppBrowserModule(context: ExtensionContext) : ExactExtension(context) {
    companion object {
        /** The app's hatch (D13): each tab's builder, after the module's words are set. */
        @JvmStatic var customTabsBuilder: ((CustomTabsIntent.Builder) -> Unit)? = null
    }

    private var said: Boolean? = null

    private fun say(line: String) = context.log("in-app-browser: $line")

    /** Whether a browser on this device provides Custom Tabs. */
    private fun provider(): Boolean = CustomTabsClient.getPackageName(context.application, emptyList()) != null

    private fun status(): JSONObject {
        val over = if (context.agent) true else provider()
        said = over
        return JSONObject().put("available", true).put("overApp", over)
    }

    override fun later(name: String, args: JSONObject, reply: (JSONObject) -> Unit) = later(name, args, null, reply)

    override fun later(name: String, args: JSONObject, preferred: Any?, reply: (JSONObject) -> Unit) {
        when (name) {
            "inAppBrowserStatus" -> reply(status())
            "openBrowser" -> open(args, (preferred as? JSONObject)?.optString("tap")?.takeIf { it.isNotEmpty() }, reply)
            else -> reply(JSONObject().put("error", "NotSupportedError"))
        }
    }

    /** A browser installed or removed while the app was away changes `overApp`. */
    override fun resumed() {
        val before = said ?: return
        if (!context.agent && provider() != before) context.changed("in-app-browser.inAppBrowserStatus")
    }

    private fun open(args: JSONObject, tap: String?, reply: (JSONObject) -> Unit) {
        val uri = args.optString("url").let { runCatching { Uri.parse(it) }.getOrNull() }
        if (uri == null || uri.scheme?.lowercase() !in setOf("http", "https") || uri.host.isNullOrEmpty()) {
            return reply(JSONObject().put("error", "TypeError").put("message", "openBrowser takes an absolute http or https URL; open any other with openURL"))
        }
        val origin = "${uri.scheme}://${uri.authority}"
        if (context.agent) {
            val action = tap ?: "opened"
            say("$action $origin")
            return reply(JSONObject().put("action", action))
        }
        if (!context.leave(uri.toString(), "browser")) {
            say("the embedder kept the app from opening $origin")
            return reply(JSONObject().put("error", "NotAllowedError").put("message", "the embedder declined to leave the app for this page"))
        }
        val activity = context.activity
            ?: return reply(JSONObject().put("error", "InvalidStateError").put("message", "no activity to open the page from"))
        val words = args.optJSONObject("android") ?: JSONObject()
        val builder = CustomTabsIntent.Builder()
        if (words.has("showTitle")) builder.setShowTitle(words.optBoolean("showTitle"))
        if (words.has("urlBarHidingEnabled")) builder.setUrlBarHidingEnabled(words.optBoolean("urlBarHidingEnabled"))
        when (words.optString("shareState")) {
            "on" -> builder.setShareState(CustomTabsIntent.SHARE_STATE_ON)
            "off" -> builder.setShareState(CustomTabsIntent.SHARE_STATE_OFF)
            "default" -> builder.setShareState(CustomTabsIntent.SHARE_STATE_DEFAULT)
        }
        when (words.optString("colorScheme")) {
            "light" -> builder.setColorScheme(CustomTabsIntent.COLOR_SCHEME_LIGHT)
            "dark" -> builder.setColorScheme(CustomTabsIntent.COLOR_SCHEME_DARK)
            "system" -> builder.setColorScheme(CustomTabsIntent.COLOR_SCHEME_SYSTEM)
        }
        if (words.has("initialActivityHeightPx")) builder.setInitialActivityHeightPx(words.optInt("initialActivityHeightPx"))
        when (words.optString("closeButtonPosition")) {
            "start" -> builder.setCloseButtonPosition(CustomTabsIntent.CLOSE_BUTTON_POSITION_START)
            "end" -> builder.setCloseButtonPosition(CustomTabsIntent.CLOSE_BUTTON_POSITION_END)
        }
        customTabsBuilder?.invoke(builder)
        try {
            builder.build().launchUrl(activity, uri)
        } catch (e: android.content.ActivityNotFoundException) {
            return reply(JSONObject().put("error", "NotSupportedError").put("message", "no browser opens this page"))
        }
        say("opened $origin")
        reply(JSONObject().put("action", "opened"))
    }
}
