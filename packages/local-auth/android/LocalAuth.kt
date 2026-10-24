// @exact/local-auth on Android (LLP 1133.010 D1, D3, D8, D10-D13): the platform's BiometricPrompt and
// BiometricManager, not androidx, answered in the interface's shapes. The build makes one per host when the app binds
// the extension, outside the agent; under the agent the runner's substitute answers and none is made, unless the
// drive binds it (`--bind local-auth`).
//
// - `authenticate` allows BIOMETRIC_WEAK and the device credential unless the call says otherwise (D3): biometrics
//   alone for `biometricOnly`, with a Cancel button; `android.allowedAuthenticators` beats both. API 29 has no
//   allowed authenticators: it allows the credential with `setDeviceCredentialAllowed`.
// - The prompt shows only over a resumed activity (D8); a call while none is fails SYSTEM_CANCEL at once.
// - The status: no permission is asked on Android, so `state` is `granted`; `biometry` is `other` when a biometric of
//   the allowed class is enrolled, the platform naming no modality; its own button label is `android.label` (API 31).
// - The authenticator entry (D13): the reader's presence backend hands it the key's `CryptoObject` and the policy's
//   authenticators, and opens the item with what it returns.
// - Tier 3 (D11): `LocalAuthModule.hatch` is given each prompt's builder after the module's words are set, and
//   `keyHatch` each presence key's builder; what the module owns it sets again after them.
package exact.localauth

import android.content.Context
import android.hardware.biometrics.BiometricManager
import android.hardware.biometrics.BiometricManager.Authenticators
import android.hardware.biometrics.BiometricPrompt
import android.os.Build
import android.os.CancellationSignal
import android.security.keystore.KeyGenParameterSpec
import dev.exact.canvas.ExactAuthenticator
import dev.exact.canvas.ExactExtension
import dev.exact.canvas.ExtensionContext
import org.json.JSONObject

class LocalAuthModule(context: ExtensionContext) : ExactExtension(context), ExactAuthenticator {
    companion object {
        /** The app's prompt hatch (D11): each prompt's builder, after the module's words are set. */
        @JvmStatic var hatch: ((BiometricPrompt.Builder) -> Unit)? = null
        /** The app's key hatch (D11): a presence name's key builder, before generation. */
        @JvmStatic var keyHatch: ((KeyGenParameterSpec.Builder) -> Unit)? = null
        private val AUTHENTICATORS = mapOf(
            "BIOMETRIC_STRONG" to Authenticators.BIOMETRIC_STRONG,
            "BIOMETRIC_WEAK" to Authenticators.BIOMETRIC_WEAK,
            "DEVICE_CREDENTIAL" to Authenticators.DEVICE_CREDENTIAL,
        )
    }

    private val app: Context get() = context.application
    private val manager get() = app.getSystemService(BiometricManager::class.java)
    private val keyguard get() = app.getSystemService(android.app.KeyguardManager::class.java)
    /** Prompts up, cancelled if the session ends. */
    private val signals = mutableSetOf<CancellationSignal>()
    private var said: String? = null

    override fun later(name: String, args: JSONObject, reply: (JSONObject) -> Unit) {
        when (name) {
            "localAuthStatus" -> reply(status())
            "authenticate" -> authenticate(args, reply)
            else -> reply(JSONObject().put("error", "NotSupportedError"))
        }
    }

    /** The person can change the lock screen or enrol a finger while the app is away: read again on return. */
    override fun resumed() {
        val before = said ?: return
        if (status().toString() != before) context.changed("local-auth.localAuthStatus")
    }

    override fun destroy(done: () -> Unit) {
        signals.toList().forEach { it.cancel() }
        done()
    }

    // The status

    private fun can(authenticators: Int): Int =
        if (Build.VERSION.SDK_INT >= 30) manager.canAuthenticate(authenticators)
        else if (authenticators == Authenticators.DEVICE_CREDENTIAL) (if (keyguard.isDeviceSecure) BiometricManager.BIOMETRIC_SUCCESS else BiometricManager.BIOMETRIC_ERROR_NONE_ENROLLED)
        else @Suppress("DEPRECATION") manager.canAuthenticate()

    private fun status(): JSONObject {
        val weak = can(Authenticators.BIOMETRIC_WEAK)
        val credential = keyguard.isDeviceSecure
        val android = JSONObject().put("canAuthenticate", JSONObject()
            .put("biometricStrong", can(Authenticators.BIOMETRIC_STRONG))
            .put("biometricWeak", weak)
            .put("deviceCredential", can(Authenticators.DEVICE_CREDENTIAL)))
        if (Build.VERSION.SDK_INT >= 31) {
            val label = manager.getStrings(Authenticators.BIOMETRIC_WEAK or Authenticators.DEVICE_CREDENTIAL).buttonLabel
            android.put("label", label?.toString() ?: JSONObject.NULL)
        } else {
            android.put("label", JSONObject.NULL)
        }
        val out = JSONObject()
            .put("available", credential || weak == BiometricManager.BIOMETRIC_SUCCESS)
            .put("biometry", if (weak == BiometricManager.BIOMETRIC_SUCCESS) "other" else "none")
            .put("deviceCredential", credential)
            .put("state", "granted")
            // The runner lays its own over this: whether the host keeps locked secrets (D7, D15).
            .put("presenceSecrets", Build.VERSION.SDK_INT >= 30 && credential)
            .put("apple", JSONObject.NULL)
            .put("android", android)
        said = out.toString()
        return out
    }

    // The prompt

    /** D1's code for a BiometricPrompt error, with the platform's own beside it. */
    private fun failure(code: Int, message: CharSequence?): JSONObject {
        val word = when (code) {
            // 13: the negative button, which the framework names only in BiometricConstants (hidden).
            BiometricPrompt.BIOMETRIC_ERROR_USER_CANCELED, 13 -> "USER_CANCEL"
            BiometricPrompt.BIOMETRIC_ERROR_CANCELED, BiometricPrompt.BIOMETRIC_ERROR_TIMEOUT -> "SYSTEM_CANCEL"
            BiometricPrompt.BIOMETRIC_ERROR_LOCKOUT, BiometricPrompt.BIOMETRIC_ERROR_LOCKOUT_PERMANENT -> "LOCKOUT"
            BiometricPrompt.BIOMETRIC_ERROR_HW_UNAVAILABLE, BiometricPrompt.BIOMETRIC_ERROR_HW_NOT_PRESENT,
            BiometricPrompt.BIOMETRIC_ERROR_NO_BIOMETRICS, BiometricPrompt.BIOMETRIC_ERROR_NO_DEVICE_CREDENTIAL,
            BiometricPrompt.BIOMETRIC_ERROR_SECURITY_UPDATE_REQUIRED -> "NOT_AVAILABLE"
            else -> "AUTHENTICATION_FAILED"
        }
        return JSONObject().put("error", word).put("message", "${message ?: word} (BiometricPrompt $code)")
            .put("android", JSONObject().put("code", code))
    }

    /** A prompt over the resumed activity, with the module's words, the hatch, then what the module owns again. */
    private fun prompt(reason: String, authenticators: Int, words: JSONObject, done: (BiometricPrompt.AuthenticationResult?, JSONObject?) -> Unit,
                       crypto: BiometricPrompt.CryptoObject?) {
        val activity = context.activity
        if (activity == null || !context.resumed) return done(null, JSONObject().put("error", "SYSTEM_CANCEL").put("message", "the app is not in front"))
        val credential = authenticators and Authenticators.DEVICE_CREDENTIAL != 0
        val own = { b: BiometricPrompt.Builder ->
            b.setTitle(reason)
            if (Build.VERSION.SDK_INT >= 30) b.setAllowedAuthenticators(authenticators)
            else if (credential) @Suppress("DEPRECATION") b.setDeviceCredentialAllowed(true)
        }
        val builder = BiometricPrompt.Builder(activity)
        own(builder)
        words.optString("subtitle").takeIf { it.isNotEmpty() }?.let { builder.setSubtitle(it) }
        words.optString("description").takeIf { it.isNotEmpty() }?.let { builder.setDescription(it) }
        if (words.has("confirmationRequired")) builder.setConfirmationRequired(words.getBoolean("confirmationRequired"))
        val executor = activity.mainExecutor
        // Biometrics alone need a negative button; the platform's own Cancel unless the call names one.
        if (!credential) {
            val text = words.optString("negativeButtonText").ifEmpty { activity.getString(android.R.string.cancel) }
            builder.setNegativeButton(text, executor) { _, _ -> }
        }
        hatch?.let { it(builder); own(builder) }
        val signal = CancellationSignal()
        signals += signal
        val callback = object : BiometricPrompt.AuthenticationCallback() {
            override fun onAuthenticationSucceeded(result: BiometricPrompt.AuthenticationResult) { signals -= signal; done(result, null) }
            override fun onAuthenticationError(code: Int, message: CharSequence?) { signals -= signal; done(null, failure(code, message)) }
        }
        val built = builder.build()
        if (crypto != null) built.authenticate(crypto, signal, executor, callback) else built.authenticate(signal, executor, callback)
    }

    private fun authenticate(args: JSONObject, reply: (JSONObject) -> Unit) {
        // Presence is not delegable (D12); the runner refuses first, and this holds without it.
        if (context.origin != "app") return reply(JSONObject().put("error", "PERMISSION_DENIED").put("message", "person-required"))
        val words = args.optJSONObject("android") ?: JSONObject()
        val named = words.optJSONArray("allowedAuthenticators")
        val authenticators = when {
            named != null && Build.VERSION.SDK_INT >= 30 -> (0 until named.length()).fold(0) { all, i -> all or (AUTHENTICATORS[named.optString(i)] ?: 0) }
            args.optBoolean("biometricOnly") -> Authenticators.BIOMETRIC_WEAK
            else -> Authenticators.BIOMETRIC_WEAK or Authenticators.DEVICE_CREDENTIAL
        }
        prompt(args.optString("reason"), authenticators, words, { result, error ->
            if (error != null) return@prompt reply(error)
            val out = JSONObject().put("timestamp", System.currentTimeMillis().toDouble())
            if (Build.VERSION.SDK_INT >= 30 && result != null) {
                val type = if (result.authenticationType == BiometricPrompt.AUTHENTICATION_RESULT_TYPE_BIOMETRIC) "biometric" else "deviceCredential"
                out.put("android", JSONObject().put("authenticationType", type))
            } else {
                out.put("android", JSONObject.NULL)
            }
            reply(out)
        }, null)
    }

    override fun authenticate(crypto: BiometricPrompt.CryptoObject?, authenticators: Int, reason: String, origin: String,
                              done: (BiometricPrompt.CryptoObject?, JSONObject?) -> Unit) {
        if (origin != "app") return done(null, JSONObject().put("error", "PERMISSION_DENIED").put("message", "person-required"))
        prompt(reason, authenticators, JSONObject(), { result, error ->
            if (error != null) done(null, error) else done(result?.cryptoObject ?: crypto, null)
        }, crypto)
    }

    override fun configure(builder: KeyGenParameterSpec.Builder) {
        keyHatch?.invoke(builder)
    }
}
