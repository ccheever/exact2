// @ref LLP 1096 D8, LLP 1125 D10. The sound arm's mixer: a C render
// callback, so the realtime thread runs no Swift
// (AVAudioSourceNodeRenderBlockRealtimeSafe is Swift-unavailable), with no
// allocation and no lock in it. Swift pushes `Play` and `End` ops onto a
// single-producer, single-consumer ring; each render applies every op in the
// ring, then sums every live voice from its first frame to its end frame,
// times its gain. A voice reads its sound at its playback rate, from a
// fractional position by linear interpolation, and a panned one is placed by
// the Web Audio StereoPannerNode's equal-power algorithm. Times are host
// seconds on CACurrentMediaTime's timebase: when the voice should reach the
// speaker.
#ifndef EXACT_SOUND_RENDER_H
#define EXACT_SOUND_RENDER_H

#include <stdint.h>
#include <stdbool.h>
#include <CoreAudioTypes/CoreAudioTypes.h>

/// Voices in slots at once: the runner bounds the voices sounding at any
/// start to 32 and those waiting to 256 (LLP 1125 D3). A Play that finds
/// no free slot takes one whose voice ends inside the buffer, that voice's
/// remaining frames mixed first, so retriggers cut within one buffer need
/// no more.
#define EXACT_SOUND_SLOTS 288
/// Ops the ring holds: a second's lookahead of plays, and their ends.
#define EXACT_SOUND_RING 1024

typedef struct ExactMixer ExactMixer;

/// A mixer for `channels` deinterleaved float channels at `rate` frames a second.
ExactMixer *exact_sound_mixer(uint32_t channels, double rate);
void exact_sound_mixer_free(ExactMixer *mixer);
/// Sound `index`'s samples, converted to the output's rate: planes of
/// `frames` floats, one after another, one per output channel, except that a
/// stereo file for a mono output keeps its two (the mixer down-mixes them,
/// after a pan). `source` is the file's own channel count (1 or 2), which
/// decides how a pan places it. The caller keeps the planes alive.
void exact_sound_set(ExactMixer *mixer, uint32_t index, const float *planes, uint32_t frames, uint32_t source);
/// Push an op: kind 1 plays `sound` from `at` times `gain`, read at `rate`
/// (Web Audio's `playbackRate`, 1 the file's own) and placed at `pan` (−1
/// left to 1 right; 0 is the voice as it was, unpanned), on a mono output
/// panned in stereo and then down-mixed as Web Audio does; kind 2 ends voice
/// `id` at `at` (an end for a slot already freed is ignored; one at or
/// before a voice's start that has not started frees its slot); kind 3 ends
/// every voice now. False when the ring is full: the op is dropped and counted.
bool exact_sound_push(ExactMixer *mixer, uint32_t kind, uint64_t id, uint32_t sound, double at, float gain, float rate, float pan);
/// Ops the ring has room for now: a producer that waits for room never
/// counts a drop.
uint32_t exact_sound_room(const ExactMixer *mixer);
/// The source node's `outputPresentationLatency`: the delay from a buffer's
/// render time (its `mHostTime`) to the speaker (LLP 1096 D6).
void exact_sound_latency(ExactMixer *mixer, double seconds);
/// Ops the ring dropped, plays no slot took, and voices started late (their
/// first frame already past), since the mixer was made.
uint64_t exact_sound_dropped(const ExactMixer *mixer);
uint64_t exact_sound_late(const ExactMixer *mixer);
/// Voices in slots now.
uint32_t exact_sound_live(const ExactMixer *mixer);
/// Host ticks (`mHostTime`, mach_absolute_time) as seconds on
/// CACurrentMediaTime's timebase.
double exact_sound_seconds(uint64_t host);
/// Fill `out` with `frames` frames: the render callback. False when every
/// frame is silence (the node's `isSilence`).
bool exact_sound_render(ExactMixer *mixer, const AudioTimeStamp *timestamp, uint32_t frames, AudioBufferList *out);

#ifdef __OBJC__
// The arm's AVAudioEngine calls (sound_engine.m). AVAudioEngine raises
// Objective-C exceptions, rather than throwing, for a graph it cannot make;
// an exception must not unwind through a Swift frame, so each call that can
// raise sits directly inside its own @try here, and a failure comes back as
// nil or false with why in `reason` (`size` bytes, NUL-terminated).
#import <AVFAudio/AVFAudio.h>
#include <stddef.h>
/// A new engine, and its output's rate and channel count.
AVAudioEngine *exact_sound_engine_new(double *rate, uint32_t *channels, char *reason, size_t size);
/// A source node rendering `mixer`, attached and connected to the main mixer
/// in the output's format, and the engine prepared.
AVAudioSourceNode *exact_sound_engine_connect(AVAudioEngine *engine, ExactMixer *mixer, double rate, uint32_t channels, char *reason, size_t size);
bool exact_sound_engine_start(AVAudioEngine *engine, char *reason, size_t size);
bool exact_sound_engine_pause(AVAudioEngine *engine, char *reason, size_t size);
/// False when even stopping raised.
bool exact_sound_engine_stop(AVAudioEngine *engine);
/// Stops and releases an engine and its source node, each a +1 handed over,
/// whatever raises: an orphan after a media services reset (QA1749).
void exact_sound_engine_dispose(void *engine, void *source);
/// A tap on the main mixer's output (the device check).
bool exact_sound_engine_tap(AVAudioEngine *engine, void (^block)(AVAudioPCMBuffer *, AVAudioTime *), char *reason, size_t size);
#endif

#endif
