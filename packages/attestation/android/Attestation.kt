// @exact/attestation on Android (LLP 1133.028 D1, D4): Play Integrity, for the app's own server. Under the agent it
// asks Play for nothing and answers `exact-substitute` tokens over D3's per-account lifecycle (AgentAttestation),
// as the Apple implementation does; `--bind attestation` asks Play.
//
// - `clientData` is hashed here, SHA-256 of its UTF-8 bytes, and its base64url form (43 characters) is a standard
//   request's `requestHash` or a classic request's `nonce` (D1), so a server hashes the same bytes on every platform.
// - One `StandardIntegrityTokenProvider` per cloud project number: `prepareAttestation` makes it, `attest` makes it
//   when none is held, and again once when a request fails `INTEGRITY_TOKEN_PROVIDER_INVALID` (D4).
// - `android.cloudProjectNumber` is required on Android (D4): a call without it fails `TypeError`.
// - Play has no per-account key: `account` is ignored and `resetAttestation` answers at once.
package exact.attestation

import android.util.Base64
import com.google.android.gms.tasks.Task
import com.google.android.play.core.integrity.IntegrityManagerFactory
import com.google.android.play.core.integrity.IntegrityServiceException
import com.google.android.play.core.integrity.IntegrityTokenRequest
import com.google.android.play.core.integrity.StandardIntegrityException
import com.google.android.play.core.integrity.StandardIntegrityManager.PrepareIntegrityTokenRequest
import com.google.android.play.core.integrity.StandardIntegrityManager.StandardIntegrityTokenProvider
import com.google.android.play.core.integrity.StandardIntegrityManager.StandardIntegrityTokenRequest
import com.google.android.play.core.integrity.model.IntegrityErrorCode
import com.google.android.play.core.integrity.model.StandardIntegrityErrorCode
import dev.exact.canvas.ExactExtension
import dev.exact.canvas.ExtensionContext
import org.json.JSONObject
import java.security.MessageDigest

class AttestationModule(context: ExtensionContext) : ExactExtension(context) {
    companion object {
        fun base64url(bytes: ByteArray): String = Base64.encodeToString(bytes, Base64.URL_SAFE or Base64.NO_PADDING or Base64.NO_WRAP)
        fun hash(clientData: String): ByteArray = MessageDigest.getInstance("SHA-256").digest(clientData.toByteArray(Charsets.UTF_8))
        fun error(name: String, message: String, code: Int? = null): JSONObject = JSONObject().put("error", name).put("message", message)
            .put("android", if (code == null) JSONObject.NULL else JSONObject().put("code", code))
    }

    private val agent by lazy { AgentAttestation(context) }
    private val standard by lazy { IntegrityManagerFactory.createStandard(context.application) }
    private val classic by lazy { IntegrityManagerFactory.create(context.application) }
    /** The token provider of each cloud project number. */
    private val providers = HashMap<Long, StandardIntegrityTokenProvider>()

    private fun say(line: String) = context.log("attestation: $line")

    override fun later(name: String, args: JSONObject, reply: (JSONObject) -> Unit) = later(name, args, null, reply)

    override fun later(name: String, args: JSONObject, preferred: Any?, reply: (JSONObject) -> Unit) {
        val account = args.optString("account", "")
        when (name) {
            "attestationStatus" -> reply(if (context.agent) agent.status() else status())
            "attest" -> {
                val clientData = args.optString("clientData", "")
                if (context.agent) return agent.attest(account, hash(clientData), reply)
                val project = projectOf(args) ?: return reply(error("TypeError", "android.cloudProjectNumber is required on Android"))
                val classicRequest = args.optJSONObject("android")?.optString("request") == "classic"
                if (classicRequest) requestClassic(project, base64url(hash(clientData)), reply)
                else requestStandard(project, base64url(hash(clientData)), retried = false, reply)
            }
            "prepareAttestation" -> {
                if (context.agent) return agent.prepare(account, reply)
                val project = projectOf(args) ?: return reply(error("TypeError", "android.cloudProjectNumber is required on Android"))
                provider(project, { reply(JSONObject()) }, reply)
            }
            "resetAttestation" -> {
                if (context.agent) agent.reset(account)
                reply(JSONObject())
            }
            else -> reply(error("NotSupportedError", "attestation has no $name"))
        }
    }

    override fun preferred(values: JSONObject) = agent.preferred(values)

    /** A dropped attestation's key (its `attested` handle): no server saw it. Play has none outside the agent. */
    override fun dispose(ids: List<String>) {
        if (!context.agent) return
        ids.forEach { agent.discard(it); say("disposed a key its reply never reached") }
    }

    private fun projectOf(args: JSONObject): Long? = args.optJSONObject("android")?.optString("cloudProjectNumber")?.toLongOrNull()

    /** Play's own reading is per request; the status says whether Play services and the Play Store are installed. */
    private fun status(): JSONObject {
        val pm = context.application.packageManager
        val installed = { pkg: String -> try { pm.getPackageInfo(pkg, 0); true } catch (e: Exception) { false } }
        val available = installed("com.android.vending") && installed("com.google.android.gms")
        return JSONObject().put("available", available).put("provider", if (available) "play-integrity" else "none").put("apple", JSONObject.NULL)
    }

    private fun provider(project: Long, ready: (StandardIntegrityTokenProvider) -> Unit, failed: (JSONObject) -> Unit) {
        providers[project]?.let { return ready(it) }
        val request = PrepareIntegrityTokenRequest.builder().setCloudProjectNumber(project).build()
        settle(standard.prepareIntegrityToken(request), { provider ->
            providers[project] = provider
            say("prepared a token provider")
            ready(provider)
        }, failed)
    }

    private fun requestStandard(project: Long, requestHash: String, retried: Boolean, reply: (JSONObject) -> Unit) {
        provider(project, { provider ->
            val request = StandardIntegrityTokenRequest.builder().setRequestHash(requestHash).build()
            settle(provider.request(request), { token -> reply(answer(token.token())) }) { failure ->
                // Google: prepare again when the provider is no longer valid (D4).
                if (!retried && failure.optJSONObject("android")?.optInt("code") == StandardIntegrityErrorCode.INTEGRITY_TOKEN_PROVIDER_INVALID) {
                    providers.remove(project)
                    requestStandard(project, requestHash, retried = true, reply)
                } else reply(failure)
            }
        }, reply)
    }

    private fun requestClassic(project: Long, nonce: String, reply: (JSONObject) -> Unit) {
        val request = IntegrityTokenRequest.builder().setCloudProjectNumber(project).setNonce(nonce).build()
        settle(classic.requestIntegrityToken(request), { response -> reply(answer(response.token())) }, reply)
    }

    private fun answer(token: String): JSONObject =
        JSONObject().put("format", "play-integrity").put("token", token).put("keyId", JSONObject.NULL).put("attested", JSONObject.NULL)

    private fun <T> settle(task: Task<T>, done: (T) -> Unit, failed: (JSONObject) -> Unit) {
        task.addOnSuccessListener { done(it) }.addOnFailureListener { failed(failure(it)) }
    }

    /** D1's names for Play's codes, the code beside. */
    private fun failure(e: Exception): JSONObject {
        val code = when (e) {
            is StandardIntegrityException -> e.errorCode
            is IntegrityServiceException -> e.errorCode
            else -> return error("OperationError", e.message ?: "Play Integrity failed")
        }
        // The standard and classic codes share their values.
        val name = when (code) {
            IntegrityErrorCode.API_NOT_AVAILABLE, IntegrityErrorCode.PLAY_STORE_NOT_FOUND, IntegrityErrorCode.PLAY_SERVICES_NOT_FOUND,
            IntegrityErrorCode.PLAY_STORE_VERSION_OUTDATED, IntegrityErrorCode.PLAY_SERVICES_VERSION_OUTDATED -> "NotSupportedError"
            IntegrityErrorCode.NETWORK_ERROR, IntegrityErrorCode.GOOGLE_SERVER_UNAVAILABLE -> "NetworkError"
            IntegrityErrorCode.TOO_MANY_REQUESTS -> "QuotaExceededError"
            IntegrityErrorCode.CLOUD_PROJECT_NUMBER_IS_INVALID, IntegrityErrorCode.APP_UID_MISMATCH,
            IntegrityErrorCode.APP_NOT_INSTALLED -> "InvalidStateError"
            else -> "OperationError"
        }
        return error(name, e.message ?: name, code)
    }
}
