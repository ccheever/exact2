// Google Pay's callbacks while its sheet is up (LLP 1133.034 D5): the service the manifest declares, which Google binds
// with `BIND_PAYMENTS_CALLBACK_SERVICE` for a request whose `callbackIntents` name shipping or authorization. It hands
// each callback to the module whose sheet is up, on the main thread, and answers Google with the module's JSON.
package exact.payments

import com.google.android.gms.wallet.PaymentData
import com.google.android.gms.wallet.callback.BasePaymentDataCallbacks
import com.google.android.gms.wallet.callback.BasePaymentDataCallbacksService
import com.google.android.gms.wallet.callback.IntermediatePaymentData
import com.google.android.gms.wallet.callback.OnCompleteListener
import com.google.android.gms.wallet.callback.PaymentAuthorizationResult
import com.google.android.gms.wallet.callback.PaymentDataRequestUpdate
import org.json.JSONObject

class PaymentCallbacks : BasePaymentDataCallbacksService() {
    override fun createPaymentDataCallbacks(): BasePaymentDataCallbacks = object : BasePaymentDataCallbacks() {
        // Play services' Java signatures leave the data nullable.
        override fun onPaymentDataChanged(data: IntermediatePaymentData?, listener: OnCompleteListener<PaymentDataRequestUpdate>) {
            val module = PaymentsModule.current
            if (module == null || data == null) return listener.complete(PaymentDataRequestUpdate.fromJson("{}"))
            module.changed(JSONObject(data.toJson())) { listener.complete(PaymentDataRequestUpdate.fromJson(it)) }
        }

        override fun onPaymentAuthorized(data: PaymentData?, listener: OnCompleteListener<PaymentAuthorizationResult>) {
            val module = PaymentsModule.current
            if (module == null || data == null) return listener.complete(PaymentAuthorizationResult.fromJson("""{"transactionState":"ERROR","error":{"reason":"OTHER_ERROR","message":"This order was closed.","intent":"PAYMENT_AUTHORIZATION"}}"""))
            module.authorized(JSONObject(data.toJson())) { listener.complete(PaymentAuthorizationResult.fromJson(it)) }
        }
    }
}
