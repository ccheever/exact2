// @exact/purchases on Android (LLP 1133.015): Play Billing 9.1, answered in the Digital Goods API's shapes. Under the
// agent the implementation is its own substitute (§9): it makes no BillingClient and keeps a synthetic store
// (AgentStore.kt) that behaves as Play does. A drive never binds it (`bind-real-money`): a sideloaded build signed in
// with an account that is not a license tester would pay.
//
// - Made after the reader's first frame (`"start": "load"`, D6): one BillingClient for the process, with pending
//   purchases and automatic reconnection, connected then, its purchases queried then and at every resume.
// - Two facts (D3): `listPurchases` is the query's `PURCHASED` rows, `unfinishedPurchases` its unacknowledged ones,
//   `PENDING` included. Play reports no expiry, so no answer names a stale time (D7).
// - Tier 3 (D11): `PurchasesModule.builder` sees the BillingClient.Builder before `build()`, `connected` the client
//   after its first connection, and `flow` each BillingFlowParams.Builder before `launchBillingFlow`.
package exact.purchases

import com.android.billingclient.api.AcknowledgePurchaseParams
import com.android.billingclient.api.BillingClient
import com.android.billingclient.api.BillingClientStateListener
import com.android.billingclient.api.BillingFlowParams
import com.android.billingclient.api.BillingResult
import com.android.billingclient.api.ConsumeParams
import com.android.billingclient.api.GetBillingConfigParams
import com.android.billingclient.api.PendingPurchasesParams
import com.android.billingclient.api.ProductDetails
import com.android.billingclient.api.Purchase
import com.android.billingclient.api.PurchasesUpdatedListener
import com.android.billingclient.api.QueryProductDetailsParams
import com.android.billingclient.api.QueryPurchasesParams
import dev.exact.canvas.ExactExtension
import dev.exact.canvas.ExtensionContext
import org.json.JSONArray
import org.json.JSONObject
import java.util.UUID

class PurchasesModule(context: ExtensionContext) : ExactExtension(context) {
    companion object {
        /** The app's hatches (D11): the builder before `build()`, the client after its first connection, and each
         * purchase's flow builder after the module's words are set. */
        @JvmStatic var builder: ((BillingClient.Builder) -> Unit)? = null
        @JvmStatic var connected: ((BillingClient) -> Unit)? = null
        @JvmStatic var flow: ((BillingFlowParams.Builder) -> Unit)? = null

        private var client: BillingClient? = null
        private var ready = false
        private val waiting = ArrayList<() -> Unit>()
        private var flowReply: ((JSONObject) -> Unit)? = null
        private var flowItem = ""
        private var country: String? = null
        internal var store: AgentStore? = null
        private val sessions = ArrayList<PurchasesModule>()

        fun error(code: String, message: String? = null): JSONObject =
            JSONObject().put("error", code).also { if (message != null) it.put("message", message) }

        /** Play's response codes in the interface's (D1). */
        fun failure(result: BillingResult): JSONObject = when (result.responseCode) {
            BillingClient.BillingResponseCode.USER_CANCELED -> error("AbortError")
            BillingClient.BillingResponseCode.FEATURE_NOT_SUPPORTED, BillingClient.BillingResponseCode.BILLING_UNAVAILABLE -> error("NotSupportedError", result.debugMessage)
            BillingClient.BillingResponseCode.ITEM_UNAVAILABLE -> error("ITEM_UNAVAILABLE", result.debugMessage)
            BillingClient.BillingResponseCode.ITEM_ALREADY_OWNED -> error("ALREADY_OWNED", result.debugMessage)
            BillingClient.BillingResponseCode.ITEM_NOT_OWNED -> error("NOT_OWNED", result.debugMessage)
            BillingClient.BillingResponseCode.NETWORK_ERROR, BillingClient.BillingResponseCode.SERVICE_UNAVAILABLE -> error("NETWORK", result.debugMessage)
            else -> error("OperationError", result.debugMessage).put("android", JSONObject().put("code", result.responseCode))
        }

        fun refuseAccount(args: JSONObject): JSONObject? {
            val token = args.optString("accountToken").ifEmpty { return null }
            return try { UUID.fromString(token); null } catch (e: IllegalArgumentException) { error("OperationError", "accountToken-not-uuid: $token is not a UUID") }
        }
    }

    private fun say(line: String) = context.log("purchases: $line")

    init {
        sessions.add(this)
        if (!context.agent && client == null) connect()
    }

    /** Both facts' readers are asked again. */
    private fun changed() = sessions.forEach {
        it.context.changed("purchases.listPurchases")
        it.context.changed("purchases.unfinishedPurchases")
    }

    override fun later(name: String, args: JSONObject, reply: (JSONObject) -> Unit) = later(name, args, null, reply)

    override fun later(name: String, args: JSONObject, preferred: Any?, reply: (JSONObject) -> Unit) {
        if (context.agent) return agentLater(name, args, preferred as? JSONObject, reply)
        afterConnect { realLater(name, args, reply) }
    }

    override fun preferred(values: JSONObject) {
        if (!context.agent) return
        val s = agentStore() ?: return
        s.catalog = values.optJSONArray("getDetails") ?: values.optJSONArray("catalog") ?: JSONArray()
        s.fault = values.optString("fault").ifEmpty { null }
        values.optJSONObject("storeEvent")?.let { if (s.apply(it, unixNow())) { say("store: ${s.disclosed()}"); changed() } }
    }

    /** Google: query purchases "when your app is launched or comes to the foreground". */
    override fun foreground(shown: Boolean) {
        if (!shown) return
        if (context.agent) agentStore()?.resumed(unixNow())
        changed()
    }

    override fun destroy(done: () -> Unit) {
        sessions.remove(this)
        if (sessions.isEmpty()) { client?.endConnection(); client = null; ready = false }
        done()
    }

    private fun status(): JSONObject = JSONObject().put("available", true).put("store", "google-play")
        .put("canMakePayments", context.agent || ready).put("storefront", if (context.agent) "US" else country ?: JSONObject.NULL)
        .put("apple", JSONObject.NULL).put("android", JSONObject().put("countryCode", if (context.agent) "US" else country ?: JSONObject.NULL))

    // MARK: under the agent (§9)

    private fun unixNow(): Double = AgentStore.unix(context.now())

    private fun agentStore(): AgentStore? {
        store?.let { return it }
        val file = context.file("app:/data/exact-agent-purchases.json") ?: return null
        return AgentStore(file) { say(it) }.also { store = it }
    }

    private fun agentLater(name: String, args: JSONObject, sheet: JSONObject?, reply: (JSONObject) -> Unit) {
        val s = agentStore() ?: return reply(error("OperationError", "no scratch tree for the synthetic store"))
        val now = unixNow()
        when (name) {
            "purchasesStatus" -> reply(status())
            "getDetails" -> {
                val ids = args.optJSONArray("itemIds") ?: JSONArray()
                reply(JSONObject().put("exact:answer", JSONArray((0 until ids.length()).mapNotNull { s.item(ids.getString(it))?.let(AgentStore::details) })))
            }
            "listPurchases" -> reply(JSONObject().put("exact:answer", JSONArray(s.owned(now).map { s.details(it) })))
            "unfinishedPurchases" -> reply(JSONObject().put("exact:answer", JSONArray(s.unfinished(now).map { s.details(it) })))
            "purchase" -> {
                refuseAccount(args)?.let { return reply(it) }
                val itemId = args.optString("itemId")
                if (s.item(itemId) == null) return reply(error("ITEM_UNAVAILABLE", "$itemId is not in the catalogue"))
                when (sheet?.optString("tap")) {
                    "buy", "pending" -> {
                        val t = s.buy(itemId, now, args.optString("accountToken").ifEmpty { null }, sheet.optString("tap") == "pending")
                            ?: return reply(error("ALREADY_OWNED"))
                        say("store: ${s.disclosed()}")
                        reply(s.details(t))
                        changed()
                    }
                    "unavailable" -> reply(error("ITEM_UNAVAILABLE"))
                    "fail" -> reply(error("NETWORK"))
                    else -> reply(error("AbortError"))
                }
            }
            // Play has no restore sheet: a fresh query (D2).
            "restorePurchases" -> { reply(JSONObject().put("exact:answer", JSONArray(s.owned(now).map { s.details(it) }))); changed() }
            "acknowledge", "consume" -> {
                val token = args.optString("purchaseToken")
                if (s.fault == "finish") { say("$name $token: refused at the drive's fault point; it stays unfinished"); return reply(error("OperationError", "fault point")) }
                if (!s.finish(token, name == "consume", now)) return reply(error("NOT_OWNED", "this account holds no $token"))
                say("store: ${s.disclosed()}")
                reply(JSONObject())
                changed()
            }
            else -> reply(error("NotSupportedError"))
        }
    }

    // MARK: Play Billing (stage 2)

    private val updated = PurchasesUpdatedListener { result, purchases ->
        val answer = flowReply
        flowReply = null
        if (answer != null) {
            val bought = purchases?.firstOrNull { flowItem in it.products }
            answer(if (result.responseCode == BillingClient.BillingResponseCode.OK && bought != null) details(bought) else failure(result))
        }
        changed()
    }

    private fun connect() {
        val b = BillingClient.newBuilder(context.application).setListener(updated)
            .enablePendingPurchases(PendingPurchasesParams.newBuilder().enableOneTimeProducts().build())
            .enableAutoServiceReconnection()
        builder?.invoke(b)
        val c = b.build()
        client = c
        c.startConnection(object : BillingClientStateListener {
            override fun onBillingSetupFinished(result: BillingResult) {
                ready = result.responseCode == BillingClient.BillingResponseCode.OK
                if (!ready) say("Play Billing is unavailable: ${result.debugMessage}")
                if (ready) {
                    connected?.invoke(c)
                    c.getBillingConfigAsync(GetBillingConfigParams.newBuilder().build()) { _, config -> country = config?.countryCode }
                }
                val now = ArrayList(waiting)
                waiting.clear()
                now.forEach { it() }
                changed()
            }
            override fun onBillingServiceDisconnected() { ready = false }
        })
    }

    /** [body] once the client's first connection has answered (D6: a call waits for the first query's chance). */
    private fun afterConnect(body: () -> Unit) { if (ready || client == null) body() else waiting.add(body) }

    private fun realLater(name: String, args: JSONObject, reply: (JSONObject) -> Unit) {
        val c = client ?: return reply(error("NotSupportedError"))
        if (!ready && name != "purchasesStatus") return reply(error("NotSupportedError", "Play Billing is unavailable"))
        when (name) {
            "purchasesStatus" -> reply(status())
            "getDetails" -> products(c, args.optJSONArray("itemIds") ?: JSONArray()) { found, result ->
                if (result.responseCode != BillingClient.BillingResponseCode.OK) return@products reply(failure(result))
                reply(JSONObject().put("exact:answer", JSONArray(found.map(::item))))
            }
            "listPurchases" -> query(c) { rows -> reply(JSONObject().put("exact:answer", JSONArray(rows.filter { it.purchaseState == Purchase.PurchaseState.PURCHASED }.map(::details)))) }
            "unfinishedPurchases" -> query(c) { rows -> reply(JSONObject().put("exact:answer", JSONArray(rows.filter { !it.isAcknowledged }.map(::details)))) }
            "restorePurchases" -> query(c) { rows -> reply(JSONObject().put("exact:answer", JSONArray(rows.filter { it.purchaseState == Purchase.PurchaseState.PURCHASED }.map(::details)))) }
            "purchase" -> purchase(c, args, reply)
            "acknowledge", "consume" -> query(c) { rows ->
                val token = args.optString("purchaseToken")
                val row = rows.firstOrNull { it.purchaseToken == token } ?: return@query reply(error("NOT_OWNED", "this account holds no $token"))
                if (name == "acknowledge" && row.isAcknowledged) return@query reply(JSONObject())
                val done = { result: BillingResult ->
                    reply(if (result.responseCode == BillingClient.BillingResponseCode.OK) JSONObject() else failure(result))
                    changed()
                }
                if (name == "consume") c.consumeAsync(ConsumeParams.newBuilder().setPurchaseToken(token).build()) { result, _ -> done(result) }
                else c.acknowledgePurchase(AcknowledgePurchaseParams.newBuilder().setPurchaseToken(token).build()) { done(it) }
            }
            else -> reply(error("NotSupportedError"))
        }
    }

    private fun products(c: BillingClient, ids: JSONArray, done: (List<ProductDetails>, BillingResult) -> Unit) {
        val wanted = (0 until ids.length()).map { ids.getString(it) }
        val found = ArrayList<ProductDetails>()
        // A product's type is Play's to know: ask for both, in-app products first.
        fun ask(type: String, then: (BillingResult) -> Unit) {
            val list = wanted.map { QueryProductDetailsParams.Product.newBuilder().setProductId(it).setProductType(type).build() }
            c.queryProductDetailsAsync(QueryProductDetailsParams.newBuilder().setProductList(list).build()) { result, details ->
                found.addAll(details.productDetailsList)
                then(result)
            }
        }
        ask(BillingClient.ProductType.INAPP) { first ->
            if (first.responseCode != BillingClient.BillingResponseCode.OK) return@ask done(found, first)
            ask(BillingClient.ProductType.SUBS) { done(wanted.mapNotNull { id -> found.firstOrNull { it.productId == id } }, it) }
        }
    }

    private fun query(c: BillingClient, done: (List<Purchase>) -> Unit) {
        val rows = ArrayList<Purchase>()
        c.queryPurchasesAsync(QueryPurchasesParams.newBuilder().setProductType(BillingClient.ProductType.INAPP).build()) { _, inapp ->
            rows.addAll(inapp)
            c.queryPurchasesAsync(QueryPurchasesParams.newBuilder().setProductType(BillingClient.ProductType.SUBS).build()) { _, subs ->
                rows.addAll(subs)
                done(rows)
            }
        }
    }

    private fun purchase(c: BillingClient, args: JSONObject, reply: (JSONObject) -> Unit) {
        refuseAccount(args)?.let { return reply(it) }
        val activity = context.activity ?: return reply(error("NotAllowedError", "no activity in front"))
        if (flowReply != null) return reply(error("NotAllowedError", "a purchase sheet is showing"))
        val itemId = args.optString("itemId")
        val words = args.optJSONObject("android") ?: JSONObject()
        products(c, JSONArray().put(itemId)) { found, result ->
            val product = found.firstOrNull() ?: return@products reply(if (result.responseCode == BillingClient.BillingResponseCode.OK) error("ITEM_UNAVAILABLE") else failure(result))
            val params = BillingFlowParams.ProductDetailsParams.newBuilder().setProductDetails(product)
            val offers = product.subscriptionOfferDetails
            if (offers != null) {
                val token = words.optString("offerToken").ifEmpty { null } ?: offerFor(offers, words.optString("basePlanId").ifEmpty { null })
                    ?: return@products reply(error("ITEM_UNAVAILABLE", "names a base plan: $itemId has several, so android.basePlanId chooses one"))
                params.setOfferToken(token)
            }
            val b = BillingFlowParams.newBuilder().setProductDetailsParamsList(listOf(params.build()))
            args.optString("accountToken").ifEmpty { null }?.let { b.setObfuscatedAccountId(it) }
            words.optString("obfuscatedProfileId").ifEmpty { null }?.let { b.setObfuscatedProfileId(it) }
            if (words.has("isOfferPersonalized")) b.setIsOfferPersonalized(words.optBoolean("isOfferPersonalized"))
            words.optString("oldPurchaseToken").ifEmpty { null }?.let { old ->
                val mode = when (words.optString("replacementMode")) {
                    "chargeProratedPrice" -> BillingFlowParams.SubscriptionUpdateParams.ReplacementMode.CHARGE_PRORATED_PRICE
                    "chargeFullPrice" -> BillingFlowParams.SubscriptionUpdateParams.ReplacementMode.CHARGE_FULL_PRICE
                    "withoutProration" -> BillingFlowParams.SubscriptionUpdateParams.ReplacementMode.WITHOUT_PRORATION
                    "deferred" -> BillingFlowParams.SubscriptionUpdateParams.ReplacementMode.DEFERRED
                    else -> BillingFlowParams.SubscriptionUpdateParams.ReplacementMode.WITH_TIME_PRORATION
                }
                b.setSubscriptionUpdateParams(BillingFlowParams.SubscriptionUpdateParams.newBuilder().setOldPurchaseToken(old).setSubscriptionReplacementMode(mode).build())
            }
            flow?.invoke(b)
            flowReply = reply
            flowItem = itemId
            val launched = c.launchBillingFlow(activity, b.build())
            if (launched.responseCode != BillingClient.BillingResponseCode.OK) { flowReply = null; reply(failure(launched)) }
        }
    }

    /** D2: the base plan's offer with a free trial or introductory phase if Play lists one, else the base plan; a
     * product with several base plans needs `android.basePlanId`. */
    private fun offerFor(offers: List<ProductDetails.SubscriptionOfferDetails>, basePlan: String?): String? {
        val plans = offers.map { it.basePlanId }.distinct()
        val plan = basePlan ?: plans.singleOrNull() ?: return null
        val of = offers.filter { it.basePlanId == plan }
        val intro = of.firstOrNull { o -> o.offerId != null && o.pricingPhases.pricingPhaseList.size > 1 }
        return (intro ?: of.firstOrNull { it.offerId == null } ?: of.firstOrNull())?.offerToken
    }

    private fun item(p: ProductDetails): JSONObject {
        val subs = p.subscriptionOfferDetails
        val out = JSONObject().put("itemId", p.productId).put("title", p.name).put("description", p.description)
            .put("iconURLs", JSONArray()).put("familyShareable", false).put("apple", JSONObject.NULL)
            .put("freeTrialPeriod", JSONObject.NULL).put("introductoryPricePeriod", JSONObject.NULL)
            .put("introductoryPrice", JSONObject.NULL).put("introductoryPriceCycles", JSONObject.NULL).put("subscriptionPeriod", JSONObject.NULL)
        val android = JSONObject().put("productType", if (subs != null) "subs" else "inapp").put("subscriptionOfferDetails", JSONObject.NULL)
        fun price(micros: Long, currency: String) = JSONObject().put("currency", currency).put("value", java.math.BigDecimal.valueOf(micros, 6).stripTrailingZeros().toPlainString())
        if (subs == null) {
            val one = p.oneTimePurchaseOfferDetails
            out.put("type", "product").put("price", price(one?.priceAmountMicros ?: 0, one?.priceCurrencyCode ?: "")).put("displayPrice", one?.formattedPrice ?: "")
        } else {
            val base = subs.firstOrNull { it.offerId == null } ?: subs.first()
            val last = base.pricingPhases.pricingPhaseList.last()
            out.put("type", "subscription").put("price", price(last.priceAmountMicros, last.priceCurrencyCode)).put("displayPrice", last.formattedPrice)
                .put("subscriptionPeriod", last.billingPeriod)
            val intro = subs.firstOrNull { it.offerId != null && it.pricingPhases.pricingPhaseList.size > 1 }?.pricingPhases?.pricingPhaseList?.first()
            if (intro != null && intro.priceAmountMicros == 0L) out.put("freeTrialPeriod", intro.billingPeriod)
            else if (intro != null) out.put("introductoryPricePeriod", intro.billingPeriod).put("introductoryPrice", price(intro.priceAmountMicros, intro.priceCurrencyCode))
                .put("introductoryPriceCycles", intro.billingCycleCount)
            android.put("subscriptionOfferDetails", JSONArray(subs.map { JSONObject().put("basePlanId", it.basePlanId).put("offerId", it.offerId ?: JSONObject.NULL).put("offerToken", it.offerToken) }))
        }
        return out.put("android", android)
    }

    private fun details(p: Purchase): JSONObject = JSONObject().put("itemId", p.products.firstOrNull() ?: "").put("purchaseToken", p.purchaseToken)
        .put("purchaseState", if (p.purchaseState == Purchase.PurchaseState.PENDING) "pending" else "purchased").put("purchaseTime", p.purchaseTime)
        .put("acknowledged", p.isAcknowledged).put("willAutoRenew", p.isAutoRenewing).put("quantity", p.quantity).put("expirationTime", JSONObject.NULL)
        .put("apple", JSONObject.NULL).put("android", JSONObject().put("orderId", p.orderId ?: JSONObject.NULL).put("originalJson", p.originalJson)
            .put("signature", p.signature).put("packageName", p.packageName).put("isAutoRenewing", p.isAutoRenewing).put("products", JSONArray(p.products)))
}
