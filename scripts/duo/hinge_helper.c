// hinge_helper.c — sets the iPhone Duo simulator's hinge angle from inside the
// simulator (`xcrun simctl spawn <udid> hinge_helper set <degrees>`), by posting
// the vendor-defined HID event the hidden hinge slider in Xcode's Device Hub sends.
// `bun scripts/smoke.mjs duo` compiles it per run with the selected Xcode's
// simulator SDK (LLP 1076 D9).
//
// From artemnovichkov/hinge (https://github.com/artemnovichkov/hinge), MIT
// License, Copyright (c) 2026 Artem Novichkov. Permission is hereby granted,
// free of charge, to any person obtaining a copy of this software and
// associated documentation files (the "Software"), to deal in the Software
// without restriction, including without limitation the rights to use, copy,
// modify, merge, publish, distribute, sublicense, and/or sell copies of the
// Software, and to permit persons to whom the Software is furnished to do so,
// subject to the following conditions: The above copyright notice and this
// permission notice shall be included in all copies or substantial portions
// of the Software. THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY
// KIND, EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF
// MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO
// EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES
// OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE,
// ARISING FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER
// DEALINGS IN THE SOFTWARE.

// Runs inside the iOS Simulator (via `xcrun simctl spawn`) and posts the same
// vendor-defined HID event that the hidden hinge slider in Xcode's Device Hub sends.
//
//   hinge_helper set <degrees>
//   hinge_helper sweep <from> <to> <seconds>

#include <CoreFoundation/CoreFoundation.h>
#include <mach/mach_time.h>
#include <math.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>

typedef struct __IOHIDEvent *IOHIDEventRef;
typedef struct __IOHIDEventSystemClient *IOHIDEventSystemClientRef;

extern IOHIDEventRef IOHIDEventCreateVendorDefinedEvent(CFAllocatorRef allocator, uint64_t timeStamp,
    uint32_t usagePage, uint32_t usage, uint32_t version, const uint8_t *data, CFIndex length, uint32_t options);
extern IOHIDEventSystemClientRef IOHIDEventSystemClientCreateWithType(CFAllocatorRef allocator, int type,
    CFDictionaryRef properties);
extern void IOHIDEventSystemClientDispatchEvent(IOHIDEventSystemClientRef client, IOHIDEventRef event);
extern CFDataRef IOCFSerialize(CFTypeRef object, CFOptionFlags options);

enum {
    kUsagePage = 0xFF61,
    kUsage = 0x5B,
    kClientTypeSimple = 4,
    kSerializeBinary = 1,
    kSweepHz = 60,
};

static void send_angle(IOHIDEventSystemClientRef client, double degrees) {
    if (degrees < 0) degrees = 0;
    if (degrees > 180) degrees = 180;

    CFNumberRef value = CFNumberCreate(NULL, kCFNumberDoubleType, &degrees);
    const void *keys[] = {CFSTR("provider"), CFSTR("source"), CFSTR("type"), CFSTR("value")};
    const void *values[] = {CFSTR("com.apple.Virtualization"), CFSTR("hinge-slider-control"), CFSTR("range"), value};
    CFDictionaryRef payload = CFDictionaryCreate(NULL, keys, values, 4,
        &kCFTypeDictionaryKeyCallBacks, &kCFTypeDictionaryValueCallBacks);
    CFDataRef data = IOCFSerialize(payload, kSerializeBinary);

    if (data) {
        IOHIDEventRef event = IOHIDEventCreateVendorDefinedEvent(NULL, mach_absolute_time(), kUsagePage, kUsage, 0,
            CFDataGetBytePtr(data), CFDataGetLength(data), 0);
        if (event) {
            IOHIDEventSystemClientDispatchEvent(client, event);
            CFRelease(event);
        }
        CFRelease(data);
    }
    CFRelease(payload);
    CFRelease(value);
}

static int usage(void) {
    fprintf(stderr, "usage: hinge_helper set <degrees>\n"
                    "       hinge_helper sweep <from> <to> <seconds>\n");
    return 64;
}

int main(int argc, char **argv) {
    if (argc < 3) return usage();

    IOHIDEventSystemClientRef client = IOHIDEventSystemClientCreateWithType(NULL, kClientTypeSimple, NULL);
    if (!client) {
        fprintf(stderr, "hinge_helper: failed to create HID event system client\n");
        return 1;
    }

    if (strcmp(argv[1], "set") == 0 && argc == 3) {
        send_angle(client, atof(argv[2]));
    } else if (strcmp(argv[1], "sweep") == 0 && argc == 5) {
        double from = atof(argv[2]), to = atof(argv[3]), seconds = fmax(atof(argv[4]), 0);
        int steps = (int)fmax(1, round(seconds * kSweepHz));
        for (int i = 0; i <= steps; i++) {
            send_angle(client, from + (to - from) * i / steps);
            if (i < steps) usleep((useconds_t)(seconds * 1e6 / steps));
        }
    } else {
        return usage();
    }

    // Give the event system a moment to deliver before the process exits.
    usleep(100000);
    return 0;
}
