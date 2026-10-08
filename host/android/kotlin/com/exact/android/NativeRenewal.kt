package com.exact.android

import android.view.View
import android.view.accessibility.AccessibilityNodeInfo

/** Reset the SDK carrier, never its authored content or logical layout.
 * Stateful/native/navigation rows stay on fresh mounts through C9 admission;
 * the retained tree is checked again before a renewal mutates any owner.
 */
internal object NativeRenewal {
    fun requireSupported(kind: String, interacting: Boolean, navigationOwned: Boolean) {
        require(kind == "view" || kind == "text" || kind == "button" || kind == "image") {
            "Android row renewal cannot reset '$kind'"
        }
        require(!interacting) { "Android row renewal cannot replace an interacting owner" }
        require(!navigationOwned) { "Android row renewal cannot reset native navigation" }
    }
    fun reset(view: View) {
        view.cancelPendingInputEvents()
        view.clearAnimation(); view.animate().cancel(); view.setAnimationMatrix(null)
        view.isPressed = false; view.isHovered = false; view.isSelected = false; view.isActivated = false
        view.clearFocus()
        view.performAccessibilityAction(AccessibilityNodeInfo.ACTION_CLEAR_ACCESSIBILITY_FOCUS, null)
        view.translationX = 0f; view.translationY = 0f; view.translationZ = 0f
        view.scaleX = 1f; view.scaleY = 1f; view.rotation = 0f; view.rotationX = 0f; view.rotationY = 0f
        view.alpha = 1f; view.visibility = View.VISIBLE
        view.contentDescription = null; view.tag = null
        view.isEnabled = true; view.importantForAccessibility = View.IMPORTANT_FOR_ACCESSIBILITY_AUTO
        view.setOnClickListener(null); view.setOnHoverListener(null); view.setOnKeyListener(null)
        view.jumpDrawablesToCurrentState()
    }
}
