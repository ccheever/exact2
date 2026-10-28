// @exact/payments on Android (LLP 1133.034 D5, D6, D10, D11, D19): the Google Pay API (`play-services-wallet`, in the
// app's one locked Gradle graph), one sheet at a time over the resumed activity. The build makes one per host when the
// app binds the extension, outside the agent; under the agent the runner's substitute answers its names, and only the
// button is drawn here.
//
// - The request is the W3C call's words in Google's: the Google Pay method's `data` (`allowedPaymentMethods`,
//   `merchantInfo`, …), the details as `transactionInfo` (the total, its display items), the shipping options as
//   `shippingOptionParameters`, and the options as `emailRequired`, `shippingAddressRequired` and the phone. The
//   environment is the bake's: `TEST` until the app has a production Google Pay merchant (§13 question 7).
// - The sheet's callbacks (`PaymentCallbacks`, the service Google binds while the sheet is up) call the app's
//   handlers through `invoke`, by the call's `request:<n>` handle, each with D5's budget: Google's address is
//   already redacted (a country, a region, a city, a postal code), and the full one comes with the payment.
// - The final handler's answer is D6's table: `success` closes the sheet; `fail`, `unknown` and `retry` are Google's
//   error result, which keeps the sheet up for another card or a corrected address.
// - A call let go cannot withdraw Google's activity: the request is abandoned, and an authorization that arrives is
//   answered as failed without calling the app (D6 phase 3).
package exact.payments

import android.app.Activity
import android.content.Context
import android.os.Handler
import android.os.Looper
import android.view.View
import com.google.android.gms.common.api.ResolvableApiException
import com.google.android.gms.wallet.AutoResolveHelper
import com.google.android.gms.wallet.IsReadyToPayRequest
import com.google.android.gms.wallet.PaymentData
import com.google.android.gms.wallet.PaymentDataRequest
import com.google.android.gms.wallet.PaymentsClient
import com.google.android.gms.wallet.Wallet
import com.google.android.gms.wallet.WalletConstants
import com.google.android.gms.wallet.button.ButtonConstants
import com.google.android.gms.wallet.button.ButtonOptions
import com.google.android.gms.wallet.button.PayButton
import dev.exact.canvas.ExactExtension
import dev.exact.canvas.ExtensionContext
import org.json.JSONArray
import org.json.JSONObject

private const val GOOGLE = "https://google.com/pay"
private const val BUDGET = 25_000L

/** [value] as a JSON object: an object, or JSON text of one (a Contract call's `json<…>`). */
internal fun objectOf(value: Any?): JSONObject? = when (value) {
    is JSONObject -> value
    is String -> try { JSONObject(value) } catch (e: org.json.JSONException) { null }
    else -> null
}

class PaymentsModule(context: ExtensionContext) : ExactExtension(context) {
    companion object {
        /** The app's access hatch (D11): each request, after the module's words are set; its replacement. */
        @JvmStatic var paymentDataRequest: ((PaymentDataRequest) -> PaymentDataRequest)? = null
        /** The module whose sheet is up, which the callbacks service reaches. */
        internal var current: PaymentsModule? = null
    }

    private val main = Handler(Looper.getMainLooper())
    private val client: PaymentsClient by lazy {
        Wallet.getPaymentsClient(context.application, Wallet.WalletOptions.Builder().setEnvironment(WalletConstants.ENVIRONMENT_TEST).build())
    }

    /** The sheet up: its call's number, arguments, reply and last result. */
    private class Sheet(val request: Int, val id: String, val args: JSONObject, val reply: (JSONObject) -> Unit) {
        var letGo = false
        var authorizing = false
        var result: JSONObject? = null
        var option: String? = null
    }
    private var sheet: Sheet? = null

    override fun later(name: String, args: JSONObject, reply: (JSONObject) -> Unit) {
        when (name) {
            "paymentRequestStatus" -> status(args, reply)
            "showPaymentRequest" -> show(args, reply)
            else -> reply(JSONObject().put("error", "NotSupportedError"))
        }
    }

    /** The Google Pay method's `data`, from a call's method data. */
    private fun googleData(args: JSONObject): JSONObject? {
        val list = args.optJSONArray("methodData") ?: return null
        for (i in 0 until list.length()) {
            val m = list.optJSONObject(i) ?: continue
            if (m.optString("supportedMethods") == GOOGLE) return objectOf(m.opt("data")) ?: JSONObject()
        }
        return null
    }

    private fun status(args: JSONObject, reply: (JSONObject) -> Unit) {
        val data = googleData(args) ?: return reply(JSONObject().put("available", true).put("canMakePayment", false).put("hasEnrolledInstrument", false))
        // The methods without their tokenization: what `isReadyToPay` takes.
        val methods = JSONArray()
        val allowed = data.optJSONArray("allowedPaymentMethods") ?: JSONArray()
        for (i in 0 until allowed.length()) {
            val m = JSONObject(allowed.getJSONObject(i).toString())
            m.remove("tokenizationSpecification")
            methods.put(m)
        }
        val ready = JSONObject().put("apiVersion", 2).put("apiVersionMinor", 0).put("allowedPaymentMethods", methods)
        client.isReadyToPay(IsReadyToPayRequest.fromJson(ready.toString())).addOnCompleteListener { task ->
            val can = task.isSuccessful && task.result == true
            reply(JSONObject().put("available", true).put("canMakePayment", can).put("hasEnrolledInstrument", JSONObject.NULL).put("methodName", GOOGLE))
        }
    }

    private fun amount(value: JSONObject?): String = value?.optJSONObject("amount")?.optString("value") ?: "0.00"
    private fun currency(value: JSONObject?): String = value?.optJSONObject("amount")?.optString("currency") ?: "USD"

    /** The details as Google's `transactionInfo`. */
    private fun transaction(details: JSONObject, country: String?): JSONObject {
        val total = details.optJSONObject("total")
        val items = JSONArray()
        val listed = details.optJSONArray("displayItems") ?: JSONArray()
        for (i in 0 until listed.length()) {
            val item = listed.getJSONObject(i)
            items.put(JSONObject().put("label", item.optString("label")).put("type", "LINE_ITEM").put("price", amount(item))
                .put("status", if (item.optBoolean("pending")) "PENDING" else "FINAL"))
        }
        return JSONObject().put("totalPriceStatus", if (total?.optBoolean("pending") == true) "ESTIMATED" else "FINAL")
            .put("totalPrice", amount(total)).put("currencyCode", currency(total)).put("totalPriceLabel", total?.optString("label") ?: "")
            .put("displayItems", items).apply { if (country != null) put("countryCode", country) }
    }

    /** The shipping options as Google's `shippingOptionParameters`. */
    private fun shippingOptions(list: JSONArray?): JSONObject? {
        if (list == null || list.length() == 0) return null
        val options = JSONArray()
        var selected: String? = null
        for (i in 0 until list.length()) {
            val o = list.getJSONObject(i)
            options.put(JSONObject().put("id", o.optString("id")).put("label", o.optString("label"))
                .put("description", "${currency(o)} ${amount(o)}"))
            if (o.optBoolean("selected")) selected = o.optString("id")
        }
        return JSONObject().put("shippingOptions", options).put("defaultSelectedOptionId", selected ?: options.getJSONObject(0).getString("id"))
    }

    private fun show(args: JSONObject, reply: (JSONObject) -> Unit) {
        if (context.origin != "app") return reply(JSONObject().put("error", "NotAllowedError").put("message", "person-required"))
        if (sheet != null) return reply(JSONObject().put("error", "AbortError").put("message", "a payment sheet is up"))
        val data = googleData(args) ?: return reply(JSONObject().put("error", "NotSupportedError"))
        val details = args.optJSONObject("details") ?: JSONObject()
        val options = args.optJSONObject("options") ?: JSONObject()
        val request = JSONObject(data.toString()).put("apiVersion", 2).put("apiVersionMinor", 0)
        request.put("transactionInfo", transaction(details, null))
        if (options.optBoolean("requestPayerEmail")) request.put("emailRequired", true)
        val shipping = options.optBoolean("requestShipping")
        if (shipping) {
            request.put("shippingAddressRequired", true)
            val parameters = request.optJSONObject("shippingAddressParameters") ?: JSONObject()
            if (options.optBoolean("requestPayerPhone")) parameters.put("phoneNumberRequired", true)
            request.put("shippingAddressParameters", parameters)
            request.put("shippingOptionRequired", true)
            shippingOptions(details.optJSONArray("shippingOptions"))?.let { request.put("shippingOptionParameters", it) }
        }
        if (options.optBoolean("requestBillingAddress")) {
            val methods = request.optJSONArray("allowedPaymentMethods") ?: JSONArray()
            for (i in 0 until methods.length()) methods.getJSONObject(i).optJSONObject("parameters")?.put("billingAddressRequired", true)
        }
        val intents = JSONArray().put("PAYMENT_AUTHORIZATION")
        if (shipping && args.has("onshippingaddresschange")) intents.put("SHIPPING_ADDRESS")
        if (shipping && args.has("onshippingoptionchange")) intents.put("SHIPPING_OPTION")
        request.put("callbackIntents", intents)
        val ours = PaymentDataRequest.fromJson(request.toString())
        val made = paymentDataRequest?.invoke(ours) ?: ours
        val s = Sheet(context.request ?: 0, details.optString("id").ifEmpty { java.util.UUID.randomUUID().toString() }, args, reply)
        sheet = s
        current = this
        context.log("payments: ${s.id}: shown, $GOOGLE")
        client.loadPaymentData(made).addOnCompleteListener { task ->
            when {
                task.isSuccessful -> finish(s, Activity.RESULT_OK, task.result)
                task.exception is ResolvableApiException -> {
                    context.startIntentSenderForResult((task.exception as ResolvableApiException).resolution.intentSender) { code, intent ->
                        finish(s, code, intent?.let { PaymentData.getFromIntent(it) }, intent)
                    }
                }
                else -> finish(s, AutoResolveHelper.RESULT_ERROR, null)
            }
        }
    }

    /** Google's activity ended: the send answers the last result, or `AbortError` with none. */
    private fun finish(s: Sheet, code: Int, data: PaymentData?, intent: android.content.Intent? = null) {
        if (sheet !== s) return
        sheet = null
        if (current === this) current = null
        if (s.letGo) return context.log("payment: closed after its call ended (${s.id}, last result ${s.result?.optString("result") ?: "none"})")
        val answer = when {
            s.result != null -> s.result!!
            code == Activity.RESULT_CANCELED -> JSONObject().put("error", "AbortError")
            code == AutoResolveHelper.RESULT_ERROR -> JSONObject().put("error", "OperationError")
                .put("message", intent?.let { AutoResolveHelper.getStatusFromIntent(it)?.statusMessage } ?: "Google Pay failed")
            data != null -> JSONObject().put("error", "OperationError").put("message", "authorized with no handler's answer")
            else -> JSONObject().put("error", "AbortError")
        }
        s.reply(answer)
    }

    /** Run the app's handler named by [keyword] for the sheet up; null past the budget or on an error. */
    private fun ask(s: Sheet, keyword: String, arg: JSONObject, done: (JSONObject?) -> Unit) {
        val handler = s.args.optString(keyword).ifEmpty { return done(null) }
        var answered = false
        val finish = { value: JSONObject? -> if (!answered) { answered = true; done(value) } }
        main.postDelayed({ if (!answered) { context.log("payments: $handler for ${s.id}: past its budget"); finish(null) } }, BUDGET)
        context.invoke("request:${s.request}", handler, arg) { answer ->
            finish(answer.optString("text").ifEmpty { null }?.let { objectOf(it) })
        }
    }

    /** Google's address as the W3C's `ContactAddress`. */
    private fun address(a: JSONObject?): JSONObject? = a?.let {
        val lines = JSONArray()
        for (k in listOf("address1", "address2", "address3")) it.optString(k).takeIf { l -> l.isNotEmpty() }?.let(lines::put)
        JSONObject().put("country", it.optString("countryCode")).put("region", it.optString("administrativeArea"))
            .put("city", it.optString("locality")).put("postalCode", it.optString("postalCode")).put("sortingCode", it.optString("sortingCode"))
            .put("addressLine", lines).put("recipient", it.optString("name")).put("phone", it.optString("phoneNumber")).put("organization", "")
    }

    /** `onPaymentDataChanged`: the redacted address or the option chosen, and the app's update as Google's. */
    internal fun changed(intermediate: JSONObject, complete: (String) -> Unit) = main.post {
        val s = sheet
        if (s == null || s.letGo) return@post complete("{}")
        val trigger = intermediate.optString("callbackTrigger")
        val option = intermediate.optJSONObject("shippingOptionData")?.optString("id")?.takeIf { it.isNotEmpty() && it != "shipping_option_unselected" }
        if (option != null) s.option = option
        val place = intermediate.optJSONObject("shippingAddress")
        val shown = JSONObject().put("requestId", s.id).put("methodName", GOOGLE).put("shippingOption", s.option ?: JSONObject.NULL)
            .put("shippingAddress", place?.let { JSONObject().put("country", it.optString("countryCode")).put("region", it.optString("administrativeArea"))
                .put("city", it.optString("locality")).put("postalCode", it.optString("postalCode")) } ?: JSONObject.NULL)
        val keyword = if (trigger == "SHIPPING_OPTION") "onshippingoptionchange" else "onshippingaddresschange"
        ask(s, keyword, shown) { u ->
            val update = JSONObject()
            val details = JSONObject(s.args.optJSONObject("details")?.toString() ?: "{}")
            if (u == null) return@ask complete(update.put("error", JSONObject().put("reason", "OTHER_ERROR").put("message", "unavailable").put("intent", "SHIPPING_ADDRESS")).toString())
            for (k in listOf("total", "displayItems", "shippingOptions")) if (u.has(k)) details.put(k, u.get(k))
            update.put("newTransactionInfo", transaction(details, null))
            shippingOptions(details.optJSONArray("shippingOptions"))?.let { p ->
                s.option?.let { chosen -> p.put("defaultSelectedOptionId", chosen) }
                update.put("newShippingOptionParameters", p)
            }
            val error = u.optString("error").ifEmpty { u.optJSONObject("shippingAddressErrors")?.let { e -> e.keys().asSequence().map { e.optString(it) }.firstOrNull() } ?: "" }
            if (error.isNotEmpty()) update.put("error", JSONObject().put("reason", "SHIPPING_ADDRESS_UNSERVICEABLE").put("message", error).put("intent", "SHIPPING_ADDRESS"))
            complete(update.toString())
        }
    }

    /** `onPaymentAuthorized`: the payment to the final handler, and its answer as Google's result. */
    internal fun authorized(payment: JSONObject, complete: (String) -> Unit) = main.post {
        val failed = { message: String, intent: String, reason: String ->
            JSONObject().put("transactionState", "ERROR").put("error", JSONObject().put("reason", reason).put("message", message).put("intent", intent)).toString()
        }
        val s = sheet
        if (s == null || s.letGo) {
            context.log("payment ${s?.id ?: ""}: authorized after its call ended; answered as failed, no handler run")
            return@post complete(failed("This order was closed.", "PAYMENT_AUTHORIZATION", "OTHER_ERROR"))
        }
        val ship = payment.optJSONObject("shippingAddress")
        val info = payment.optJSONObject("paymentMethodData")
        val response = JSONObject().put("requestId", s.id).put("methodName", GOOGLE).put("details", payment)
            .put("shippingAddress", address(ship) ?: JSONObject.NULL).put("shippingOption", s.option ?: JSONObject.NULL)
            .put("payerName", ship?.optString("name") ?: JSONObject.NULL).put("payerEmail", payment.optString("email").ifEmpty { null } ?: JSONObject.NULL)
            .put("payerPhone", ship?.optString("phoneNumber")?.ifEmpty { null } ?: JSONObject.NULL)
        s.authorizing = true
        ask(s, "onpaymentauthorized", response) { value ->
            s.authorizing = false
            val retry = value?.optJSONObject("retry")
            if (retry != null) {
                val field = retry.optJSONObject("shippingAddress")?.let { e -> e.keys().asSequence().map { e.optString(it) }.firstOrNull() }
                return@ask complete(if (field != null) failed(field, "SHIPPING_ADDRESS", "SHIPPING_ADDRESS_INVALID")
                    else failed(retry.optString("error").ifEmpty { "Try another card." }, "PAYMENT_AUTHORIZATION", "PAYMENT_DATA_INVALID"))
            }
            val result = value?.optString("result")?.takeIf { it in listOf("success", "fail", "unknown") } ?: "fail"
            val out = JSONObject().put("requestId", s.id).put("methodName", GOOGLE).put("result", result)
                .put("shippingAddress", response.opt("shippingAddress")).put("shippingOption", response.opt("shippingOption"))
                .put("payerName", response.opt("payerName")).put("payerEmail", response.opt("payerEmail")).put("payerPhone", response.opt("payerPhone"))
                .put("android", JSONObject().put("paymentMethodDescription", info?.optString("description") ?: "").put("cardNetwork", info?.optJSONObject("info")?.optString("cardNetwork") ?: ""))
            if (value == null) out.put("message", "authorize-timeout")
            value?.opt("data")?.let { out.put("data", it.toString()) }
            s.result = out
            context.log("payments: ${s.id}: $result")
            // Past the budget the request is abandoned: no later authorization calls the app (D6).
            if (value == null) s.letGo = true
            complete(if (result == "success") JSONObject().put("transactionState", "SUCCESS").toString()
                else failed("The payment didn’t go through.", "PAYMENT_AUTHORIZATION", "PAYMENT_DATA_INVALID"))
        }
    }

    /** The call was let go: Google's activity stays, so the request is abandoned (D6 phase 3). */
    override fun cancel(request: Int) {
        val s = sheet ?: return
        if (s.request != request || s.authorizing) return
        s.letGo = true
        context.log("payment ${s.id}: abandoned, its call was let go; the sheet stays until it is closed")
    }

    override fun view(tag: String, session: String, id: String, context: Context, props: JSONObject, emit: (String, String) -> Unit): View {
        require(tag == "exact-payment-button") { "PaymentsModule serves no view $tag" }
        val button = PayButton(context)
        button.tag = props.toString()
        button.initialize(buttonOptions(props))
        button.setOnClickListener { emit("press", "") }
        return button
    }

    override fun update(view: View, props: JSONObject) {
        val button = view as? PayButton ?: return
        // A PayButton is initialized once: its type and methods are fixed then.
        button.isEnabled = !props.optBoolean("disabled")
    }

    private fun buttonOptions(props: JSONObject): ButtonOptions {
        val type = when (props.optString("buttonType")) {
            "buy" -> ButtonConstants.ButtonType.BUY
            "checkout" -> ButtonConstants.ButtonType.CHECKOUT
            "order" -> ButtonConstants.ButtonType.ORDER
            "book" -> ButtonConstants.ButtonType.BOOK
            "donate" -> ButtonConstants.ButtonType.DONATE
            "subscribe" -> ButtonConstants.ButtonType.SUBSCRIBE
            "pay" -> ButtonConstants.ButtonType.PAY
            else -> ButtonConstants.ButtonType.PLAIN
        }
        // The button shows the card it would use, from the checkout's Google Pay methods.
        val checkout = objectOf(props.opt("checkout"))
        val methods = checkout?.optJSONArray("methodData")?.let { list ->
            (0 until list.length()).map { list.getJSONObject(it) }.firstOrNull { it.optString("supportedMethods") == GOOGLE }
        }?.let { objectOf(it.opt("data")) }?.optJSONArray("allowedPaymentMethods")
            ?: JSONArray().put(JSONObject().put("type", "CARD").put("parameters", JSONObject()
                .put("allowedAuthMethods", JSONArray().put("PAN_ONLY").put("CRYPTOGRAM_3DS"))
                .put("allowedCardNetworks", JSONArray().put("VISA").put("MASTERCARD"))))
        return ButtonOptions.newBuilder().setButtonType(type).setAllowedPaymentMethods(methods.toString()).build()
    }
}
