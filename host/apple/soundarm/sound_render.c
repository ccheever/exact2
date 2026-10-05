// @ref LLP 1096 D6, D8. See sound_render.h.
#include "sound_render.h"
#include <math.h>
#include <stdatomic.h>
#include <stdlib.h>
#include <string.h>
#include <mach/mach_time.h>

typedef struct { uint32_t kind; uint32_t sound; uint64_t id; double at; float gain; } Op;

typedef struct {
    bool live, started;
    uint64_t id;
    uint32_t sound;
    double start, end;   // host seconds at the speaker; `end` infinite until an End
    float gain;
    uint32_t position;   // frames of its sound already played
} Voice;

typedef struct { const float *planes; uint32_t frames; } Sound;

struct ExactMixer {
    uint32_t channels;
    double rate;
    _Atomic double latency;
    Op ring[EXACT_SOUND_RING];
    _Atomic uint32_t head, tail;
    Voice slots[EXACT_SOUND_SLOTS];
    Sound *sounds;
    uint32_t count;
    _Atomic uint64_t dropped, late;
    _Atomic uint32_t live;
};

ExactMixer *exact_sound_mixer(uint32_t channels, double rate) {
    ExactMixer *m = calloc(1, sizeof(ExactMixer));
    if (!m) return NULL;
    m->channels = channels;
    m->rate = rate;
    return m;
}

void exact_sound_mixer_free(ExactMixer *m) {
    if (!m) return;
    free(m->sounds);
    free(m);
}

void exact_sound_set(ExactMixer *m, uint32_t index, const float *planes, uint32_t frames) {
    // Called before the engine starts (or while it is stopped): not on the render thread.
    if (index >= m->count) {
        Sound *grown = realloc(m->sounds, (index + 1) * sizeof(Sound));
        if (!grown) return;
        memset(grown + m->count, 0, (index + 1 - m->count) * sizeof(Sound));
        m->sounds = grown;
        m->count = index + 1;
    }
    m->sounds[index] = (Sound){ planes, frames };
}

bool exact_sound_push(ExactMixer *m, uint32_t kind, uint64_t id, uint32_t sound, double at, float gain) {
    uint32_t tail = atomic_load_explicit(&m->tail, memory_order_relaxed);
    uint32_t head = atomic_load_explicit(&m->head, memory_order_acquire);
    if (tail - head >= EXACT_SOUND_RING) {
        atomic_fetch_add_explicit(&m->dropped, 1, memory_order_relaxed);
        return false;
    }
    m->ring[tail % EXACT_SOUND_RING] = (Op){ kind, sound, id, at, gain };
    atomic_store_explicit(&m->tail, tail + 1, memory_order_release);
    return true;
}

void exact_sound_latency(ExactMixer *m, double seconds) { atomic_store_explicit(&m->latency, seconds, memory_order_relaxed); }
uint64_t exact_sound_dropped(const ExactMixer *m) { return atomic_load_explicit(&m->dropped, memory_order_relaxed); }
uint64_t exact_sound_late(const ExactMixer *m) { return atomic_load_explicit(&m->late, memory_order_relaxed); }
uint32_t exact_sound_live(const ExactMixer *m) { return atomic_load_explicit(&m->live, memory_order_relaxed); }

/// Every op in the ring, in order: a Play and an End for the same buffer both take effect in it.
static void apply(ExactMixer *m) {
    uint32_t head = atomic_load_explicit(&m->head, memory_order_relaxed);
    uint32_t tail = atomic_load_explicit(&m->tail, memory_order_acquire);
    for (; head != tail; head++) {
        Op op = m->ring[head % EXACT_SOUND_RING];
        if (op.kind == 1) {
            Voice *free_slot = NULL;
            for (int i = 0; i < EXACT_SOUND_SLOTS && !free_slot; i++) if (!m->slots[i].live) free_slot = &m->slots[i];
            if (!free_slot || op.sound >= m->count || !m->sounds[op.sound].planes) {
                atomic_fetch_add_explicit(&m->dropped, 1, memory_order_relaxed);
                continue;
            }
            *free_slot = (Voice){ true, false, op.id, op.sound, op.at, INFINITY, op.gain, 0 };
        } else if (op.kind == 2) {
            for (int i = 0; i < EXACT_SOUND_SLOTS; i++) {
                Voice *v = &m->slots[i];
                if (v->live && v->id == op.id && op.at < v->end) v->end = op.at;
            }
        } else if (op.kind == 3) {
            for (int i = 0; i < EXACT_SOUND_SLOTS; i++) m->slots[i].live = false;
        }
    }
    atomic_store_explicit(&m->head, head, memory_order_release);
}

/// Host ticks (mach_absolute_time, CACurrentMediaTime's base) as seconds.
double exact_sound_seconds(uint64_t host) {
    static mach_timebase_info_data_t base;
    if (base.denom == 0) mach_timebase_info(&base);
    return (double)host * base.numer / base.denom / 1e9;
}

bool exact_sound_render(ExactMixer *m, const AudioTimeStamp *ts, uint32_t frames, AudioBufferList *out) {
    apply(m);
    bool interleaved = out->mNumberBuffers == 1 && m->channels > 1;
    for (uint32_t b = 0; b < out->mNumberBuffers; b++) memset(out->mBuffers[b].mData, 0, out->mBuffers[b].mDataByteSize);
    // When this buffer's first frame reaches the speaker (D6): its render
    // time plus the delay downstream of the node.
    double rendered = (ts && (ts->mFlags & kAudioTimeStampHostTimeValid)) ? exact_sound_seconds(ts->mHostTime) : 0;
    double speaker = rendered + atomic_load_explicit(&m->latency, memory_order_relaxed);
    bool sounded = false;
    uint32_t live = 0;
    for (int s = 0; s < EXACT_SOUND_SLOTS; s++) {
        Voice *v = &m->slots[s];
        if (!v->live) continue;
        const Sound *sound = &m->sounds[v->sound];
        int64_t begin = 0;
        if (!v->started) {
            int64_t first = llround((v->start - speaker) * m->rate);
            if (first >= (int64_t)frames) { live++; continue; }
            // A voice the mapping puts in the past starts here, from its first
            // frame: its attack is never skipped.
            if (first < 0) { first = 0; atomic_fetch_add_explicit(&m->late, 1, memory_order_relaxed); }
            begin = first;
        }
        int64_t stop = frames;
        if (isfinite(v->end)) {
            int64_t endFrame = llround((v->end - speaker) * m->rate);
            if (endFrame < stop) stop = endFrame;
        }
        // Cut before it sounded here, or ended in an earlier buffer.
        if (stop <= begin) { v->live = false; continue; }
        v->started = true;
        for (int64_t i = begin; i < stop && v->position < sound->frames; i++, v->position++) {
            for (uint32_t c = 0; c < m->channels; c++) {
                float x = sound->planes[(size_t)c * sound->frames + v->position] * v->gain;
                if (interleaved) ((float *)out->mBuffers[0].mData)[i * m->channels + c] += x;
                else if (c < out->mNumberBuffers) ((float *)out->mBuffers[c].mData)[i] += x;
            }
            sounded = true;
        }
        // A slot frees at its end: the file's, or an End's.
        if (v->position >= sound->frames || stop < (int64_t)frames) v->live = false;
        else live++;
    }
    atomic_store_explicit(&m->live, live, memory_order_relaxed);
    return sounded;
}
