package dev.exact.canvas

import android.app.KeyguardManager
import android.content.Context
import android.hardware.biometrics.BiometricManager.Authenticators
import android.hardware.biometrics.BiometricPrompt
import android.os.Build
import android.security.keystore.KeyGenParameterSpec
import android.security.keystore.KeyPermanentlyInvalidatedException
import android.security.keystore.KeyProperties
import android.security.keystore.StrongBoxUnavailableException
import org.json.JSONObject
import java.io.DataInputStream
import java.io.DataOutputStream
import java.io.File
import java.security.KeyFactory
import java.security.KeyPairGenerator
import java.security.KeyStore
import java.security.MessageDigest
import java.security.PrivateKey
import java.security.SecureRandom
import java.security.spec.MGF1ParameterSpec
import java.security.spec.X509EncodedKeySpec
import java.util.concurrent.Executors
import javax.crypto.Cipher
import javax.crypto.spec.GCMParameterSpec
import javax.crypto.spec.OAEPParameterSpec
import javax.crypto.spec.PSource
import javax.crypto.spec.SecretKeySpec

/**
 * Locked secrets in the reader (LLP 1133.010 D6, D8, D13). The host's backend owns every move (its Rust, over the
 * files here); this keeps the Keystore's half:
 *
 * - **A write** ([keystore], op 1, called synchronously from the host's thread): a Keystore key pair per name and
 *   policy, generated at the name's first locked write from its line alone (RSA with OAEP; authentication required
 *   for every use with the policy's authenticators; StrongBox when the line says so; unlocked-device-required from
 *   API 35, D3). The value is sealed under a fresh AES-GCM key, which the public half wraps, so a write never asks
 *   for the person ("public key operations are not restricted"). The file is written beside, then renamed to its
 *   generation's name, so the last value stays readable until the new one is durable.
 * - **A read** ([check], the host's request): a decrypting `Cipher` over the private key, the bound implementation's
 *   prompt with it as the `CryptoObject` ([ExactAuthenticator]), then the data key unwrapped and the value opened on
 *   a background executor, answered on the store's completion path ([Exact.presence]).
 * - **The platform's drops:** a key the platform invalidated (the lock screen removed; a `biometry-current-set` key
 *   after enrolment changed) or a file sealed under a line's earlier policy is deleted unread, and the read answers
 *   nothing: the app signs in again.
 *
 * The files are under `exact-presence/` in the app's files, a directory of its own outside the plain secrets', which
 * the manifest keeps out of backups and device transfer (a restored file without its key could never open).
 */
object Presence {
    /** The app, for the Keystore's and the keyguard's services. */
    @Volatile internal var application: Context? = null
    private const val PREFIX = "exact.presence."
    private const val OAEP = "RSA/ECB/OAEPWithSHA-256AndMGF1Padding"
    /** Keystore's RSA decrypts OAEP with MGF1 over SHA-1 only; the public half encrypts to match. */
    private val oaep = OAEPParameterSpec("SHA-256", "MGF1", MGF1ParameterSpec.SHA1, PSource.PSpecified.DEFAULT)
    private val background = Executors.newSingleThreadExecutor()

    /** The host's synchronous call (`host/linux/src/canvas/presence.rs`): `ok:<text>` or `err:<why>`. */
    @JvmStatic fun keystore(op: Int, request: String, value: String): String = try {
        when (op) {
            1 -> { add(JSONObject(request), value); "ok:" }
            5 -> "ok:" + if (capable()) "1" else "0"
            else -> "err:keystore op $op is not one this reader knows"
        }
    } catch (e: StrongBoxUnavailableException) {
        "err:NOT_AVAILABLE: this device has no StrongBox (android.strongBox)"
    } catch (e: Exception) {
        "err:${e.javaClass.simpleName}: ${e.message}"
    }

    /** A locked item can be kept: API 30 (a `CryptoObject` with the device credential) and a secure lock screen. */
    private fun capable(): Boolean {
        val app = application ?: return false
        return Build.VERSION.SDK_INT >= 30 && app.getSystemService(KeyguardManager::class.java).isDeviceSecure
    }

    /** The policy word and the platform words of a line's `policy`. */
    private fun words(policy: String) = policy.split(' ').filter { it.isNotEmpty() }

    /** The prompt's authenticators for a line (`BiometricManager.Authenticators`). */
    fun authenticators(policy: String): Int = when (words(policy).firstOrNull()) {
        "biometry-any", "biometry-current-set" -> Authenticators.BIOMETRIC_STRONG
        "device-credential" -> Authenticators.DEVICE_CREDENTIAL
        else -> Authenticators.BIOMETRIC_STRONG or Authenticators.DEVICE_CREDENTIAL
    }

    /** A name's key under its line: a changed policy or StrongBox word is another alias, and the old key's files are
     * dropped as a policy change (D4). */
    private fun alias(name: String, policy: String): String {
        val owned = words(policy).filter { it != "optional" && !it.startsWith("apple.") }.joinToString(" ")
        val hash = MessageDigest.getInstance("SHA-256").digest(owned.toByteArray()).take(6).joinToString("") { "%02x".format(it) }
        return "$PREFIX$name.$hash"
    }

    private fun keyStore() = KeyStore.getInstance("AndroidKeyStore").apply { load(null) }

    private fun add(request: JSONObject, value: String) {
        val dir = File(request.getString("dir"))
        val account = request.getString("account")
        val policy = request.getString("policy")
        val alias = alias(account.substringBefore('#'), policy)
        val store = keyStore()
        if (!store.containsAlias(alias)) generate(alias, policy)
        val public = store.getCertificate(alias).publicKey
        // A software key of the public half: the public operation needs no person.
        val unrestricted = KeyFactory.getInstance(public.algorithm).generatePublic(X509EncodedKeySpec(public.encoded))
        val data = ByteArray(32).also { SecureRandom().nextBytes(it) }
        try {
            val wrap = Cipher.getInstance(OAEP).apply { init(Cipher.ENCRYPT_MODE, unrestricted, oaep) }
            val wrapped = wrap.doFinal(data)
            val iv = ByteArray(12).also { SecureRandom().nextBytes(it) }
            val seal = Cipher.getInstance("AES/GCM/NoPadding").apply { init(Cipher.ENCRYPT_MODE, SecretKeySpec(data, "AES"), GCMParameterSpec(128, iv)) }
            val sealed = seal.doFinal(value.toByteArray())
            dir.mkdirs()
            val beside = File(dir, ".$account.${System.nanoTime()}")
            val stream = beside.outputStream()
            DataOutputStream(stream).use { out ->
                out.writeUTF(alias)
                for (part in listOf(wrapped, iv, sealed)) { out.writeInt(part.size); out.write(part) }
                out.flush()
                stream.fd.sync()
            }
            if (!beside.renameTo(File(dir, account))) { beside.delete(); throw java.io.IOException("could not keep $account") }
        } finally {
            data.fill(0)
        }
    }

    private fun generate(alias: String, policy: String) {
        val words = words(policy)
        val strongBox = "android.strongBox" in words
        // The key's types: AUTH_BIOMETRIC_STRONG 1, AUTH_DEVICE_CREDENTIAL 2.
        val types = when (words.firstOrNull()) {
            "biometry-any", "biometry-current-set" -> KeyProperties.AUTH_BIOMETRIC_STRONG
            "device-credential" -> KeyProperties.AUTH_DEVICE_CREDENTIAL
            else -> KeyProperties.AUTH_BIOMETRIC_STRONG or KeyProperties.AUTH_DEVICE_CREDENTIAL
        }
        val own = { b: KeyGenParameterSpec.Builder ->
            b.setDigests(KeyProperties.DIGEST_SHA256, KeyProperties.DIGEST_SHA1)
                .setEncryptionPaddings(KeyProperties.ENCRYPTION_PADDING_RSA_OAEP)
                .setUserAuthenticationRequired(true)
                .setInvalidatedByBiometricEnrollment(words.firstOrNull() == "biometry-current-set")
                .setIsStrongBoxBacked(strongBox)
            if (Build.VERSION.SDK_INT >= 30) b.setUserAuthenticationParameters(0, types)
            if (Build.VERSION.SDK_INT >= 35) b.setUnlockedDeviceRequired(true)
        }
        val builder = KeyGenParameterSpec.Builder(alias, KeyProperties.PURPOSE_DECRYPT or KeyProperties.PURPOSE_ENCRYPT).setKeySize(2048)
        own(builder)
        // The key hatch, then what the backend owns set again: a hatch cannot weaken the gate (D11).
        Extensions.authenticator()?.configure(builder)
        own(builder)
        KeyPairGenerator.getInstance(KeyProperties.KEY_ALGORITHM_RSA, "AndroidKeyStore").apply { initialize(builder.build()) }.generateKeyPair()
    }

    /** One sealed item: its key's alias, the wrapped data key, the IV, the sealed value. */
    private class Sealed(val alias: String, val wrapped: ByteArray, val iv: ByteArray, val sealed: ByteArray)

    private fun read(file: File): Sealed = DataInputStream(file.inputStream()).use { input ->
        val alias = input.readUTF()
        val part = { ByteArray(input.readInt()).also { input.readFully(it) } }
        Sealed(alias, part(), part(), part())
    }

    /**
     * The host's request for a check (`PresenceAsk::json`), on the main thread: with a `service`, the item's
     * directory, the prompt over its key and then the read; without one (under the agent, its store keeping the
     * value), the prompt alone.
     */
    fun check(body: String) {
        val request = JSONObject(body)
        val call = request.getLong("call")
        val answer = { fields: JSONObject -> Exact.presence(fields.put("call", call).toString()) }
        val failed = { code: String, message: String -> answer(JSONObject().put("status", "failed").put("code", code).put("message", message)) }
        val origin = request.optString("origin", "app")
        if (origin != "app") return failed("PERMISSION_DENIED", "person-required")
        val authenticator = Extensions.authenticator() ?: return failed("NotSupportedError", "this APK binds no authenticator (LLP 1133.010 D13)")
        val policy = request.optString("policy")
        val reason = request.optString("reason")
        val service = request.optString("service").takeIf { it.isNotEmpty() && !request.isNull("service") }
        if (service == null) {
            authenticator.authenticate(null, authenticators(policy), reason, origin) { _, error ->
                if (error != null) failed(error.optString("error"), error.optString("message")) else answer(JSONObject().put("status", "passed"))
            }
            return
        }
        val file = File(service, request.getString("account"))
        val gone = { answer(JSONObject().put("status", "read")) }
        if (!file.exists()) return gone()
        val sealed = try { read(file) } catch (e: Exception) { return failed("UnknownError", "the item could not be read: $e") }
        val store = keyStore()
        // A file of an earlier policy, or a key the platform dropped: deleted unread.
        val drop = { store.deleteEntry(sealed.alias); file.delete(); gone() }
        if (sealed.alias != alias(request.getString("name"), policy)) { file.delete(); return gone() }
        val key = store.getKey(sealed.alias, null) as? PrivateKey ?: return drop()
        val cipher = Cipher.getInstance(OAEP)
        try {
            cipher.init(Cipher.DECRYPT_MODE, key, oaep)
        } catch (e: KeyPermanentlyInvalidatedException) {
            return drop()
        }
        authenticator.authenticate(BiometricPrompt.CryptoObject(cipher), authenticators(policy), reason, origin) { crypto, error ->
            if (error != null) return@authenticate failed(error.optString("error"), error.optString("message"))
            val unwrap = crypto?.cipher ?: return@authenticate failed("UnknownError", "the prompt returned no cipher")
            background.execute {
                val data = try { unwrap.doFinal(sealed.wrapped) } catch (e: Exception) { null }
                if (data == null) return@execute failed("AUTHENTICATION_FAILED", "the key did not open the item")
                try {
                    val open = Cipher.getInstance("AES/GCM/NoPadding").apply { init(Cipher.DECRYPT_MODE, SecretKeySpec(data, "AES"), GCMParameterSpec(128, sealed.iv)) }
                    answer(JSONObject().put("status", "read").put("value", String(open.doFinal(sealed.sealed))))
                } catch (e: Exception) {
                    failed("UnknownError", "the item did not open: ${e.javaClass.simpleName}")
                } finally {
                    data.fill(0)
                }
            }
        }
    }
}
