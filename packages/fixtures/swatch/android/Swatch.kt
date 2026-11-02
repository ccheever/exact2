// @fixture/swatch on Android: a box filled with its `tint`, which sends `message` "tap" when tapped, made by the
// process's one extension for the session it belongs to, which keeps what each session was told of its views and
// answers it as `swatchReports`.
package fixture.swatch

import android.content.Context
import android.graphics.Color
import android.graphics.drawable.GradientDrawable
import android.view.View
import dev.exact.canvas.ExactExtension
import dev.exact.canvas.ExtensionContext
import org.json.JSONObject

class SwatchModule(context: ExtensionContext) : ExactExtension(context) {
    /** What each session heard, by session. */
    private val heard = HashMap<String, MutableList<String>>()

    private fun say(session: String, line: String) {
        heard.getOrPut(session) { ArrayList() } += line
        context.changed("swatch.swatchReports")
    }

    override fun later(name: String, args: JSONObject, reply: (JSONObject) -> Unit) {
        if (name != "swatchReports") return reply(JSONObject().put("error", "NotSupportedError"))
        val lines = heard[context.session].orEmpty()
        reply(JSONObject().put("text", lines.joinToString("; ")).put("count", lines.size))
    }

    override fun view(tag: String, session: String, id: String, context: Context, props: JSONObject, emit: (String, String) -> Unit): View {
        require(tag == "swatch-box") { "no view $tag" }
        val box = View(context)
        box.background = GradientDrawable().apply { cornerRadius = 8 * context.resources.displayMetrics.density }
        box.setOnClickListener { emit("message", "tap") }
        update(box, props)
        say(session, "made $id")
        return box
    }

    override fun update(view: View, props: JSONObject) {
        val tint = runCatching { Color.parseColor(props.optString("tint")) }.getOrNull() ?: Color.TRANSPARENT
        (view.background as? GradientDrawable)?.setColor(tint)
    }

    override fun displayed(session: String, id: String, shown: Boolean, origin: String) = say(session, "displayed $id $shown $origin")

    override fun bound(session: String, id: String, prop: String, origin: String) = say(session, "bound $id $prop $origin")
}
