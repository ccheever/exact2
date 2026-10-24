// @exact/passkeys on Android (LLP 1133.011 D1, D3-D6, D10, D11, D13): Credential Manager (`androidx.credentials`
// 1.6, in the app's one locked Gradle graph) over the resumed activity, in WebAuthn's JSON forms, which Credential
// Manager takes and answers as they are. The build makes one per host when the app binds the extension, outside the
// agent; under the agent the runner's substitute answers and none is made.
//
// - Credential Manager writes the response's origin itself, `android:apk-key-hash:<the signing certificate's
//   SHA-256>`, which the relying party must accept (D6).
// - Conditional `get` (D3, API 35): the request is set on each field the reader made whose `autocomplete` ends in
//   `webauthn` (`webauthnField`, D10 item 4), and the field's suggestions offer the passkeys; below API 35, or with
//   no such field, the capability reads false and the call fails `NotSupportedError`.
// - No answer carries a PRF output (D10): `clientExtensionResults.prf.results` is removed. A lock's user handle (§7
//   item 3, beginning `exact-lock:`) is never registered, delivered or signalled from here.
// - One ceremony at a time (D9): a newer one ends a pending conditional `get` with `AbortError`.
// - The hatches (D5) are given each request and return a replacement, since both are immutable; one that changes the
//   options' JSON, which the module owns, is not used, and a development build says so.
package exact.passkeys

import android.os.Build
import android.os.CancellationSignal
import android.util.Base64
import android.widget.EditText
import androidx.credentials.CreateCredentialResponse
import androidx.credentials.CreatePublicKeyCredentialRequest
import androidx.credentials.CreatePublicKeyCredentialResponse
import androidx.credentials.CredentialManager
import androidx.credentials.CredentialManagerCallback
import androidx.credentials.GetCredentialRequest
import androidx.credentials.GetCredentialResponse
import androidx.credentials.GetPublicKeyCredentialOption
import androidx.credentials.PendingGetCredentialRequest
import androidx.credentials.PublicKeyCredential
import androidx.credentials.SignalAllAcceptedCredentialIdsRequest
import androidx.credentials.SignalCredentialStateRequest
import androidx.credentials.SignalCredentialStateResponse
import androidx.credentials.SignalCurrentUserDetailsRequest
import androidx.credentials.SignalUnknownCredentialRequest
import androidx.credentials.exceptions.CreateCredentialCancellationException
import androidx.credentials.exceptions.CreateCredentialException
import androidx.credentials.exceptions.CreateCredentialProviderConfigurationException
import androidx.credentials.exceptions.GetCredentialCancellationException
import androidx.credentials.exceptions.GetCredentialException
import androidx.credentials.exceptions.GetCredentialProviderConfigurationException
import androidx.credentials.exceptions.NoCredentialException
import androidx.credentials.exceptions.publickeycredential.CreatePublicKeyCredentialDomException
import androidx.credentials.exceptions.publickeycredential.GetPublicKeyCredentialDomException
import androidx.credentials.exceptions.publickeycredential.SignalCredentialStateException
import androidx.credentials.pendingGetCredentialRequest
import dev.exact.canvas.ExactExtension
import dev.exact.canvas.ExtensionContext
import org.json.JSONObject

class PasskeysModule(context: ExtensionContext) : ExactExtension(context) {
    companion object {
        /** The app's hatches (D5): each request, after the module's words are set; the replacement is used. */
        @JvmStatic var createRequest: ((CreatePublicKeyCredentialRequest) -> CreatePublicKeyCredentialRequest)? = null
        @JvmStatic var getRequest: ((GetCredentialRequest) -> GetCredentialRequest)? = null
        private val MARK = "exact-lock:".toByteArray()
    }

    private val manager by lazy { CredentialManager.create(context.application) }
    /** A pending conditional `get`: the fields it is set on, and how it ends. */
    private var conditional: Pair<PendingGetCredentialRequest, (JSONObject) -> Unit>? = null
    private val fields = LinkedHashSet<EditText>()
    /** No `*ProviderConfigurationException` on the last call (D4's `android.providerConfigured`). */
    private var configured = true
    /** Each registration's RP ID by credential id, for `dispose` (D11). */
    private val made = HashMap<String, String>()
    private val signals = mutableSetOf<CancellationSignal>()

    override fun later(name: String, args: JSONObject, reply: (JSONObject) -> Unit) {
        when (name) {
            "getClientCapabilities" -> reply(capabilities())
            "create" -> create(args, reply)
            "get" -> get(args, reply)
            "signalUnknownCredential" -> signal(SignalUnknownCredentialRequest(args.toString()), args, reply)
            "signalAllAcceptedCredentials" -> signal(SignalAllAcceptedCredentialIdsRequest(args.toString()), args, reply)
            "signalCurrentUserDetails" -> signal(SignalCurrentUserDetailsRequest(args.toString()), args, reply)
            else -> reply(JSONObject().put("error", "NotSupportedError"))
        }
    }

    /** A provider can be set up in Settings while the app is away. */
    override fun resumed() = context.changed("passkeys.getClientCapabilities")

    override fun webauthnField(view: EditText, present: Boolean) {
        if (present) fields.add(view) else fields.remove(view)
        val pending = conditional?.first
        if (Build.VERSION.SDK_INT >= 35 && pending != null) view.pendingGetCredentialRequest = if (present) pending else null
        if (Build.VERSION.SDK_INT >= 35) context.changed("passkeys.getClientCapabilities")
    }

    override fun dispose(ids: List<String>) {
        for (id in ids) {
            val rp = made.remove(id) ?: continue
            if (Build.VERSION.SDK_INT < 35) { context.log("dispose passkeys $id: an orphaned passkey for $rp; this Android has no signal for it"); continue }
            signal(SignalUnknownCredentialRequest(JSONObject().put("rpId", rp).put("credentialId", id).toString()), JSONObject()) {
                context.log("dispose passkeys $id: ${it.optString("error").ifEmpty { "signalled unknown" }}")
            }
        }
    }

    override fun destroy(done: () -> Unit) {
        signals.toList().forEach { it.cancel() }
        endConditional("the session ended")
        done()
    }

    // Capabilities (D13)

    private fun capabilities(): JSONObject {
        val newer = Build.VERSION.SDK_INT >= 35
        val keyguard = context.application.getSystemService(android.app.KeyguardManager::class.java)
        return JSONObject()
            .put("available", true)
            .put("conditionalCreate", false)
            .put("conditionalGet", newer && fields.isNotEmpty())
            .put("hybridTransport", true)
            .put("passkeyPlatformAuthenticator", true)
            .put("userVerifyingPlatformAuthenticator", keyguard?.isDeviceSecure == true)
            .put("relatedOrigins", false)
            .put("signalAllAcceptedCredentials", newer)
            .put("signalCurrentUserDetails", newer)
            .put("signalUnknownCredential", newer)
            .put("preferImmediatelyAvailableCredentials", true)
            // Which providers evaluate an extension, Credential Manager does not say (D13).
            .put("extensions", JSONObject().put("prf", JSONObject.NULL).put("largeBlob", JSONObject.NULL).put("credProps", JSONObject.NULL))
            .put("android", JSONObject().put("providerConfigured", configured))
    }

    // Ceremonies

    private fun refused(reply: (JSONObject) -> Unit): Boolean {
        if (context.origin != "app") { reply(JSONObject().put("error", "NotAllowedError").put("message", "person-required")); return true }
        if (context.activity == null || !context.resumed) { reply(JSONObject().put("error", "NotAllowedError").put("message", "the app is not in front")); return true }
        return false
    }

    private fun create(args: JSONObject, reply: (JSONObject) -> Unit) {
        if (refused(reply)) return
        val options = args.optJSONObject("publicKey") ?: return reply(JSONObject().put("error", "TypeError"))
        if (marked(options.optJSONObject("user")?.optString("id"))) return reply(JSONObject().put("error", "NotAllowedError").put("message", "a lock's user handle is the lock's alone"))
        if (args.optString("mediation") == "conditional") return reply(JSONObject().put("error", "NotSupportedError").put("message", "conditional create is not built on Android"))
        endConditional("a newer ceremony replaced this conditional one")
        val json = options.toString()
        val ours = CreatePublicKeyCredentialRequest(json)
        val request = createRequest?.invoke(ours)?.let { if (it.requestJson == json) it else { context.log("passkeys: the create hatch changed the options' JSON, which the module owns; not used (LLP 1133.011 D5)"); ours } } ?: ours
        val signal = CancellationSignal().also { signals.add(it) }
        val activity = context.activity!!
        manager.createCredentialAsync(activity, request, signal, activity.mainExecutor, object : CredentialManagerCallback<CreateCredentialResponse, CreateCredentialException> {
            override fun onResult(result: CreateCredentialResponse) {
                signals.remove(signal)
                configured = true
                val made = (result as? CreatePublicKeyCredentialResponse)?.registrationResponseJson ?: return reply(JSONObject().put("error", "UnknownError"))
                val answer = withheld(JSONObject(made))
                options.optJSONObject("rp")?.optString("id")?.let { this@PasskeysModule.made[answer.optString("id")] = it }
                reply(answer)
            }
            override fun onError(e: CreateCredentialException) {
                signals.remove(signal)
                reply(failure(e, e.type, e.errorMessage, if (e is CreatePublicKeyCredentialDomException) e.domError.type else null,
                    e is CreateCredentialCancellationException, e is CreateCredentialProviderConfigurationException))
            }
        })
    }

    private fun get(args: JSONObject, reply: (JSONObject) -> Unit) {
        if (refused(reply)) return
        val options = args.optJSONObject("publicKey") ?: return reply(JSONObject().put("error", "TypeError"))
        val json = options.toString()
        val conditionally = args.optString("mediation") == "conditional"
        if (conditionally && (Build.VERSION.SDK_INT < 35 || fields.isEmpty()))
            return reply(JSONObject().put("error", "NotSupportedError").put("message", "conditional get needs Android 15 and a field whose autocomplete ends in webauthn"))
        endConditional("a newer ceremony replaced this conditional one")
        val discoverable = (options.optJSONArray("allowCredentials")?.length() ?: 0) == 0
        val ours = GetCredentialRequest.Builder()
            .addCredentialOption(GetPublicKeyCredentialOption(json))
            .setPreferImmediatelyAvailableCredentials(args.optBoolean("preferImmediatelyAvailableCredentials"))
            .build()
        val request = getRequest?.invoke(ours)?.let { r ->
            val same = r.credentialOptions.size == 1 && (r.credentialOptions[0] as? GetPublicKeyCredentialOption)?.requestJson == json
            if (same) r else { context.log("passkeys: the get hatch changed the credential options, which the module owns; not used (LLP 1133.011 D5)"); ours }
        } ?: ours
        val answered = { result: GetCredentialResponse ->
            configured = true
            val credential = result.credential as? PublicKeyCredential
            if (credential == null) reply(JSONObject().put("error", "UnknownError"))
            else {
                val answer = withheld(JSONObject(credential.authenticationResponseJson))
                val handle = answer.optJSONObject("response")?.optString("userHandle")
                // A lock's assertion, or a discoverable one with no user handle, is never delivered (§7 item 3).
                if (marked(handle) || (discoverable && handle.isNullOrEmpty())) reply(JSONObject().put("error", "NotAllowedError")) else reply(answer)
            }
        }
        if (conditionally && Build.VERSION.SDK_INT >= 35) {
            val pending = PendingGetCredentialRequest(request) { result -> endConditional(null); answered(result) }
            conditional = pending to reply
            fields.forEach { it.pendingGetCredentialRequest = pending }
            return
        }
        val signal = CancellationSignal().also { signals.add(it) }
        val activity = context.activity!!
        manager.getCredentialAsync(activity, request, signal, activity.mainExecutor, object : CredentialManagerCallback<GetCredentialResponse, GetCredentialException> {
            override fun onResult(result: GetCredentialResponse) { signals.remove(signal); answered(result) }
            override fun onError(e: GetCredentialException) {
                signals.remove(signal)
                reply(failure(e, e.type, e.errorMessage, if (e is GetPublicKeyCredentialDomException) e.domError.type else null,
                    e is GetCredentialCancellationException || e is NoCredentialException, e is GetCredentialProviderConfigurationException))
            }
        })
    }

    /** The pending conditional `get` ends: with `AbortError` and [why], or answered already (null). */
    private fun endConditional(why: String?) {
        val (_, reply) = conditional ?: return
        conditional = null
        if (Build.VERSION.SDK_INT >= 35) fields.forEach { it.pendingGetCredentialRequest = null }
        if (why != null) reply(JSONObject().put("error", "AbortError").put("message", why))
    }

    private fun signal(request: SignalCredentialStateRequest, args: JSONObject, reply: (JSONObject) -> Unit) {
        if (marked(args.optString("userId"))) return reply(JSONObject().put("error", "NotAllowedError"))
        // Where Android has no signal, there is nothing to tell (D13).
        if (Build.VERSION.SDK_INT < 35) return reply(JSONObject())
        manager.signalCredentialStateAsync(request, context.application.mainExecutor, object : CredentialManagerCallback<SignalCredentialStateResponse, SignalCredentialStateException> {
            override fun onResult(result: SignalCredentialStateResponse) = reply(JSONObject())
            override fun onError(e: SignalCredentialStateException) = reply(JSONObject().put("error", "UnknownError").put("message", e.message ?: e.type)
                .put("android", JSONObject().put("type", e.type)))
        })
    }

    /** D1's code for a Credential Manager exception, with its own type beside it (D4). */
    private fun failure(e: Exception, type: String, message: CharSequence?, dom: String?, cancelled: Boolean, unconfigured: Boolean): JSONObject {
        if (unconfigured) configured = false
        val name = when {
            // `androidx.credentials.TYPE_INVALID_STATE_ERROR` is `InvalidStateError`.
            dom != null -> dom.substringAfterLast("TYPE_").split('_').joinToString("") { it.lowercase().replaceFirstChar(Char::uppercase) }
            cancelled -> "NotAllowedError"
            unconfigured -> "NotSupportedError"
            else -> "NotAllowedError"
        }
        val android = JSONObject().put("type", if (dom != null) "$type/$dom" else type)
        return JSONObject().put("error", name).put("message", "${message ?: e.javaClass.simpleName} (${e.javaClass.simpleName})").put("android", android)
    }

    /** No PRF output leaves the implementation (D10). */
    private fun withheld(answer: JSONObject): JSONObject {
        answer.optJSONObject("clientExtensionResults")?.optJSONObject("prf")?.remove("results")
        return answer
    }

    private fun marked(handle: String?): Boolean {
        if (handle.isNullOrEmpty()) return false
        val bytes = try { Base64.decode(handle, Base64.URL_SAFE or Base64.NO_PADDING or Base64.NO_WRAP) } catch (e: IllegalArgumentException) { return false }
        return bytes.size >= MARK.size && MARK.indices.all { bytes[it] == MARK[it] }
    }
}
