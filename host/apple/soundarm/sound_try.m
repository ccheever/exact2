// @ref LLP 1096 D8. See `exact_sound_try` in sound_render.h.
#import <Foundation/Foundation.h>
#include "sound_render.h"

bool exact_sound_try(void (^body)(void), char *reason, size_t size) {
    @try {
        body();
        return true;
    } @catch (NSException *exception) {
        snprintf(reason, size, "%s: %s", exception.name.UTF8String, exception.reason.UTF8String ?: "no reason given");
        return false;
    }
}
