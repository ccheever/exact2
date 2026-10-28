// @exact/compose on Android (LLP 1133.024 D15): the mail and message apps' composers, by intent.
//
// - A mail with no attachment is `ACTION_SENDTO` on a `mailto:` URI, which only mail apps handle; with attachments it is
//   `ACTION_SEND` or `ACTION_SEND_MULTIPLE` with each file's content URI from the host's provider
//   ([ExtensionContext.contentUri]) under a read grant, and a `mailto:` selector, so only mail apps answer. The fields
//   are `EXTRA_EMAIL`, `EXTRA_CC`, `EXTRA_BCC`, `EXTRA_SUBJECT` and `EXTRA_TEXT`.
// - A message is `ACTION_SENDTO` on `smsto:` with `sms_body`; with attachments `ACTION_SEND` with the streams,
//   `sms_body` and `subject`, addressed to the default SMS app.
// - Mail and message apps return no result, so each answers `opened`. The fact is whether an app resolves
//   (`SENDTO mailto:` and `smsto:`, which the package's `queries` make visible), read again at each resume.
// - A hand-off leaves the app, so the embedder decides first ([ExtensionContext.leave], D22 item 7).
// - Under the agent nothing opens: the `compose` ticket's tap is the answer. The journal names the composer, the counts
//   and the attachments' types, never an address, the subject or the body.
package exact.compose

import android.content.ActivityNotFoundException
import android.content.ClipData
import android.content.Intent
import android.net.Uri
import android.provider.Telephony
import dev.exact.canvas.ExactExtension
import dev.exact.canvas.ExtensionContext
import org.json.JSONArray
import org.json.JSONObject

class ComposeModule(context: ExtensionContext) : ExactExtension(context) {
    private var said: String? = null

    private fun say(line: String) = context.log("compose: $line")

    private fun resolves(intent: Intent) = intent.resolveActivity(context.application.packageManager) != null
    private val mail get() = resolves(Intent(Intent.ACTION_SENDTO, Uri.parse("mailto:")))
    private val sms get() = resolves(Intent(Intent.ACTION_SENDTO, Uri.parse("smsto:")))

    private fun status(): JSONObject {
        val (email, message) = if (context.agent) true to true else mail to sms
        return JSONObject().put("available", true).put("email", email).put("emailAttachments", email).put("emailCopiesWithAttachments", email)
            .put("message", message).put("messageSubject", message).put("messageAttachments", message).also { said = it.toString() }
    }

    override fun later(name: String, args: JSONObject, reply: (JSONObject) -> Unit) = later(name, args, null, reply)

    override fun later(name: String, args: JSONObject, preferred: Any?, reply: (JSONObject) -> Unit) {
        val tap = (preferred as? JSONObject)?.optString("tap")?.takeIf { it.isNotEmpty() }
        when (name) {
            "composeStatus" -> reply(status())
            "composeEmail" -> compose("email", args, tap, reply)
            "composeMessage" -> compose("message", args, tap, reply)
            else -> reply(JSONObject().put("error", "NotSupportedError"))
        }
    }

    /** A mail app installed or removed while the app was away changes the fact. */
    override fun resumed() {
        val before = said ?: return
        if (!context.agent && status().toString() != before) context.changed("compose.composeStatus")
    }

    private fun list(args: JSONObject, key: String): List<String> =
        (args.optJSONArray(key) ?: JSONArray()).let { a -> (0 until a.length()).map { a.optString(it) }.filter { it.isNotEmpty() } }

    private fun error(name: String, message: String) = JSONObject().put("error", name).put("message", message)

    private fun compose(kind: String, args: JSONObject, tap: String?, reply: (JSONObject) -> Unit) {
        val (to, cc, bcc) = Triple(list(args, "to"), list(args, "cc"), list(args, "bcc"))
        val subject = args.optString("subject")
        val body = args.optString("body")
        val paths = list(args, "attachments")
        val files = paths.map { context.file(it) ?: return reply(error("NotFoundError", "an attachment names no app file")) }
        val types = files.map { dev.exact.canvas.Files.type(it) }
        val line = { action: String -> "$kind $action, ${to.size + cc.size + bcc.size} recipients, ${files.size} attachments" + if (types.isEmpty()) "" else " (${types.joinToString()})" }
        if (!(if (kind == "email") mail || context.agent else sms || context.agent)) return reply(error("NotSupportedError", "no app on this device composes a $kind; check composeStatus()"))
        if (context.agent) {
            val action = tap ?: "opened"
            say(line(action))
            return reply(JSONObject().put("action", action))
        }
        val scheme = if (kind == "email") "mailto:" else "smsto:${Uri.encode(to.joinToString(";"))}"
        val leaving = if (kind == "email") "mailto:${to.joinToString(",") { Uri.encode(it, "@") }}" else scheme
        if (!context.leave(leaving, "external")) {
            say("the embedder kept the app from opening the $kind composer")
            return reply(error("NotAllowedError", "the embedder declined to leave the app for the composer"))
        }
        val activity = context.activity ?: return reply(error("InvalidStateError", "no activity to open the composer from"))
        val uris = paths.map { context.contentUri(it) ?: return reply(error("NotReadableError", "an attachment could not be shared")) }
        val intent = when {
            uris.isEmpty() -> Intent(Intent.ACTION_SENDTO, Uri.parse(scheme))
            uris.size == 1 -> Intent(Intent.ACTION_SEND).putExtra(Intent.EXTRA_STREAM, uris[0])
            else -> Intent(Intent.ACTION_SEND_MULTIPLE).putParcelableArrayListExtra(Intent.EXTRA_STREAM, ArrayList(uris))
        }
        if (uris.isNotEmpty()) {
            intent.type = types.distinct().singleOrNull() ?: "*/*"
            // The grant goes with the clip: every stream readable by the app that answers, and only by it.
            intent.clipData = ClipData.newRawUri(null, uris[0]).apply { uris.drop(1).forEach { addItem(ClipData.Item(it)) } }
            intent.addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION)
            if (kind == "email") intent.selector = Intent(Intent.ACTION_SENDTO, Uri.parse("mailto:"))
            else Telephony.Sms.getDefaultSmsPackage(activity)?.let { intent.setPackage(it) }
        }
        if (kind == "email") {
            intent.putExtra(Intent.EXTRA_EMAIL, to.toTypedArray()).putExtra(Intent.EXTRA_CC, cc.toTypedArray()).putExtra(Intent.EXTRA_BCC, bcc.toTypedArray())
            if (subject.isNotEmpty()) intent.putExtra(Intent.EXTRA_SUBJECT, subject)
            if (body.isNotEmpty()) intent.putExtra(Intent.EXTRA_TEXT, body)
        } else {
            if (body.isNotEmpty()) intent.putExtra("sms_body", body)
            if (subject.isNotEmpty()) intent.putExtra("subject", subject)
            if (uris.isNotEmpty() && to.isNotEmpty()) intent.putExtra("address", to.joinToString(";"))
        }
        try {
            activity.startActivity(intent)
        } catch (e: ActivityNotFoundException) {
            say(line("failed"))
            return reply(error("NotSupportedError", "no app on this device composes this $kind"))
        }
        say(line("opened"))
        reply(JSONObject().put("action", "opened"))
    }
}
