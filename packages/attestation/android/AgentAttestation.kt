// @exact/attestation's agent path on Android (LLP 1133.028 §7), the Apple one's (AgentAttestation.swift): D3's
// lifecycle over synthetic keys in the session's memory, with no call to Play. A token is `exact-substitute`, the
// base64url of a JSON object naming the client data's SHA-256, the key id, the kind and the counter, which
// `packages/attestation/test/verify.mjs` checks. The agent's words are Apple's: the status overlay, `fail` (the next
// call fails with that name; a `NetworkError` keeps the key) and `delay` (each answer waits that long on the
// agent's clock).
package exact.attestation

import android.os.Handler
import android.os.Looper
import dev.exact.canvas.ExtensionContext
import org.json.JSONObject
import java.security.SecureRandom

internal class AgentAttestation(private val context: ExtensionContext) {
    private class Key(val id: String, var attested: Boolean, var counter: Int)
    private val keys = HashMap<String, Key>()
    private var overlay = JSONObject()
    private var fail: String? = null
    private var lastFail: String? = null
    private var delay = 0.0
    private val main = Handler(Looper.getMainLooper())

    private fun say(line: String) = context.log("attestation: $line")

    fun preferred(values: JSONObject) {
        overlay = values.optJSONObject("attestationStatus") ?: JSONObject()
        val word = values.optString("fail", "").takeIf { values.has("fail") && !values.isNull("fail") }
        // A new word arms one failure; the same word again does not.
        if (!word.isNullOrEmpty() && word != lastFail) fail = word
        lastFail = word
        delay = values.optDouble("delay", 0.0).takeIf { !it.isNaN() } ?: 0.0
    }

    fun status(): JSONObject {
        val out = JSONObject().put("available", true).put("provider", "play-integrity").put("apple", JSONObject.NULL)
        overlay.keys().forEach { out.put(it, overlay.get(it)) }
        if (!out.optBoolean("available", false)) out.put("provider", "none")
        return out
    }

    private val available get() = status().optBoolean("available", false)

    fun prepare(account: String, reply: (JSONObject) -> Unit) {
        if (!available) return later(reply, AttestationModule.error("NotSupportedError", "attestation is unavailable"))
        takeFailure()?.let { return later(reply, it) }
        keys.getOrPut(account) { Key(newId(), false, 0) }
        later(reply, JSONObject())
    }

    fun attest(account: String, hash: ByteArray, reply: (JSONObject) -> Unit) {
        if (!available) return later(reply, AttestationModule.error("NotSupportedError", "attestation is unavailable"))
        val key = keys.getOrPut(account) { Key(newId(), false, 0) }
        takeFailure()?.let { failed ->
            // As D3 says of Apple's errors: a network failure keeps the key, any other discards one not yet attested.
            if (failed.optString("error") != "NetworkError" && !key.attested) keys.remove(account)
            say("failed ${failed.optString("error")}")
            return later(reply, failed)
        }
        val attesting = !key.attested
        if (!attesting) key.counter += 1
        key.attested = true
        // Keys sorted, as the Apple path writes them.
        val token = JSONObject().put("clientDataHash", AttestationModule.base64url(hash)).put("counter", key.counter)
            .put("keyId", key.id).put("kind", if (attesting) "attestation" else "assertion")
        say(if (attesting) "attested a new key" else "asserted (counter ${key.counter})")
        later(reply, JSONObject().put("format", "exact-substitute").put("token", AttestationModule.base64url(token.toString().toByteArray()))
            .put("keyId", key.id).put("attested", if (attesting) key.id else JSONObject.NULL))
    }

    fun reset(account: String) { keys.remove(account) }

    fun discard(keyId: String) { keys.entries.removeAll { it.value.id == keyId } }

    private fun takeFailure(): JSONObject? {
        val name = fail ?: return null
        fail = null
        return AttestationModule.error(name, "the agent's failure")
    }

    /** [answer] after [delay] ms of the agent's clock, polled, since nothing tells an implementation that it moved. */
    private fun later(reply: (JSONObject) -> Unit, answer: JSONObject) {
        if (delay <= 0) return reply(answer)
        val due = context.now() + delay
        val wait = object : Runnable {
            override fun run() { if (context.now() >= due) reply(answer) else main.postDelayed(this, 50) }
        }
        wait.run()
    }

    private fun newId(): String = android.util.Base64.encodeToString(ByteArray(32).also { SecureRandom().nextBytes(it) }, android.util.Base64.NO_WRAP)
}
