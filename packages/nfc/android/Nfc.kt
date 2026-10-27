// @exact/nfc on Android (LLP 1133.035 stage 3): reader mode on the reader activity, answered in the interface's
// shapes. Under the agent the runner's declared substitute answers and nothing here runs, unless the drive binds it.
//
// - One registry per process (D6): a scan or a write is a claim of the session's instance. Reader mode is on while a
//   claim lives and the activity is resumed, `enableReaderMode` in `resumed()` and `disableReaderMode` in `paused()`
//   (D16 item 5), so a translucent activity over the app suspends a scan without ending it.
// - Claims share one reader's settings (D6): `android.technologies`, `skipNdefCheck`, `platformSounds` and
//   `presenceCheckDelay`, each word left out being D5's default. Another claim's settings fail `InvalidStateError`
//   `busy`; a write never shares a reader with `skipNdefCheck`.
// - A tag is handled on the reader callback's thread, which blocking I/O needs (`Ndef.writeNdefMessage` "must not be
//   called from the main application thread"), and answered on the main thread.
// - One write at a time: a newer write or lock replaces the waiting one, which fails `AbortError`. A tag that arrives
//   while a write waits is written and not read (D4).
// - `radio` follows `ACTION_ADAPTER_STATE_CHANGED`, and is read again on `resumed()`. A scan started while it is off
//   fails `NotReadableError` `radio-off`; one running when it goes off is suspended until it comes back.
// - Tier 3 (D14): `NfcModule.hatch` is given the adapter once, and each tag before the module reads it, on the reader
//   callback's thread.
package exact.nfc

import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.content.IntentFilter
import android.nfc.FormatException
import android.nfc.NdefMessage
import android.nfc.NdefRecord
import android.nfc.NfcAdapter
import android.nfc.Tag
import android.nfc.tech.IsoDep
import android.nfc.tech.MifareClassic
import android.nfc.tech.MifareUltralight
import android.nfc.tech.Ndef
import android.nfc.tech.NdefFormatable
import android.nfc.tech.NfcA
import android.nfc.tech.NfcB
import android.nfc.tech.NfcF
import android.nfc.tech.NfcV
import android.nfc.tech.TagTechnology
import android.os.Bundle
import android.os.Handler
import android.os.Looper
import android.provider.Settings
import android.util.Base64
import dev.exact.canvas.ExactExtension
import dev.exact.canvas.ExtensionContext
import org.json.JSONArray
import org.json.JSONObject
import java.io.IOException

/** The tier 3 hatch (D14). */
interface NfcHatch {
    /** The adapter, once, when the module is made. */
    fun adapter(adapter: NfcAdapter) {}
    /** Each tag before the module reads it, on the reader callback's thread; hand it back closed. */
    fun tag(tag: Tag) {}
}

/** One call waiting on the reader: a scan, a write or a lock. */
internal class Claim(
    val kind: String,
    val owner: NfcModule,
    val request: Int?,
    val args: JSONObject,
    val settings: JSONObject,
    var reply: ((JSONObject) -> Unit)?,
) {
    val once = args.optBoolean("once", false)
    fun settle(answer: JSONObject) { val r = reply; reply = null; r?.invoke(answer) }
}

class NfcModule(context: ExtensionContext) : ExactExtension(context) {
    companion object {
        @JvmStatic var hatch: NfcHatch? = null
        internal const val KEPT = 16
    }

    private val main = Handler(Looper.getMainLooper())
    internal val adapter: NfcAdapter? = NfcAdapter.getDefaultAdapter(context.application)?.also { hatch?.adapter(it) }
    /** The last 16 readings' records with their bytes, by reading id (D2). */
    private val readings = ArrayDeque<Pair<String, JSONArray>>()
    /** The tags of this session's live scans, by their `NFCTag` id (D11). */
    internal val leases = HashMap<String, Tag>()
    private var last = 0
    /** The main thread's count of emitted events, which keeps two alike from coalescing. */
    private var emitted = 0

    init { Registry.attach(this) }

    private fun status() = JSONObject()
        .put("available", adapter != null)
        .put("radio", if (adapter == null) "unknown" else if (adapter.isEnabled) "on" else "off")
        .put("state", "granted")
        .put("readerUI", "app")

    override fun later(name: String, args: JSONObject, reply: (JSONObject) -> Unit) {
        when (name) {
            "nfcStatus" -> reply(status())
            "scan", "write", "makeReadOnly" -> {
                if (adapter == null) return reply(error("NotSupportedError"))
                if (name == "write") {
                    val records = args.optJSONObject("message")?.optJSONArray("records")
                    if (records == null || records.length() == 0) return reply(error("TypeError", "a message has a record"))
                    try { message(records) } catch (e: IllegalArgumentException) { return reply(error("TypeError", e.message)) }
                }
                if (name == "scan" && !adapter.isEnabled) return reply(error("NotReadableError", "radio-off"))
                Registry.claim(Claim(name, this, context.request, args, settings(name, args), reply))
            }
            "abortScan" -> { Registry.end(this, setOf("scan"), ended("stopped")); reply(JSONObject()) }
            "abortWrite" -> { Registry.end(this, setOf("write", "makeReadOnly"), error("AbortError")); reply(JSONObject()) }
            "ndefRecords" -> {
                val id = args.optString("reading")
                val kept = readings.firstOrNull { it.first == id }
                reply(if (kept != null) JSONObject().put("exact:answer", kept.second) else error("NotFoundError", id))
            }
            "transceive" -> Registry.transceive(this, args, reply)
            "openNFCSettings" -> {
                val activity = context.activity
                val intent = Intent(Settings.ACTION_NFC_SETTINGS)
                try { (activity ?: context.application).startActivity(if (activity == null) intent.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK) else intent) }
                catch (e: android.content.ActivityNotFoundException) {
                    // "In some cases, a matching Activity may not exist": the app's own details instead.
                    val details = Intent(Settings.ACTION_APPLICATION_DETAILS_SETTINGS, android.net.Uri.parse("package:" + context.application.packageName))
                    (activity ?: context.application).startActivity(if (activity == null) details.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK) else details)
                }
                reply(JSONObject())
            }
            else -> reply(error("NotSupportedError"))
        }
    }

    override fun cancel(request: Int) = Registry.cancel(this, request)

    override fun foreground(shown: Boolean) { if (!shown) Registry.background(this) }

    override fun resumed() { Registry.resumed(); context.changed("nfc.nfcStatus") }

    override fun paused() = Registry.paused()

    override fun destroy(done: () -> Unit) {
        Registry.end(this, setOf("scan"), ended("stopped"))
        Registry.end(this, setOf("write", "makeReadOnly"), error("AbortError"))
        done()
    }

    internal fun radioChanged() = context.changed("nfc.nfcStatus")

    /** A reading of this session: kept for `ndefRecords`, raised on the root. */
    internal fun read(tag: Tag, message: NdefMessage?): String {
        val id = "r${++last}"
        val records = message?.records ?: emptyArray()
        readings.addLast(id to JSONArray(records.map { Records.json(it, full = true) }))
        while (readings.size > KEPT) readings.removeFirst()
        leases[id] = tag
        val reading = identity(tag).put("id", id).put("records", JSONArray(records.map { Records.json(it, full = false) }))
        emit("reading", reading)
        return id
    }

    /** A tag with no NDEF (D8): `readingerror` with its `NFCTag`. */
    internal fun unread(tag: Tag) {
        val id = "t${++last}"
        leases[id] = tag
        emit("readingerror", identity(tag).put("id", id))
    }

    /** An event on the root, with the device's origin (LLP 1133.004 D6), through the module's one channel. */
    private fun emit(event: String, arg: JSONObject) = main.post {
        context.changed("exact-event:" + JSONObject().put("extension", "nfc").put("event", event).put("args", JSONArray().put(arg)).put("seq", ++emitted))
    }

    private fun identity(tag: Tag): JSONObject {
        val id = tag.id ?: ByteArray(0)
        val android = JSONObject().put("id", Records.hex(id, ""))
            .put("techList", JSONArray(tag.techList.map { it.substringAfterLast('.') }))
            .put("maxTransceiveLength", maxTransceive(tag))
        return JSONObject().put("serialNumber", Records.hex(id, ":")).put("apple", JSONObject.NULL).put("android", android)
    }

    private fun maxTransceive(tag: Tag): Int = listOf<(Tag) -> TagTechnology?>({ IsoDep.get(it) }, { NfcA.get(it) }, { NfcB.get(it) }, { NfcF.get(it) }, { NfcV.get(it) })
        .firstNotNullOfOrNull { it(tag) }?.let { t ->
            when (t) { is IsoDep -> t.maxTransceiveLength; is NfcA -> t.maxTransceiveLength; is NfcB -> t.maxTransceiveLength; is NfcF -> t.maxTransceiveLength; is NfcV -> t.maxTransceiveLength; else -> 0 }
        } ?: 0

    /** A claim's reader settings (D6): its words, a word left out being the default. */
    private fun settings(name: String, args: JSONObject): JSONObject {
        val a = args.optJSONObject("android") ?: JSONObject()
        val techs = a.optJSONArray("technologies")?.let { j -> (0 until j.length()).map { j.getString(it) }.sorted() } ?: listOf("nfcA", "nfcB", "nfcF", "nfcV")
        return JSONObject().put("technologies", JSONArray(techs))
            .put("skipNdefCheck", name == "scan" && a.optBoolean("skipNdefCheck", false))
            .put("platformSounds", a.optBoolean("platformSounds", true))
            .put("presenceCheckDelay", if (a.has("presenceCheckDelay")) a.getInt("presenceCheckDelay") else JSONObject.NULL)
    }

    internal fun message(records: JSONArray): NdefMessage = NdefMessage((0 until records.length()).map { Records.init(records.getJSONObject(it), context.application.resources.configuration.locales[0].language.ifEmpty { "en" }) }.toTypedArray())
}

internal fun error(name: String, message: String? = null): JSONObject = JSONObject().put("error", name).put("message", message ?: JSONObject.NULL)

internal fun ended(reason: String, reading: String? = null, message: String? = null): JSONObject =
    JSONObject().put("reason", reason).put("reading", reading ?: JSONObject.NULL).put("message", message ?: JSONObject.NULL)

/** The process's one reader (D6). */
internal object Registry : NfcAdapter.ReaderCallback {
    private val main = Handler(Looper.getMainLooper())
    private val claims = ArrayList<Claim>()
    private val modules = LinkedHashSet<NfcModule>()
    private var resumed = true
    private var reading = false
    private var watching = false

    fun attach(m: NfcModule) {
        modules += m
        if (watching) return
        watching = true
        m.context.application.registerReceiver(object : BroadcastReceiver() {
            override fun onReceive(c: Context, i: Intent) {
                modules.forEach { it.radioChanged() }
                apply()
            }
        }, IntentFilter(NfcAdapter.ACTION_ADAPTER_STATE_CHANGED))
    }

    private fun current(): JSONObject? = claims.firstOrNull()?.settings

    fun claim(c: Claim) {
        val running = current()
        val writes = c.kind != "scan"
        val hidden = claims.any { (writes && it.settings.optBoolean("skipNdefCheck")) || (it.kind != "scan" && c.settings.optBoolean("skipNdefCheck")) }
        if (running != null && (running.toString() != c.settings.toString() || hidden)) return c.settle(error("InvalidStateError", "busy"))
        if (writes) {
            val old = claims.filter { it.kind != "scan" }
            claims.removeAll(old.toSet())
            old.forEach { it.settle(error("AbortError", "replaced")) }
        }
        claims += c
        apply()
    }

    fun end(owner: NfcModule, kinds: Set<String>, answer: JSONObject) {
        val gone = claims.filter { it.owner === owner && it.kind in kinds }
        claims.removeAll(gone.toSet())
        gone.forEach { it.settle(answer) }
        if ("scan" in kinds) owner.leases.clear()
        apply()
    }

    fun cancel(owner: NfcModule, request: Int) {
        val c = claims.firstOrNull { it.owner === owner && it.request == request } ?: return
        claims.remove(c)
        c.settle(if (c.kind == "scan") ended("stopped") else error("AbortError", "let go"))
        apply()
    }

    /** Leaving the foreground fails a waiting write (D6's table); a scan is suspended with reader mode. */
    fun background(owner: NfcModule) = end(owner, setOf("write", "makeReadOnly"), error("AbortError", "background"))

    fun resumed() { resumed = true; apply() }

    fun paused() { resumed = false; apply() }

    /** Reader mode on while a claim lives, the activity is resumed and the radio is on; off otherwise. */
    private fun apply() {
        val module = modules.firstOrNull() ?: return
        val adapter = module.adapter ?: return
        val activity = module.context.activity ?: return
        val want = claims.isNotEmpty() && resumed && adapter.isEnabled
        if (want == reading && !want) return
        if (want) {
            val s = current()!!
            var flags = 0
            val techs = s.getJSONArray("technologies")
            for (i in 0 until techs.length()) flags = flags or when (techs.getString(i)) {
                "nfcA" -> NfcAdapter.FLAG_READER_NFC_A; "nfcB" -> NfcAdapter.FLAG_READER_NFC_B; "nfcF" -> NfcAdapter.FLAG_READER_NFC_F
                "nfcV" -> NfcAdapter.FLAG_READER_NFC_V; "barcode" -> NfcAdapter.FLAG_READER_NFC_BARCODE; else -> 0
            }
            if (s.optBoolean("skipNdefCheck")) flags = flags or NfcAdapter.FLAG_READER_SKIP_NDEF_CHECK
            if (!s.optBoolean("platformSounds", true)) flags = flags or NfcAdapter.FLAG_READER_NO_PLATFORM_SOUNDS
            val extras = Bundle()
            if (!s.isNull("presenceCheckDelay")) extras.putInt(NfcAdapter.EXTRA_READER_PRESENCE_CHECK_DELAY, s.getInt("presenceCheckDelay"))
            adapter.enableReaderMode(activity, this, flags, extras)
        } else adapter.disableReaderMode(activity)
        reading = want
    }

    /** A tag, on the reader callback's thread: the waiting write's, else read for every live scan. */
    override fun onTagDiscovered(tag: Tag) {
        NfcModule.hatch?.tag(tag)
        val write = onMain { claims.lastOrNull { it.kind != "scan" } }
        if (write != null) {
            val answer = written(write, tag)
            return main.post { if (claims.remove(write)) { write.settle(answer); apply() } }
        }
        val ndef = Ndef.get(tag)
        val message = try { ndef?.use { it.connect(); it.ndefMessage } } catch (e: IOException) { null } catch (e: FormatException) { null }
        main.post {
            val scans = claims.filter { it.kind == "scan" }
            val ids = HashMap<NfcModule, String>()
            for (c in scans) {
                if (ndef == null) { if (ids[c.owner] == null) { c.owner.unread(tag); ids[c.owner] = "" } }
                else ids.getOrPut(c.owner) { c.owner.read(tag, message) }
            }
            if (ndef == null) return@post
            val done = scans.filter { it.once }
            claims.removeAll(done.toSet())
            done.forEach { it.settle(ended("read", ids[it.owner])) }
            apply()
        }
    }

    /** D7's steps on [tag] for [c], off the main thread. */
    private fun written(c: Claim, tag: Tag): JSONObject {
        val serial = JSONObject().put("serialNumber", Records.hex(tag.id ?: ByteArray(0), ":"))
        val lock = c.kind == "makeReadOnly"
        val message = if (lock) null else c.owner.message(c.args.getJSONObject("message").getJSONArray("records"))
        val ndef = Ndef.get(tag)
        try {
            if (ndef == null) {
                val formatable = NdefFormatable.get(tag) ?: return error("NotSupportedError", "not-ndef")
                formatable.use {
                    it.connect()
                    if (lock) it.formatReadOnly(NdefMessage(NdefRecord(NdefRecord.TNF_EMPTY, null, null, null))) else it.format(message)
                }
                return serial
            }
            ndef.use {
                it.connect()
                if (!lock && !c.args.optBoolean("overwrite", true) && (it.ndefMessage?.records?.isNotEmpty() == true)) return error("NotAllowedError", "overwrite")
                if (!it.isWritable) return error("NetworkError", "read-only")
                if (lock) {
                    if (!it.canMakeReadOnly()) return error("NotSupportedError", "not-ndef")
                    if (!it.makeReadOnly()) return error("NetworkError", "failed")
                    return serial
                }
                if (message!!.byteArrayLength > it.maxSize) return error("NetworkError", "too-small")
                it.writeNdefMessage(message)
            }
            return serial
        } catch (e: android.nfc.TagLostException) {
            return error("NetworkError", "lost")
        } catch (e: IOException) {
            return error("NetworkError", "failed")
        } catch (e: FormatException) {
            return error("NetworkError", "failed")
        }
    }

    /** `transceive` (D12) to a tag a live scan of [owner] holds. */
    fun transceive(owner: NfcModule, args: JSONObject, reply: (JSONObject) -> Unit) {
        val tag = owner.leases[args.optString("tag")] ?: return reply(error("InvalidStateError", "the scan that read it has ended"))
        val data = Records.bytes(args.opt("data")) ?: return reply(error("TypeError", "data"))
        val tech: TagTechnology = when (args.optString("technology")) {
            "IsoDep" -> IsoDep.get(tag)?.also { t -> if (args.has("timeout")) t.timeout = args.getInt("timeout") }
            "NfcA" -> NfcA.get(tag); "NfcB" -> NfcB.get(tag); "NfcF" -> NfcF.get(tag); "NfcV" -> NfcV.get(tag)
            "MifareUltralight" -> MifareUltralight.get(tag); "MifareClassic" -> MifareClassic.get(tag)
            else -> null
        } ?: return reply(error("NotSupportedError", "this tag has no ${args.optString("technology")}"))
        Thread {
            val answer = try {
                tech.use { t ->
                    t.connect()
                    val out = when (t) {
                        is IsoDep -> t.transceive(data); is NfcA -> t.transceive(data); is NfcB -> t.transceive(data)
                        is NfcF -> t.transceive(data); is NfcV -> t.transceive(data); is MifareUltralight -> t.transceive(data)
                        is MifareClassic -> t.transceive(data); else -> ByteArray(0)
                    }
                    JSONObject().put("data", Records.wire(out)).put("sw1", JSONObject.NULL).put("sw2", JSONObject.NULL)
                }
            } catch (e: android.nfc.TagLostException) { error("NetworkError", "lost") } catch (e: IOException) { error("NetworkError", e.message) }
            main.post { reply(answer) }
        }.start()
    }

    private fun <T> onMain(f: () -> T): T {
        if (Looper.myLooper() == Looper.getMainLooper()) return f()
        val done = java.util.concurrent.CountDownLatch(1)
        var out: T? = null
        main.post { out = f(); done.countDown() }
        done.await()
        @Suppress("UNCHECKED_CAST") return out as T
    }
}

/** Web NFC's records over Android's `NdefRecord` ("Mapping NDEF record to NDEFRecord"). */
internal object Records {
    private val prefixes = arrayOf("", "http://www.", "https://www.", "http://", "https://", "tel:", "mailto:", "ftp://anonymous:anonymous@",
        "ftp://ftp.", "ftps://", "sftp://", "smb://", "nfs://", "ftp://", "dav://", "news:", "telnet://", "imap:", "rtsp://", "urn:", "pop:",
        "sip:", "sips:", "tftp:", "btspp://", "btl2cap://", "btgoep://", "tcpobex://", "irdaobex://", "file://", "urn:epc:id:", "urn:epc:tag:",
        "urn:epc:pat:", "urn:epc:raw:", "urn:epc:", "urn:nfc:")

    fun hex(bytes: ByteArray, separator: String) = bytes.joinToString(separator) { "%02x".format(it) }

    fun wire(bytes: ByteArray): JSONObject = JSONObject().put("base64", Base64.encodeToString(bytes, Base64.NO_WRAP))

    fun bytes(value: Any?): ByteArray? = when (value) {
        is String -> value.toByteArray()
        is JSONObject -> value.optString("base64", "").let { Base64.decode(it, Base64.DEFAULT) }
        else -> null
    }

    fun json(r: NdefRecord, full: Boolean): JSONObject {
        val type = String(r.type ?: ByteArray(0))
        var data = r.payload ?: ByteArray(0)
        var text: String? = null
        var encoding: String? = null
        var lang: String? = null
        var inner: JSONArray? = null
        val (recordType, mediaType) = when (r.tnf) {
            NdefRecord.TNF_EMPTY -> "empty" to null
            NdefRecord.TNF_WELL_KNOWN -> when (type) { "T" -> "text" to null; "U" -> "url" to null; "Sp" -> "smart-poster" to null; else -> ":$type" to null }
            NdefRecord.TNF_MIME_MEDIA -> "mime" to type
            NdefRecord.TNF_ABSOLUTE_URI -> "absolute-url" to null
            NdefRecord.TNF_EXTERNAL_TYPE -> type to null
            else -> "unknown" to null
        }
        when (recordType) {
            "url" -> { val at = data.firstOrNull()?.toInt()?.and(0xff) ?: 0; data = (prefixes.getOrElse(at) { "" }).toByteArray() + data.drop(1).toByteArray(); text = String(data) }
            "absolute-url" -> text = type
            "text" -> {
                val status = data.firstOrNull()?.toInt() ?: 0
                val n = status and 0x3f
                encoding = if (status and 0x80 == 0) "utf-8" else "utf-16be"
                lang = String(data, 1, minOf(n, data.size - 1))
                data = data.copyOfRange(minOf(1 + n, data.size), data.size)
                text = String(data, if (encoding == "utf-8") Charsets.UTF_8 else Charsets.UTF_16BE)
            }
            "smart-poster" -> {
                val records = try { NdefMessage(data).records } catch (e: FormatException) { emptyArray() }
                // One level deep (`NDEFInnerRecord`): a deeper record is in its parent's bytes.
                inner = JSONArray(records.map { json(it, full).apply { remove("records") } })
                text = (0 until inner.length()).map { inner.getJSONObject(it) }.firstOrNull { it.optString("recordType") == "url" }?.optString("text")
            }
        }
        val out = JSONObject().put("recordType", recordType).put("mediaType", mediaType ?: JSONObject.NULL)
            .put("id", r.id?.takeIf { it.isNotEmpty() }?.let { String(it) } ?: JSONObject.NULL)
            .put("encoding", encoding ?: JSONObject.NULL).put("lang", lang ?: JSONObject.NULL)
            .put("text", text ?: JSONObject.NULL).put("size", data.size)
        if (full) out.put("data", wire(data)).put("records", inner ?: JSONObject.NULL)
        return out
    }

    /** An `NDEFRecordInit` as a record; `IllegalArgumentException` for one that cannot be encoded (`TypeError`). */
    fun init(r: JSONObject, fallback: String): NdefRecord {
        val recordType = r.optString("recordType")
        val id = r.optString("id", "").toByteArray()
        val data = bytes(r.opt("data"))
        return when (recordType) {
            "empty" -> NdefRecord(NdefRecord.TNF_EMPTY, null, null, null)
            "text" -> {
                val lang = r.optString("lang", fallback).toByteArray()
                NdefRecord(NdefRecord.TNF_WELL_KNOWN, NdefRecord.RTD_TEXT, id, byteArrayOf(lang.size.toByte()) + lang + (data ?: throw IllegalArgumentException("a text record has data")))
            }
            "url" -> {
                val url = data?.let { String(it) }?.takeIf { ':' in it } ?: throw IllegalArgumentException("a url record's data is a URL")
                val at = prefixes.indices.drop(1).filter { url.startsWith(prefixes[it]) }.maxByOrNull { prefixes[it].length } ?: 0
                NdefRecord(NdefRecord.TNF_WELL_KNOWN, NdefRecord.RTD_URI, id, byteArrayOf(at.toByte()) + url.substring(prefixes[at].length).toByteArray())
            }
            "absolute-url" -> NdefRecord(NdefRecord.TNF_ABSOLUTE_URI, (data?.let { String(it) }?.takeIf { ':' in it } ?: throw IllegalArgumentException("an absolute-url record's data is a URL")).toByteArray(), id, ByteArray(0))
            "mime" -> NdefRecord(NdefRecord.TNF_MIME_MEDIA, r.optString("mediaType", "application/octet-stream").toByteArray(), id, data ?: throw IllegalArgumentException("a mime record has data"))
            "unknown" -> NdefRecord(NdefRecord.TNF_UNKNOWN, null, id, data ?: ByteArray(0))
            else -> if (':' in recordType && data != null) NdefRecord(NdefRecord.TNF_EXTERNAL_TYPE, recordType.toByteArray(), id, data)
                else throw IllegalArgumentException("`$recordType` is no record type this module writes")
        }
    }
}
