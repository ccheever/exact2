// @exact/contacts on Android (LLP 1133.018): the person's contacts, picked through the system's picker or read from
// the Contacts Provider under `READ_CONTACTS`. Under the agent it asks the provider for nothing and answers from the
// agent's address book (AgentContacts.kt, §10); the runner holds the permission and sheet tickets and answers the
// status. Otherwise the provider answers (ContactStore.kt).
//
// - A pick is a copy (D4): no `id`, icons written to `app:/tmp/picked/contacts/` for the app to keep.
// - A store read answers a bounded page (D6) whose cursor is a key in the provider's order, `SORT_KEY_PRIMARY` then
//   `_ID`. Its icons are written to `app:/tmp/contacts/` and deleted with their answer: when the reader is released,
//   when its page is answered again, at the first call of a launch, and when access narrows to none.
// - A change is announced, coalesced (D7): while the re-asks one announcement caused are in flight, further changes
//   set a mark, and one more announcement follows them landing.
// - Android has no limited access to contacts and no access picker: `showContactAccessPicker` is iOS's.
// - Tier 3 (D10): `pickIntentHatch` sees the picker's `Intent` after the module's extras, before it starts, on the
//   main thread, never under the agent.
package exact.contacts

import android.content.Intent
import android.os.Handler
import android.os.Looper
import dev.exact.canvas.ExactExtension
import dev.exact.canvas.ExtensionContext
import org.json.JSONArray
import org.json.JSONObject

class ContactsModule(context: ExtensionContext) : ExactExtension(context) {
    companion object {
        /** The app's picker hatch (D10): the picker's `Intent`, which it may add extras to. */
        @JvmStatic var pickIntentHatch: ((Intent) -> Unit)? = null
        /** Emptied once a launch. */
        private var emptied = false

        /** The properties [args] asks for, or null when it asks none or one no picker has. */
        internal fun asked(args: JSONObject): List<String>? {
            val asked = AgentContacts.strings(args.optJSONArray("properties"))
            return if (asked.isNotEmpty() && asked.all { it in AgentContacts.properties }) asked else null
        }

        /** A reader's arguments as one key, its page's place included. */
        internal fun key(args: JSONObject): String = args.keys().asSequence().sorted().joinToString(",") { "$it=${args.opt(it)}" }

        internal fun error(code: String, message: String? = null) = JSONObject().put("error", code).apply { if (message != null) put("message", message) }

        /** A record with every property absent, as the picker's answer spells one not asked. */
        internal fun blank(id: String?) = JSONObject().put("id", id ?: JSONObject.NULL).put("name", JSONObject.NULL).put("email", JSONObject.NULL)
            .put("tel", JSONObject.NULL).put("address", JSONObject.NULL).put("icon", JSONObject.NULL)
    }

    private val main = Handler(Looper.getMainLooper())
    private val book by lazy { AgentContacts(context.application.assets) }
    /** The Contacts Provider, outside the agent. */
    private val real by lazy { ContactStore(context, ::say, ::changedStore) }
    private var picks = 0
    private var pages = 0
    /** The live names read and not released, which observing follows. */
    private val readers = HashSet<String>()
    /** Each store reader's icon directory, by its arguments. */
    private val directories = HashMap<String, String>()
    /** Coalescing (D7): an announcement's re-asks in flight; a change since. */
    private var awaiting = false
    private var dirty = false
    private var rounds = 0
    /** The agent's words last seen, so each new value acts once. */
    private var store = "contacts.json"
    private var notify: Int? = null

    init {
        if (!emptied) {
            emptied = true
            remove("app:/tmp/contacts")
        }
        if (context.agent) book.missing?.let { say(it) }
    }

    private fun say(line: String) = context.log("contacts: $line")

    override fun later(name: String, args: JSONObject, reply: (JSONObject) -> Unit) = later(name, args, null, reply)

    override fun later(name: String, args: JSONObject, preferred: Any?, reply: (JSONObject) -> Unit) {
        if (!context.agent) {
            if (name == "queryContacts") readers += name
            return real.later(name, args) { answer ->
                reply(answer)
                if (name == "queryContacts" && awaiting) main.post { landed() }
            }
        }
        when (name) {
            "getContactProperties" -> reply(JSONObject().put("properties", JSONArray(AgentContacts.properties)))
            "contactsStatus" -> reply(status())
            "selectContacts" -> reply(pick(args, preferred as? JSONObject))
            "queryContacts" -> {
                readers += name
                reply(page(args))
                if (awaiting) main.post { landed() }
            }
            else -> reply(error("NotSupportedError"))
        }
    }

    override fun release(name: String) {
        if (!readers.remove(name) || readers.isNotEmpty()) return
        say("no reader: observing stops")
        if (!context.agent) real.stopObserving()
    }

    override fun release(name: String, args: JSONObject) {
        if (!context.agent) return real.release(args)
        directories.remove(key(args))?.let(::remove)
    }

    /** Under the agent: the access the runner's status gives, the store the agent names, and a burst of `notify`
     * changes. */
    override fun preferred(values: JSONObject) {
        if (!context.agent) return
        val access = values.optJSONObject("contactsStatus")?.optString("access")?.ifEmpty { null } ?: book.access
        val file = if (values.isNull("store")) "contacts.json" else values.optString("store").ifEmpty { "contacts.json" }
        val narrowed = access != book.access
        book.access = access
        if (access != "full" && access != "limited") emptyStoreIcons()
        if (file != store) {
            store = file
            say(if (book.load(file)) "store $file" else "store $file: no such fixture")
            changedStore()
        } else if (narrowed) {
            changedStore()
        }
        val burst = if (values.has("notify")) values.optInt("notify") else null
        if (burst != null && burst != notify) {
            say("notify $burst")
            repeat(burst) { changedStore() }
        }
        notify = burst
    }

    override fun permissions(states: JSONObject) {
        if (context.agent && states.optString("contacts") == "denied") {
            preferred(JSONObject().put("contactsStatus", JSONObject().put("access", "none")).put("store", store).apply { notify?.let { put("notify", it) } })
        }
    }

    override fun foreground(shown: Boolean) {
        if (!shown) return
        // Access may have changed in Settings meanwhile (D5).
        if (!context.agent) real.refresh()
        changedStore()
    }

    // Announcing (D7)

    /** The store changed: announced, or marked while an announcement's re-asks are in flight. */
    private fun changedStore() {
        if (readers.isEmpty()) return
        if (awaiting) { dirty = true; return }
        announce()
    }

    private fun announce() {
        awaiting = true
        rounds += 1
        say("announced (round $rounds)")
        context.changed("contacts.queryContacts")
        context.changed("contacts.contactsStatus")
        // A round no reader asks again still ends.
        val round = rounds
        main.postDelayed({ if (rounds == round) landed() }, 2000)
    }

    private fun landed() {
        if (!awaiting) return
        awaiting = false
        if (dirty) {
            dirty = false
            announce()
        }
    }

    // The agent's answers (§10)

    private fun status() = JSONObject().put("available", true).put("state", "prompt").put("access", book.access)
        .put("apple", JSONObject.NULL).put("android", JSONObject.NULL)

    private fun pick(args: JSONObject, sheet: JSONObject?): JSONObject {
        val properties = asked(args) ?: return error("TypeError", "properties is empty or names one the picker has not")
        val ids = sheet?.optJSONArray("typed")?.let(AgentContacts::strings) ?: return JSONObject().put("contacts", JSONArray())
        picks += 1
        val out = JSONArray()
        for (id in ids) {
            val contact = book.contact(id) ?: return error("NotFoundError", "no fixture contact $id")
            out.put(record(contact, properties, false, "app:/tmp/picked/contacts/$picks"))
        }
        say("picked ${ids.joinToString(",")} (${properties.joinToString(",")})")
        return JSONObject().put("contacts", out)
    }

    private fun page(args: JSONObject): JSONObject {
        val properties = asked(args) ?: return error("TypeError", "properties is empty or names one the store has not")
        if (book.access != "full" && book.access != "limited") return error("NotAllowedError")
        var list = book.visible
        args.optJSONArray("ids")?.let { a -> val ids = AgentContacts.strings(a).toSet(); list = list.filter { it.optString("id") in ids } }
        val text = if (args.isNull("text")) "" else args.optString("text")
        if (text.isNotEmpty()) list = list.filter { AgentContacts.matches(it, text) }
        if (args.has("after") && !args.isNull("after")) {
            val after = args.getString("after")
            val i = list.indexOfFirst { it.optString("id") == after }
            if (i < 0) return error("InvalidStateError", "cursor-stale")
            list = list.drop(i + 1)
        }
        val limit = args.optInt("limit", 100).coerceIn(1, 1000)
        val shown = list.take(limit)
        // This reader's icons from its last page go with that answer.
        val key = key(args)
        directories.remove(key)?.let(::remove)
        pages += 1
        val dir = "app:/tmp/contacts/$pages"
        if ("icon" in properties) directories[key] = dir
        val next: Any = if (list.size > limit) shown.last().optString("id") else JSONObject.NULL
        say("page queryContacts ${shown.size} of ${list.size} after ${args.optString("after").ifEmpty { "start" }} (${book.access})")
        return JSONObject().put("contacts", JSONArray(shown.map { record(it, properties, true, dir) })).put("next", next)
    }

    /** [contact] with the properties asked, its icons written under [icons]. */
    private fun record(contact: JSONObject, properties: List<String>, id: Boolean, icons: String): JSONObject {
        val cid = contact.optString("id")
        val out = blank(if (id) cid else null)
        for (p in properties) {
            if (p != "icon") { out.put(p, contact.optJSONArray(p) ?: JSONArray()); continue }
            val paths = JSONArray()
            AgentContacts.strings(contact.optJSONArray("icon")).forEachIndexed { i, name ->
                val path = "$icons/$cid-$i.png"
                val bytes = book.icon(name) ?: return@forEachIndexed
                val file = context.file(path) ?: return@forEachIndexed
                try {
                    file.parentFile?.mkdirs()
                    file.writeBytes(bytes)
                    paths.put(path)
                } catch (e: java.io.IOException) { say("icon $path: $e") }
            }
            out.put("icon", paths)
        }
        return out
    }

    private fun remove(dir: String) { context.file("$dir/x")?.parentFile?.deleteRecursively() }

    private fun emptyStoreIcons() {
        remove("app:/tmp/contacts")
        directories.clear()
    }
}
