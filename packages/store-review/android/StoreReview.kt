// @exact/store-review on Android (LLP 1133.015 D12): Play's In-App Review. `requestReviewFlow`, then
// `launchReviewFlow` over the activity in front; Play says nothing of whether the dialog showed, so the request
// answers nothing and journals that it was made. The flow's task completes when it has finished, shown or not, which
// is when the request answers. Under the agent it asks Play for nothing and journals each request.
package exact.storereview

import com.google.android.play.core.review.ReviewManagerFactory
import dev.exact.canvas.ExactExtension
import dev.exact.canvas.ExtensionContext
import org.json.JSONObject

class StoreReviewModule(context: ExtensionContext) : ExactExtension(context) {
    private val manager by lazy { ReviewManagerFactory.create(context.application) }

    override fun later(name: String, args: JSONObject, reply: (JSONObject) -> Unit) {
        when (name) {
            "storeReviewStatus" -> reply(JSONObject().put("available", true))
            "requestReview" -> request(reply)
            else -> reply(JSONObject().put("error", "NotSupportedError"))
        }
    }

    private fun request(reply: (JSONObject) -> Unit) {
        if (context.agent) {
            context.log("requestReview: requested (${context.origin})")
            return reply(JSONObject())
        }
        val activity = context.activity ?: return reply(JSONObject()).also { context.log("requestReview: not requested, no activity in front") }
        manager.requestReviewFlow().addOnCompleteListener { asked ->
            if (!asked.isSuccessful) {
                context.log("requestReview: Play declined to prepare the flow")
                return@addOnCompleteListener reply(JSONObject())
            }
            manager.launchReviewFlow(activity, asked.result).addOnCompleteListener {
                context.log("requestReview: requested (${context.origin})")
                reply(JSONObject())
            }
        }
    }
}
