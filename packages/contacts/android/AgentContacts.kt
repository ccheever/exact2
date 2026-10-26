// The agent's address book on Android (LLP 1133.018 §10), as Apple's `AgentContacts.swift` is: what @exact/contacts
// reads and picks from under the agent, where it asks the Contacts Provider for nothing. It is the package's
// `fixture/contacts.json`, or another file there the agent's `store` names, which an APK carries as the app's assets
// under `contacts-fixture/` (the fixture app's `assets/`; the reader puts an app's assets at `app/assets/` in the APK).
// What the app may read of it is the access the runner's status gives: everything under `full`; under `limited`,
// which a drive may set on any target, the fixture's `limited` list.
package exact.contacts

import android.content.res.AssetManager
import org.json.JSONArray
import org.json.JSONObject

internal class AgentContacts(private val assets: AssetManager) {
    companion object {
        val properties = listOf("address", "email", "icon", "name", "tel")

        /** A contact whose name, email or number holds [text], ignoring case. */
        fun matches(contact: JSONObject, text: String): Boolean {
            val needle = text.lowercase()
            return listOf("name", "email", "tel").any { key -> strings(contact.optJSONArray(key)).any { it.lowercase().contains(needle) } }
        }

        fun strings(a: JSONArray?): List<String> = a?.let { (0 until it.length()).map(it::getString) } ?: emptyList()
    }

    var contacts: List<JSONObject> = emptyList()
        private set
    private var limited: List<String> = emptyList()
    private var icons: Map<String, ByteArray> = emptyMap()
    var access = "none"
    /** Said once when the fixture is not in the APK. */
    var missing: String? = null
        private set

    init { if (!load("contacts.json")) missing = "no agent address book: the APK carries no assets/contacts-fixture/contacts.json" }

    /** Reads fixture file [name]; false where there is none. */
    fun load(name: String): Boolean {
        if (name.contains('/')) return false
        val json = try {
            JSONObject(assets.open("app/assets/contacts-fixture/$name").bufferedReader().use { it.readText() })
        } catch (e: java.io.IOException) { return false } catch (e: org.json.JSONException) { return false }
        val list = json.optJSONArray("contacts") ?: JSONArray()
        contacts = (0 until list.length()).map(list::getJSONObject)
        limited = strings(json.optJSONArray("limited"))
        val named = json.optJSONObject("icons") ?: JSONObject()
        icons = named.keys().asSequence().associateWith { android.util.Base64.decode(named.getString(it), android.util.Base64.DEFAULT) }
        return true
    }

    fun contact(id: String): JSONObject? = contacts.firstOrNull { it.optString("id") == id }

    /** The contacts the app may read now, in the fixture's order. */
    val visible: List<JSONObject>
        get() = when (access) {
            "full" -> contacts
            "limited" -> contacts.filter { it.optString("id") in limited }
            else -> emptyList()
        }

    /** The bytes of icon [name]. */
    fun icon(name: String): ByteArray? = icons[name]
}
