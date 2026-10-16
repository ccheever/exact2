// @ref LLP 1125 D8. A single-producer, single-consumer ring of MIDI 1.0
// input messages: CoreMIDI's receive block, on its realtime thread, pushes;
// the arm's own thread pops. Neither side allocates or takes a lock, so the
// receive block never waits on the main thread or the allocator. A message
// that finds the ring full is counted and dropped (4,096 messages arrive
// between two drains only from a flood).
#include <stdint.h>

#define EXACT_MIDI_RING 4096

typedef struct {
    int32_t uid;
    uint8_t count;
    uint8_t bytes[3];
    double at;
} exact_midi_in;

typedef struct {
    exact_midi_in slots[EXACT_MIDI_RING];
    long head;
    long tail;
    long dropped;
    long signaled;
} exact_midi_ring;

/// Push one message (the producer); false when the ring is full.
static inline int exact_midi_ring_push(exact_midi_ring *r, int32_t uid, double at, uint8_t b0, uint8_t b1, uint8_t b2, uint8_t count) {
    long head = __atomic_load_n(&r->head, __ATOMIC_RELAXED), tail = __atomic_load_n(&r->tail, __ATOMIC_ACQUIRE);
    if (head - tail >= EXACT_MIDI_RING) {
        __atomic_fetch_add(&r->dropped, 1, __ATOMIC_RELAXED);
        return 0;
    }
    exact_midi_in *s = &r->slots[head % EXACT_MIDI_RING];
    s->uid = uid;
    s->at = at;
    s->count = count;
    s->bytes[0] = b0;
    s->bytes[1] = b1;
    s->bytes[2] = b2;
    __atomic_store_n(&r->head, head + 1, __ATOMIC_RELEASE);
    return 1;
}

/// Whether the producer should wake the consumer: the first push since the
/// consumer last looked.
static inline int exact_midi_ring_wake(exact_midi_ring *r) {
    return __atomic_exchange_n(&r->signaled, 1, __ATOMIC_ACQ_REL) == 0;
}

/// The consumer is about to look: a later push wakes it again.
static inline void exact_midi_ring_looked(exact_midi_ring *r) {
    __atomic_store_n(&r->signaled, 0, __ATOMIC_RELEASE);
}

/// Pop one message (the consumer); false when the ring is empty.
static inline int exact_midi_ring_pop(exact_midi_ring *r, exact_midi_in *out) {
    long tail = __atomic_load_n(&r->tail, __ATOMIC_RELAXED), head = __atomic_load_n(&r->head, __ATOMIC_ACQUIRE);
    if (tail == head) return 0;
    *out = r->slots[tail % EXACT_MIDI_RING];
    __atomic_store_n(&r->tail, tail + 1, __ATOMIC_RELEASE);
    return 1;
}

/// The messages dropped since the last call.
static inline long exact_midi_ring_dropped(exact_midi_ring *r) {
    return __atomic_exchange_n(&r->dropped, 0, __ATOMIC_RELAXED);
}
