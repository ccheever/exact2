// @exact/age-range on Android (LLP 1133.028 D8, D10, D11): Google Play's Age Signals. `requestAgeSignalsAccess` over
// the activity in front, then `checkAgeSignals`, answered in Play's own bands (the Console's; 0–12, 13–15, 16–17
// and 18+ by default): Play's `ageLower` and `ageUpper` as they come, never coarsened into the app's gates, which
// the module cannot read from the Console (D8).
//
// Under the agent it asks Play for nothing: the `age-range` ticket's answer reaches it as `preferred`, a typed age
// answering Play's default band, `decline` `declined`, `unverified` Play's `VERIFICATION_REQUIRED`, and
// `unavailable` NotSupportedError (§7).
package exact.agerange

import android.os.Build
import com.google.android.gms.tasks.Task
import com.google.android.play.agesignals.AgeSignalsAccessRequest
import com.google.android.play.agesignals.AgeSignalsException
import com.google.android.play.agesignals.AgeSignalsManagerFactory
import com.google.android.play.agesignals.AgeSignalsRequest
import com.google.android.play.agesignals.AgeSignalsResult
import com.google.android.play.agesignals.model.AgeRangeSource
import com.google.android.play.agesignals.model.AgeSignalsErrorCode
import com.google.android.play.agesignals.model.AgeSignalsStatus
import com.google.android.play.agesignals.model.SignificantChangeStatus
import dev.exact.canvas.ExactExtension
import dev.exact.canvas.ExtensionContext
import org.json.JSONArray
import org.json.JSONObject
import java.text.SimpleDateFormat
import java.util.Locale
import java.util.TimeZone

class AgeRangeModule(context: ExtensionContext) : ExactExtension(context) {
    companion object {
        /** Play's default bands, inclusive (D11). */
        private val BANDS = listOf(0 to 12, 13 to 15, 16 to 17, 18 to null)

        /** Apple's rule for gates (D8), which the module keeps on every platform: one to three whole ages,
         * ascending, at least two years apart. */
        fun refused(gates: JSONArray?): String? {
            val ages = (0 until (gates?.length() ?: 0)).map { gates!!.optDouble(it) }
            if (ages.size !in 1..3 || ages.any { it.isNaN() || it < 0 || it != Math.rint(it) }) return "ageGates are one to three whole ages"
            return if (ages.zipWithNext().all { (a, b) -> b - a >= 2 }) null else "ageGates ascend at least two years apart"
        }

        fun range(status: String, lower: Int, upper: Int?, declaration: String, android: Any): JSONObject = JSONObject()
            .put("status", status).put("lowerBound", lower).put("upperBound", upper ?: JSONObject.NULL)
            .put("declaration", declaration).put("apple", JSONObject.NULL).put("android", android)
    }

    private val manager by lazy { AgeSignalsManagerFactory.create(context.application) }
    private var overlay = JSONObject()
    private var asking = false

    private fun say(line: String) = context.log("age-range: $line")

    override fun later(name: String, args: JSONObject, reply: (JSONObject) -> Unit) = later(name, args, null, reply)

    override fun later(name: String, args: JSONObject, preferred: Any?, reply: (JSONObject) -> Unit) {
        when (name) {
            "ageRangeStatus" -> reply(status())
            "requestAgeRange" -> {
                refused(args.optJSONArray("ageGates"))?.let { return reply(JSONObject().put("error", "TypeError").put("message", it)) }
                if (context.agent) return reply(agentRequest(preferred as? JSONObject))
                // One request at a time: Play's prompt is one activity.
                if (asking) return reply(JSONObject().put("error", "InvalidStateError").put("message", "a request is already showing"))
                request(reply)
            }
            else -> reply(JSONObject().put("error", "NotSupportedError"))
        }
    }

    override fun preferred(values: JSONObject) { overlay = values.optJSONObject("ageRangeStatus") ?: JSONObject() }

    /** The library is here; outside the agent, whether Play installed the app, without which Play answers
     * `APP_NOT_OWNED` (D17). */
    private fun status(): JSONObject {
        val out = JSONObject().put("available", context.agent || installedByPlay()).put("regulated", JSONObject.NULL).put("apple", JSONObject.NULL)
        if (context.agent) overlay.keys().forEach { out.put(it, overlay.get(it)) }
        return out
    }

    private fun installedByPlay(): Boolean {
        val pm = context.application.packageManager
        val pkg = context.application.packageName
        val installer = try {
            if (Build.VERSION.SDK_INT >= 30) pm.getInstallSourceInfo(pkg).installingPackageName
            else @Suppress("DEPRECATION") pm.getInstallerPackageName(pkg)
        } catch (e: Exception) { null }
        return installer == "com.android.vending"
    }

    private fun request(reply: (JSONObject) -> Unit) {
        val activity = context.activity ?: return reply(JSONObject().put("error", "NotAllowedError").put("message", "no activity in front"))
        asking = true
        val done = { answer: JSONObject -> asking = false; reply(answer) }
        settle(manager.requestAgeSignalsAccess(AgeSignalsAccessRequest.builder().setActivity(activity).build()), { access ->
            when (access.ageSignalsStatus()) {
                AgeSignalsStatus.SHARED -> settle(manager.checkAgeSignals(AgeSignalsRequest.builder().build()), { done(answer(it)) }, done)
                AgeSignalsStatus.VERIFICATION_REQUIRED -> done(range("unverified", 0, null, "unknown", android("VERIFICATION_REQUIRED")))
                else -> done(range("declined", 0, null, "unknown", android("NOT_SHARED")))
            }
        }, done)
    }

    private fun answer(result: AgeSignalsResult): JSONObject {
        val source = when (result.ageRangeSource()) {
            AgeRangeSource.TIER_A -> "TIER_A"; AgeRangeSource.TIER_B -> "TIER_B"
            AgeRangeSource.TIER_C -> "TIER_C"; AgeRangeSource.TIER_D -> "TIER_D"; else -> null
        }
        val declaration = when (source) { "TIER_A" -> "self"; "TIER_B" -> "guardian"; "TIER_C", "TIER_D" -> "confirmed"; else -> "unknown" }
        val change = when (result.significantChangeStatus()) {
            SignificantChangeStatus.APPROVED -> "APPROVED"; SignificantChangeStatus.PENDING -> "PENDING"
            SignificantChangeStatus.DECLINED -> "DECLINED"; else -> null
        }
        val date = result.significantChangeApprovalDate()?.let {
            SimpleDateFormat("yyyy-MM-dd'T'HH:mm:ss'Z'", Locale.ROOT).apply { timeZone = TimeZone.getTimeZone("UTC") }.format(it)
        }
        say("shared")
        return range("sharing", result.ageLower() ?: 0, result.ageUpper(), declaration,
            android("SHARED", source, change, date, result.installId()))
    }

    private fun android(access: String, source: String? = null, change: String? = null, date: String? = null, installId: String? = null) =
        JSONObject().put("accessStatus", access).put("ageRangeSource", source ?: JSONObject.NULL)
            .put("significantChangeStatus", change ?: JSONObject.NULL).put("significantChangeApprovalDate", date ?: JSONObject.NULL)
            .put("installId", installId ?: JSONObject.NULL)

    private fun agentRequest(sheet: JSONObject?): JSONObject {
        if (!status().optBoolean("available", false)) return JSONObject().put("error", "NotSupportedError")
        val age = sheet?.optJSONArray("typed")?.optString(0)?.trim()?.toIntOrNull()
        if (age != null) {
            val (lower, upper) = BANDS.last { it.first <= age }
            say("shared, in Play's default bands")
            return range("sharing", lower, upper, "self", android("SHARED", "TIER_A"))
        }
        return when (sheet?.optString("tap")) {
            "unavailable" -> JSONObject().put("error", "NotSupportedError").put("android", JSONObject().put("code", AgeSignalsErrorCode.API_NOT_AVAILABLE))
            "unverified" -> range("unverified", 0, null, "unknown", android("VERIFICATION_REQUIRED"))
            else -> { say("declined"); range("declined", 0, null, "unknown", android("NOT_SHARED")) }
        }
    }

    private fun <T> settle(task: Task<T>, done: (T) -> Unit, failed: (JSONObject) -> Unit) {
        task.addOnSuccessListener { done(it) }.addOnFailureListener { failed(failure(it)) }
    }

    /** D8's names for Play's codes, the code beside. */
    private fun failure(e: Exception): JSONObject {
        val code = (e as? AgeSignalsException)?.errorCode
            ?: return JSONObject().put("error", "OperationError").put("message", e.message ?: "Age Signals failed")
        val name = when (code) {
            AgeSignalsErrorCode.API_NOT_AVAILABLE, AgeSignalsErrorCode.PLAY_STORE_NOT_FOUND, AgeSignalsErrorCode.PLAY_SERVICES_NOT_FOUND,
            AgeSignalsErrorCode.PLAY_STORE_VERSION_OUTDATED, AgeSignalsErrorCode.PLAY_SERVICES_VERSION_OUTDATED,
            AgeSignalsErrorCode.SDK_VERSION_OUTDATED -> "NotSupportedError"
            AgeSignalsErrorCode.APP_NOT_OWNED -> "NotAllowedError"
            AgeSignalsErrorCode.NETWORK_ERROR -> "NetworkError"
            else -> "OperationError"
        }
        return JSONObject().put("error", name).put("message", e.message ?: name).put("android", JSONObject().put("code", code))
    }
}
