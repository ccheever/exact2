// @ref LLP 1096 D8. The sound arm's mixer: a C render callback, so the
// realtime thread runs no Swift (AVAudioSourceNodeRenderBlockRealtimeSafe is
// Swift-unavailable), with no allocation and no lock in it. Swift pushes
// `Play` and `End` ops onto a single-producer, single-consumer ring; each
// render applies every op in the ring, then sums every live voice from its
// first frame to its end frame, times its gain. Times are host seconds on
// CACurrentMediaTime's timebase: when the voice should reach the speaker.
#ifndef EXACT_SOUND_RENDER_H
#define EXACT_SOUND_RENDER_H

#include <stdint.h>
#include <stdbool.h>
#include <CoreAudioTypes/CoreAudioTypes.h>

/// Voices sounding or waiting at once (LLP 1096 §9 Q2).
#define EXACT_SOUND_SLOTS 32
/// Ops the ring holds: room for 32 plays and 32 ends several times over.
#define EXACT_SOUND_RING 256

typedef struct ExactMixer ExactMixer;

/// A mixer for `channels` deinterleaved float channels at `rate` frames a second.
ExactMixer *exact_sound_mixer(uint32_t channels, double rate);
void exact_sound_mixer_free(ExactMixer *mixer);
/// Sound `index`'s samples, converted to the output format: `channels`
/// planes of `frames` floats, one after another. The caller keeps them alive.
void exact_sound_set(ExactMixer *mixer, uint32_t index, const float *planes, uint32_t frames);
/// Push an op: kind 1 plays `sound` from `at` times `gain`; kind 2 ends voice
/// `id` at `at` (an end for a slot already freed is ignored); kind 3 ends
/// every voice now. False when the ring is full: the op is dropped and counted.
bool exact_sound_push(ExactMixer *mixer, uint32_t kind, uint64_t id, uint32_t sound, double at, float gain);
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

#endif
