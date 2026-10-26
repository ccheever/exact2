// @exact/contacts' production path on Android (LLP 1133.018 stage 3): the system's contact picker, and the person's
// contacts through the Contacts Provider under `READ_CONTACTS`.
//
// - The picker needs no permission (D4). On Android 17 (API 37) it is the system's,
//   `ContactsPickerSessionContract.ACTION_PICK_CONTACTS`, asked for the data fields of the properties, several
//   contacts where `multiple`; its session URI is read once. Below 17 it is `ACTION_PICK` through one data type, the
//   first of `email`, `tel` and `address` asked, which grants a read of that one row: the answer has `name` and that
//   property, every other property asked an empty list, and one contact whatever `multiple` says
//   ([common intents](https://developer.android.com/guide/components/intents-common#Contacts)).
// - A store read asks the permission while `prompt`, runs on the module's thread (D6) and answers a page in the
//   provider's order, `SORT_KEY_PRIMARY` then `_ID`. Its cursor is that key, so a page asked again after a change
//   starts after the same place whether or not its contact is still there. `id` is the contact's lookup URI
//   (`CONTENT_LOOKUP_URI`), which outlives the provider's row ids; `ids=` reads by it.
// - A content observer on `Contacts.CONTENT_URI`, with descendants, is a change of the store, which the module
//   announces. Android has no event for a change in Settings: a return to the foreground reads the status again.
package exact.contacts

import android.Manifest
import android.app.Activity
import android.content.ContentResolver
import android.content.Intent
import android.content.pm.PackageManager
import android.database.ContentObserver
import android.database.Cursor
import android.net.Uri
import android.os.Build
import android.os.Handler
import android.os.Looper
import android.provider.ContactsContract
import android.provider.ContactsPickerSessionContract
import android.provider.ContactsContract.CommonDataKinds.Email
import android.provider.ContactsContract.CommonDataKinds.Organization
import android.provider.ContactsContract.CommonDataKinds.Phone
import android.provider.ContactsContract.CommonDataKinds.StructuredPostal
import android.provider.ContactsContract.Contacts
import android.provider.ContactsContract.Data
import dev.exact.canvas.ExtensionContext
import org.json.JSONArray
import org.json.JSONObject
import java.util.concurrent.Executors

internal class ContactStore(private val context: ExtensionContext, private val say: (String) -> Unit, private val changed: () -> Unit) {
    private val app get() = context.application
    private val resolver: ContentResolver get() = app.contentResolver
    private val main = Handler(Looper.getMainLooper())
    /** The module's thread: queries and icon copies. */
    private val work = Executors.newSingleThreadExecutor()
    private var picking = false
    private var picks = 0
    private var pages = 0
    private var observing = false
    /** Each store reader's icon directory, by its arguments. */
    private val directories = HashMap<String, String>()
    /** The status last answered, which a return to the foreground compares with. */
    private var said: String? = null
    private val observer = object : ContentObserver(main) {
        override fun onChange(selfChange: Boolean) = changed()
    }

    private val declared: Boolean by lazy {
        Manifest.permission.READ_CONTACTS in app.packageManager.getPackageInfo(app.packageName, PackageManager.GET_PERMISSIONS).requestedPermissions.orEmpty()
    }

    fun later(name: String, args: JSONObject, reply: (JSONObject) -> Unit) {
        when (name) {
            "getContactProperties" -> reply(JSONObject().put("properties", JSONArray(AgentContacts.properties)))
            "contactsStatus" -> reply(status())
            "selectContacts" -> pick(args, reply)
            "queryContacts" -> readable(reply) { observe(); page(args, reply) }
            else -> reply(ContactsModule.error("NotSupportedError"))
        }
    }

    // Access (D5)

    fun status(): JSONObject {
        val granted = app.checkSelfPermission(Manifest.permission.READ_CONTACTS) == PackageManager.PERMISSION_GRANTED
        // Not granted: the state is the host's history of the request (LLP 1133.008 D8 item 7).
        val history = if (declared) context.permission(Manifest.permission.READ_CONTACTS) else null
        val out = JSONObject()
            .put("available", true)
            .put("state", if (granted) "granted" else history?.state ?: "prompt")
            .put("access", if (granted) "full" else "none")
            .put("apple", JSONObject.NULL)
            .put("android", JSONObject()
                .put("shouldShowRationale", history?.shouldShowRationale == true)
                .put("permanentlyDenied", !granted && history?.permanentlyDenied == true))
        said = out.toString()
        return out
    }

    /** A return to the foreground: a grant changed in Settings is a change; narrowed to none, the icons go. */
    fun refresh() {
        val before = said ?: return
        val now = status()
        if (now.toString() == before) return
        if (now.getString("access") != "full") emptyIcons()
        changed()
    }

    /** [then] once the app may read: asked while `prompt`; refused while `denied` or undeclared. */
    private fun readable(reply: (JSONObject) -> Unit, then: () -> Unit) {
        if (!declared) return reply(ContactsModule.error("NotAllowedError", "the manifest declares no READ_CONTACTS"))
        when (status().getString("state")) {
            "granted" -> then()
            "denied" -> reply(ContactsModule.error("NotAllowedError"))
            else -> context.askPermission(Manifest.permission.READ_CONTACTS) {
                say("asked: ${status().getString("state")}")
                if (status().getString("state") == "granted") then() else reply(ContactsModule.error("NotAllowedError"))
            }
        }
    }

    private fun observe() {
        if (observing) return
        observing = true
        resolver.registerContentObserver(Contacts.CONTENT_URI, true, observer)
    }

    fun stopObserving() {
        if (!observing) return
        observing = false
        resolver.unregisterContentObserver(observer)
    }

    // The picker (D4)

    private fun pick(args: JSONObject, reply: (JSONObject) -> Unit) {
        val properties = ContactsModule.asked(args) ?: return reply(ContactsModule.error("TypeError", "properties is empty or names one the picker has not"))
        if (picking) return reply(ContactsModule.error("InvalidStateError", "a picker is showing"))
        val multiple = args.optBoolean("multiple")
        val android = args.optJSONObject("android")
        val system = Build.VERSION.SDK_INT >= 37
        // Below 17: one data type, the first asked of these, else the contact itself.
        val type = listOf("email" to Email.CONTENT_TYPE, "tel" to Phone.CONTENT_TYPE, "address" to StructuredPostal.CONTENT_TYPE)
            .firstOrNull { it.first in properties }
        val intent = if (system) systemPicker(properties, multiple, android) else Intent(Intent.ACTION_PICK).setType(type?.second ?: Contacts.CONTENT_TYPE)
        ContactsModule.pickIntentHatch?.invoke(intent)
        picking = true
        say("picker shown (${properties.joinToString(",")}${if (multiple) ", multiple" else ""}${if (system) "" else ", one ${type?.first ?: "contact"}"})")
        context.startActivityForResult(intent) { code, data ->
            picking = false
            val uri = data?.data
            if (code != Activity.RESULT_OK || uri == null) {
                say("picked 0")
                return@startActivityForResult reply(JSONObject().put("contacts", JSONArray()))
            }
            picks += 1
            val icons = "app:/tmp/picked/contacts/$picks"
            work.execute {
                val answer = try {
                    if (system) session(uri, properties, icons) else picked(uri, properties, type?.first, icons)
                } catch (e: SecurityException) { ContactsModule.error("NotAllowedError", e.message) } catch (e: RuntimeException) { ContactsModule.error("UnknownError", e.message) }
                main.post {
                    say("picked ${answer.optJSONArray("contacts")?.length() ?: 0}")
                    reply(answer)
                }
            }
        }
    }

    private fun systemPicker(properties: List<String>, multiple: Boolean, options: JSONObject?): Intent {
        val fields = ArrayList<String>()
        if ("email" in properties) fields += Email.CONTENT_ITEM_TYPE
        if ("tel" in properties) fields += Phone.CONTENT_ITEM_TYPE
        if ("address" in properties) fields += StructuredPostal.CONTENT_ITEM_TYPE
        return Intent(ContactsPickerSessionContract.ACTION_PICK_CONTACTS).apply {
            putExtra(Intent.EXTRA_USE_SYSTEM_CONTACTS_PICKER, true)
            putStringArrayListExtra(ContactsPickerSessionContract.EXTRA_PICK_CONTACTS_REQUESTED_DATA_FIELDS, fields)
            if (multiple) putExtra(Intent.EXTRA_ALLOW_MULTIPLE, true)
            if (options?.has("selectionLimit") == true) putExtra(ContactsPickerSessionContract.EXTRA_PICK_CONTACTS_SELECTION_LIMIT, options.getInt("selectionLimit"))
            if (options?.has("matchAllDataFields") == true) {
                putExtra(ContactsPickerSessionContract.EXTRA_PICK_CONTACTS_MATCH_ALL_DATA_FIELDS, options.getBoolean("matchAllDataFields"))
            }
        }
    }

    /** Android 17's session: one row a datum, `ContactsContract.Data`'s columns, grouped by `LOOKUP_KEY`. It takes no
     * selection, so it is read whole, once. */
    private fun session(uri: Uri, properties: List<String>, icons: String): JSONObject {
        val people = LinkedHashMap<String, Person>()
        resolver.query(uri, null, null, null, null)?.use { c ->
            while (c.moveToNext()) {
                val key = c.text(Contacts.LOOKUP_KEY) ?: continue
                val person = people.getOrPut(key) { Person(c.text(Contacts.DISPLAY_NAME_PRIMARY), c.text(Contacts.PHOTO_THUMBNAIL_URI)) }
                person.add(c)
            }
        }
        val out = people.values.mapIndexed { n, p -> p.record(properties, null, icons, n) }
        return JSONObject().put("contacts", JSONArray(out))
    }

    /** Below 17: the row the person chose, of [type] (`email`, `tel`, `address`) or the contact itself. */
    private fun picked(uri: Uri, properties: List<String>, type: String?, icons: String): JSONObject {
        val person = resolver.query(uri, null, null, null, null)?.use { c ->
            if (!c.moveToFirst()) null else Person(c.text(Contacts.DISPLAY_NAME_PRIMARY) ?: c.text(Contacts.DISPLAY_NAME), c.text(Contacts.PHOTO_THUMBNAIL_URI)).apply {
                if (type != null) add(c)
            }
        } ?: return JSONObject().put("contacts", JSONArray())
        return JSONObject().put("contacts", JSONArray().put(person.record(properties, null, icons, 0)))
    }

    // The store (D6)

    private fun page(args: JSONObject, reply: (JSONObject) -> Unit) {
        val properties = ContactsModule.asked(args) ?: return reply(ContactsModule.error("TypeError", "properties is empty or names one the store has not"))
        val limit = args.optInt("limit", 100).coerceIn(1, 1000)
        val text = if (args.isNull("text")) null else args.optString("text").ifEmpty { null }
        val ids = args.optJSONArray("ids")?.let(AgentContacts::strings)
        val after = if (args.has("after") && !args.isNull("after")) args.getString("after") else null
        // This reader's icons from its last page go with that answer.
        val key = ContactsModule.key(args)
        directories.remove(key)?.let(::remove)
        pages += 1
        val icons = "app:/tmp/contacts/$pages"
        if ("icon" in properties) directories[key] = icons
        work.execute {
            val began = System.nanoTime()
            val answer = try { read(properties, limit, text, ids, after, icons) } catch (e: SecurityException) {
                ContactsModule.error("NotAllowedError", e.message)
            } catch (e: IllegalArgumentException) { ContactsModule.error("TypeError", e.message) } catch (e: RuntimeException) { ContactsModule.error("UnknownError", e.message) }
            val ms = (System.nanoTime() - began) / 1_000_000
            main.post {
                if (!answer.has("error")) say("page queryContacts ${answer.getJSONArray("contacts").length()} after ${after ?: "start"} in $ms ms (full)")
                reply(answer)
            }
        }
    }

    private fun read(properties: List<String>, limit: Int, text: String?, ids: List<String>?, after: String?, icons: String): JSONObject {
        val base = text?.let { Uri.withAppendedPath(Contacts.CONTENT_FILTER_URI, Uri.encode(it)) } ?: Contacts.CONTENT_URI
        // One more than the page, to know whether another follows.
        val uri = base.buildUpon().appendQueryParameter(ContactsContract.LIMIT_PARAM_KEY, (limit + 1).toString()).build()
        val where = ArrayList<String>()
        val values = ArrayList<String>()
        if (ids != null) {
            val rows = ids.mapNotNull { id -> try { Contacts.lookupContact(resolver, Uri.parse(id)) } catch (e: IllegalArgumentException) { null } }
                .mapNotNull { looked -> resolver.query(looked, arrayOf(Contacts._ID), null, null, null)?.use { if (it.moveToFirst()) it.getLong(0) else null } }
            where += "${Contacts._ID} IN (${rows.joinToString(",").ifEmpty { "-1" }})"
        }
        if (after != null) {
            // `<_ID>|<sort key>`: after that place in the order, whether or not the contact still exists.
            val bar = after.indexOf('|')
            val id = after.substring(0, bar.coerceAtLeast(0)).toLongOrNull()
            if (bar < 0 || id == null) return ContactsModule.error("TypeError", "after is not a cursor")
            val sort = after.substring(bar + 1)
            where += "(${Contacts.SORT_KEY_PRIMARY} > ? OR (${Contacts.SORT_KEY_PRIMARY} = ? AND ${Contacts._ID} > ?))"
            values += listOf(sort, sort, id.toString())
        }
        val columns = arrayOf(Contacts._ID, Contacts.LOOKUP_KEY, Contacts.DISPLAY_NAME_PRIMARY, Contacts.SORT_KEY_PRIMARY, Contacts.PHOTO_THUMBNAIL_URI)
        val rows = ArrayList<Triple<Long, String, Person>>()
        resolver.query(uri, columns, where.joinToString(" AND ").ifEmpty { null }, values.toTypedArray(), "${Contacts.SORT_KEY_PRIMARY}, ${Contacts._ID}")?.use { c ->
            while (c.moveToNext() && rows.size <= limit) {
                rows += Triple(c.getLong(0), c.getString(3).orEmpty(), Person(c.getString(2), c.getString(4), Contacts.getLookupUri(c.getLong(0), c.getString(1)).toString()))
            }
        }
        val more = rows.size > limit
        val shown = rows.take(limit)
        // The data of the page's contacts in one query, by the properties asked.
        val kinds = listOfNotNull(
            Email.CONTENT_ITEM_TYPE.takeIf { "email" in properties }, Phone.CONTENT_ITEM_TYPE.takeIf { "tel" in properties },
            StructuredPostal.CONTENT_ITEM_TYPE.takeIf { "address" in properties }, Organization.CONTENT_ITEM_TYPE.takeIf { "address" in properties },
        )
        if (kinds.isNotEmpty() && shown.isNotEmpty()) {
            val byId = shown.associate { it.first to it.third }
            val selection = "${Data.CONTACT_ID} IN (${byId.keys.joinToString(",")}) AND ${Data.MIMETYPE} IN (${kinds.joinToString(",") { "?" }})"
            resolver.query(Data.CONTENT_URI, null, selection, kinds.toTypedArray(), "${Data.CONTACT_ID}, ${Data.IS_SUPER_PRIMARY} DESC, ${Data._ID}")?.use { c ->
                while (c.moveToNext()) byId[c.getLong(c.getColumnIndexOrThrow(Data.CONTACT_ID))]?.add(c)
            }
        }
        val next: Any = if (more) shown.last().let { "${it.first}|${it.second}" } else JSONObject.NULL
        return JSONObject().put("contacts", JSONArray(shown.mapIndexed { n, (_, _, p) -> p.record(properties, p.id, icons, n) })).put("next", next)
    }

    /** No reader reads with [args] any longer: its icons go. */
    fun release(args: JSONObject) { directories.remove(ContactsModule.key(args))?.let(::remove) }

    /** Every store icon, as at a launch's first call or when access narrows to none. */
    fun emptyIcons() {
        remove("app:/tmp/contacts")
        directories.clear()
    }

    private fun remove(dir: String) { context.file("$dir/x")?.parentFile?.deleteRecursively() }

    // A contact, as read

    private inner class Person(val name: String?, val photo: String?, val id: String? = null) {
        val email = ArrayList<String>()
        val tel = ArrayList<String>()
        val address = ArrayList<JSONObject>()
        var organization = ""

        /** One `ContactsContract.Data` row of this contact. */
        fun add(c: Cursor) {
            val value = c.text(Data.DATA1) ?: return
            when (c.text(Data.MIMETYPE)) {
                Email.CONTENT_ITEM_TYPE -> email += value
                Phone.CONTENT_ITEM_TYPE -> tel += value
                Organization.CONTENT_ITEM_TYPE -> organization = c.text(Organization.COMPANY).orEmpty()
                StructuredPostal.CONTENT_ITEM_TYPE -> address += JSONObject()
                    .put("addressLine", JSONArray((c.text(StructuredPostal.STREET) ?: value).split('\n').filter { it.isNotBlank() }))
                    .put("city", c.text(StructuredPostal.CITY).orEmpty())
                    // A two-letter country is the region code the spec asks; Android keeps what was typed.
                    .put("country", c.text(StructuredPostal.COUNTRY).orEmpty().let { if (it.length == 2) it.uppercase() else it })
                    .put("dependentLocality", c.text(StructuredPostal.NEIGHBORHOOD).orEmpty())
                    .put("organization", "").put("phone", "")
                    .put("postalCode", c.text(StructuredPostal.POSTCODE).orEmpty())
                    .put("recipient", name.orEmpty())
                    .put("region", c.text(StructuredPostal.REGION).orEmpty())
                    .put("sortingCode", "")
            }
        }

        /** The answer's record, with the properties asked; its icon, the [n]th of its answer, written under [icons]. */
        fun record(properties: List<String>, id: String?, icons: String, n: Int): JSONObject {
            val out = ContactsModule.blank(id)
            for (p in properties) when (p) {
                "name" -> out.put("name", JSONArray(listOfNotNull(name?.ifEmpty { null })))
                "email" -> out.put("email", JSONArray(email))
                "tel" -> out.put("tel", JSONArray(tel))
                "address" -> out.put("address", JSONArray(address.map { it.put("organization", organization) }))
                "icon" -> out.put("icon", JSONArray(listOfNotNull(icon(icons, n))))
            }
            return out
        }

        private fun icon(icons: String, n: Int): String? {
            val uri = photo?.let(Uri::parse) ?: return null
            return try {
                val bytes = resolver.openInputStream(uri)?.use { it.readBytes() } ?: return null
                val path = "$icons/$n-0.${if (bytes.size > 1 && bytes[0] == 0x89.toByte() && bytes[1] == 0x50.toByte()) "png" else "jpg"}"
                val file = context.file(path) ?: return null
                file.parentFile?.mkdirs()
                file.writeBytes(bytes)
                path
            } catch (e: java.io.IOException) { null } catch (e: SecurityException) { null }
        }
    }

    private fun Cursor.text(column: String): String? = getColumnIndex(column).takeIf { it >= 0 && !isNull(it) }?.let(::getString)
}
