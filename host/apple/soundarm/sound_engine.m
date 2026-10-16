// @ref LLP 1096 D8. See the AVAudioEngine calls in sound_render.h.
#import <AVFAudio/AVFAudio.h>
#include "sound_render.h"

#if !__has_feature(objc_arc)
#error "sound_engine.m is compiled with -fobjc-arc"
#endif

static void raised(NSException *exception, char *reason, size_t size) {
    snprintf(reason, size, "AVAudioEngine raised %s: %s", exception.name.UTF8String, exception.reason.UTF8String ?: "no reason given");
}

AVAudioEngine *exact_sound_engine_new(double *rate, uint32_t *channels, char *reason, size_t size) {
    @try {
        AVAudioEngine *engine = [[AVAudioEngine alloc] init];
        AVAudioFormat *output = [engine.outputNode outputFormatForBus:0];
        *rate = output.sampleRate;
        *channels = output.channelCount;
        return engine;
    } @catch (NSException *exception) {
        raised(exception, reason, size);
        return nil;
    }
}

AVAudioSourceNode *exact_sound_engine_connect(AVAudioEngine *engine, ExactMixer *mixer, double rate, uint32_t channels, char *reason, size_t size) {
    @try {
        AVAudioFormat *format = [[AVAudioFormat alloc] initStandardFormatWithSampleRate:rate channels:channels];
        if (!format) {
            snprintf(reason, size, "the output's format (%.0f Hz, %u channels) is not one the mixer takes", rate, channels);
            return nil;
        }
        // The C mixer does the work: no Objective-C message and no allocation per buffer.
        AVAudioSourceNode *source = [[AVAudioSourceNode alloc] initWithFormat:format renderBlock:^OSStatus(BOOL *isSilence, const AudioTimeStamp *timestamp, AVAudioFrameCount frames, AudioBufferList *out) {
            *isSilence = !exact_sound_render(mixer, timestamp, frames, out);
            return noErr;
        }];
        [engine attachNode:source];
        // The 27 SDKs' connect returns its error; the older one only raises.
        if (@available(macOS 27.0, iOS 27.0, tvOS 27.0, *)) {
            NSError *error = nil;
            if (![engine connect:source to:engine.mainMixerNode format:format error:&error]) {
                snprintf(reason, size, "the source did not connect: %s", error.localizedDescription.UTF8String ?: "no reason given");
                return nil;
            }
        } else {
#pragma clang diagnostic push
#pragma clang diagnostic ignored "-Wdeprecated-declarations"
            [engine connect:source to:engine.mainMixerNode format:format];
#pragma clang diagnostic pop
        }
        [engine prepare];
        return source;
    } @catch (NSException *exception) {
        raised(exception, reason, size);
        return nil;
    }
}

bool exact_sound_engine_start(AVAudioEngine *engine, char *reason, size_t size) {
    @try {
        NSError *error = nil;
        if ([engine startAndReturnError:&error]) return true;
        snprintf(reason, size, "the engine did not start: %s", error.localizedDescription.UTF8String ?: "no reason given");
        return false;
    } @catch (NSException *exception) {
        raised(exception, reason, size);
        return false;
    }
}

bool exact_sound_engine_pause(AVAudioEngine *engine, char *reason, size_t size) {
    @try {
        [engine pause];
        return true;
    } @catch (NSException *exception) {
        raised(exception, reason, size);
        return false;
    }
}

bool exact_sound_engine_stop(AVAudioEngine *engine) {
    @try {
        [engine stop];
        return true;
    } @catch (NSException *exception) {
        return false;
    }
}

bool exact_sound_engine_tap(AVAudioEngine *engine, void (^block)(AVAudioPCMBuffer *, AVAudioTime *), char *reason, size_t size) {
    @try {
        if (@available(macOS 27.0, iOS 27.0, tvOS 27.0, *)) {
            NSError *error = nil;
            if ([engine.mainMixerNode installTapOnBus:0 bufferSize:256 format:nil error:&error block:block]) return true;
            snprintf(reason, size, "the tap was refused: %s", error.localizedDescription.UTF8String ?: "no reason given");
            return false;
        }
#pragma clang diagnostic push
#pragma clang diagnostic ignored "-Wdeprecated-declarations"
        [engine.mainMixerNode installTapOnBus:0 bufferSize:256 format:nil block:block];
#pragma clang diagnostic pop
        return true;
    } @catch (NSException *exception) {
        raised(exception, reason, size);
        return false;
    }
}
