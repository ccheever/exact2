#include <jni.h>
#include "../include/exact_android.h"

#include <atomic>
#include <cstring>
#include <limits>
#include <memory>
#include <mutex>
#include <new>
#include <stdexcept>
#include <thread>
#include <unordered_map>
#include <vector>

namespace {
JavaVM *vm = nullptr;
jclass text_class = nullptr;
jclass state_error = nullptr;
jclass argument_error = nullptr;
jclass memory_error = nullptr;
jmethodID measure_method = nullptr;
jmethodID fonts_method = nullptr;
jmethodID wake_method = nullptr;
jmethodID position_method = nullptr;
jmethodID limit_method = nullptr;

void fail(JNIEnv *env, jclass type, const char *message) {
    if (!env->ExceptionCheck()) env->ThrowNew(type, message);
}

bool window(JNIEnv *env, jobject buffer, size_t count, uint64_t &calls) {
    ++calls;
    auto self = env->CallObjectMethod(buffer, position_method, jint(0));
    if (env->ExceptionCheck()) return false;
    env->DeleteLocalRef(self);
    ++calls;
    self = env->CallObjectMethod(buffer, limit_method, jint(count));
    if (env->ExceptionCheck()) return false;
    env->DeleteLocalRef(self);
    return true;
}

// One retained, initialized backing per callback wire. The JVM wrapper changes
// only when the backing grows. Kotlin reads it synchronously and must not keep it.
struct PackedBuffer {
    std::vector<uint8_t> bytes;
    size_t used = 0;
    jobject wrapper = nullptr;
    const uint8_t *exposed = nullptr;
    size_t exposed_size = 0;
    uint64_t allocations = 0;
    uint64_t window_calls = 0;

    void start() { used = 0; }
    void append(const void *source, size_t count) {
        constexpr auto maximum = size_t(std::numeric_limits<jint>::max());
        if (count > maximum - used)
            throw std::length_error("Android callback exceeds the wire size");
        const auto required = used + count;
        if (required > bytes.size()) {
            const auto doubled = bytes.empty() ? size_t(512) : bytes.size() * 2;
            const auto grown = doubled > maximum ? maximum : doubled;
            bytes.resize(required > grown ? required : grown);
        }
        if (count != 0) std::memcpy(bytes.data() + used, source, count);
        used = required;
    }
    void word(uint32_t value) {
        const uint8_t data[] = {uint8_t(value), uint8_t(value >> 8),
                                uint8_t(value >> 16), uint8_t(value >> 24)};
        append(data, sizeof(data));
    }
    void wide(uint64_t value) {
        word(uint32_t(value));
        word(uint32_t(value >> 32));
    }
    void number(float value) {
        uint32_t bits;
        static_assert(sizeof(bits) == sizeof(value));
        std::memcpy(&bits, &value, sizeof(bits));
        word(bits);
    }
    void text(const uint8_t *source, size_t count) {
        if (count > std::numeric_limits<uint32_t>::max())
            throw std::length_error("Android text exceeds the wire size");
        word(uint32_t(count));
        append(source, count);
    }
    jobject view(JNIEnv *env, bool caller_owns_window = false) {
        if (wrapper && (exposed != bytes.data() || exposed_size != bytes.size())) {
            env->DeleteGlobalRef(wrapper);
            wrapper = nullptr;
        }
        if (!wrapper) {
            const auto local = env->NewDirectByteBuffer(bytes.data(), jlong(bytes.size()));
            if (!local) return nullptr;
            ++allocations;
            wrapper = env->NewGlobalRef(local);
            env->DeleteLocalRef(local);
            if (!wrapper) return nullptr;
            exposed = bytes.data();
            exposed_size = bytes.size();
        }
        // Paragraph measurement receives its used length in the same Java
        // call. That owner resets the window locally, avoiding two upcalls.
        if (caller_owns_window) return wrapper;
        return window(env, wrapper, used, window_calls) ? wrapper : nullptr;
    }
    void release(JNIEnv *env) {
        if (wrapper) env->DeleteGlobalRef(wrapper);
        wrapper = nullptr;
    }
};

struct Session {
    ExactRuntime runtime = 0;
    jobject engine = nullptr;
    const std::thread::id owner = std::this_thread::get_id();
    PackedBuffer measure_wire;
    PackedBuffer fonts_wire;
    std::vector<uint8_t> scroll_input;
    jobject output_wrapper = nullptr;
    const uint8_t *output_address = nullptr;
    uint32_t output_capacity = 0;
    uint64_t transactions = 0;
    uint64_t output_bytes = 0;
    uint64_t output_allocations = 0;
    uint64_t output_window_calls = 0;
    uint64_t measure_calls = 0;
    uint64_t font_calls = 0;
    std::atomic<uint64_t> wake_calls{0};
    std::mutex wake_mutex;
    jthrowable wake_exception = nullptr;
    bool wake_failed = false;

    void release(JNIEnv *env) {
        measure_wire.release(env);
        fonts_wire.release(env);
        if (output_wrapper) env->DeleteGlobalRef(output_wrapper);
        if (wake_exception) env->DeleteGlobalRef(wake_exception);
        if (engine) env->DeleteGlobalRef(engine);
    }
};

// Rust handles are process-unique and thread-owned. Looking up a handle on
// another thread fails before entering the Rust registry, including on close.
thread_local std::unordered_map<jlong, std::unique_ptr<Session>> sessions;

Session *session(JNIEnv *env, jlong handle) {
    const auto found = sessions.find(handle);
    if (found == sessions.end()) {
        fail(env, state_error, "Exact session is closed or belongs to another thread");
        return nullptr;
    }
    auto *value = found->second.get();
    std::lock_guard<std::mutex> guard(value->wake_mutex);
    if (value->wake_exception) {
        env->Throw(value->wake_exception);
        env->DeleteGlobalRef(value->wake_exception);
        value->wake_exception = nullptr;
        return nullptr;
    }
    if (value->wake_failed) {
        value->wake_failed = false;
        fail(env, state_error, "Android could not enqueue an Exact completion");
        return nullptr;
    }
    return value;
}

template <typename T, typename F> T boundary(JNIEnv *env, T fallback, F operation) {
    if (env->ExceptionCheck()) return fallback;
    try {
        return operation();
    } catch (const std::bad_alloc &) {
        fail(env, memory_error, "Exact Android allocation failed");
    } catch (const std::exception &error) {
        fail(env, state_error, error.what());
    } catch (...) {
        fail(env, state_error, "Exact Android native operation failed");
    }
    return fallback;
}

void style(PackedBuffer &wire, const ExactTextRun &run) {
    wire.number(run.font_size);
    wire.word(run.font_weight);
    wire.word(run.font_family);
    wire.word(run.italic);
    wire.word(run.has_line_height);
    wire.number(run.line_height);
    wire.number(run.letter_spacing);
    wire.word(run.font_variant_numeric);
}

float read_number(const uint8_t *bytes) {
    const uint32_t bits = uint32_t(bytes[0]) | (uint32_t(bytes[1]) << 8) |
                          (uint32_t(bytes[2]) << 16) | (uint32_t(bytes[3]) << 24);
    float value;
    std::memcpy(&value, &bits, sizeof(value));
    return value;
}

ExactMetrics measure(void *context, const ExactMeasureRequest *request) {
    auto &value = *static_cast<Session *>(context);
    JNIEnv *env = nullptr;
    const ExactMetrics invalid{0, 0, std::numeric_limits<float>::quiet_NaN()};
    if (value.owner != std::this_thread::get_id() ||
        vm->GetEnv(reinterpret_cast<void **>(&env), JNI_VERSION_1_6) != JNI_OK)
        return invalid;
    return boundary(env, invalid, [&]() {
        ++value.measure_calls;
        if (request->text_indent != 0 || request->hyphens > 1)
            throw std::invalid_argument("Android C9 text-indent and automatic hyphenation are not implemented");
        if (request->count > std::numeric_limits<uint32_t>::max() ||
            request->exclusion_count > std::numeric_limits<uint32_t>::max())
            throw std::length_error("Android paragraph exceeds the wire size");
        auto &wire = value.measure_wire;
        wire.start();
        // v1: header 64B, strut style 32B, then runs (style32, length, UTF-8),
        // then exclusions (kind, x/y/a/b/radius, pair count, x/y pairs).
        wire.word(1);
        wire.word(request->view);
        wire.word(request->node_index);
        wire.word(request->node_generation);
        wire.wide(request->revision);
        wire.number(request->width);
        wire.number(request->height);
        wire.word(request->align);
        wire.word(request->line_clamp);
        wire.word(request->overflow_wrap);
        wire.word(request->white_space);
        wire.word(request->direction);
        wire.word(request->markup);
        wire.word(uint32_t(request->count));
        wire.word(uint32_t(request->exclusion_count));
        style(wire, request->strut);
        for (size_t index = 0; index < request->count; ++index) {
            const auto &run = request->runs[index];
            style(wire, run);
            wire.text(run.text, run.len);
        }
        for (size_t index = 0; index < request->exclusion_count; ++index) {
            const auto &shape = request->exclusions[index];
            if (shape.count > std::numeric_limits<uint32_t>::max())
                throw std::length_error("Android exclusion exceeds the wire size");
            wire.word(shape.kind);
            wire.number(shape.x);
            wire.number(shape.y);
            wire.number(shape.a);
            wire.number(shape.b);
            wire.number(shape.radius);
            wire.word(uint32_t(shape.count));
            for (size_t pair = 0; pair < shape.count; ++pair) {
                wire.number(shape.pairs[pair].x);
                wire.number(shape.pairs[pair].y);
            }
        }
        const auto input = wire.view(env, true);
        if (!input) return invalid;
        const auto output = env->CallObjectMethod(value.engine, measure_method, input, jint(wire.used));
        if (env->ExceptionCheck() || !output) return invalid;
        const auto bytes = static_cast<const uint8_t *>(env->GetDirectBufferAddress(output));
        const auto count = env->GetDirectBufferCapacity(output);
        ExactMetrics result = invalid;
        if (bytes && count >= 12) {
            result = {read_number(bytes), read_number(bytes + 4), read_number(bytes + 8)};
        } else {
            fail(env, state_error, "TextEngine.measure must return three direct little-endian floats");
        }
        env->DeleteLocalRef(output);
        return result;
    });
}

void fonts(void *context, const ExactFontCatalog *catalog) {
    auto &value = *static_cast<Session *>(context);
    JNIEnv *env = nullptr;
    if (value.owner != std::this_thread::get_id() ||
        vm->GetEnv(reinterpret_cast<void **>(&env), JNI_VERSION_1_6) != JNI_OK) return;
    boundary(env, false, [&]() {
        ++value.font_calls;
        if (catalog->count > std::numeric_limits<uint32_t>::max())
            throw std::length_error("Android font catalog exceeds the wire size");
        auto &wire = value.fonts_wire;
        wire.start();
        uint32_t installed = 0;
        for (size_t index = 0; index < catalog->count; ++index) {
            const auto &face = catalog->faces[index];
            const bool system_default = face.source_len == 0 && face.weight == 1 &&
                face.family_len == 9 && std::memcmp(face.family, "system-ui", 9) == 0;
            if (face.source_len == 0 && !system_default)
                throw std::invalid_argument("Android C9 local and non-system generic font members are not implemented");
            for (size_t other = 0; other < catalog->count; ++other) {
                const auto &member = catalog->faces[other];
                if (other == index || member.stack != face.stack) continue;
                if (system_default || member.source_len == 0 ||
                    member.family_len != face.family_len ||
                    std::memcmp(member.family, face.family, face.family_len) != 0)
                    throw std::invalid_argument("Android C9 ordered font fallback stacks are not implemented");
            }
            if (!system_default) ++installed;
        }
        wire.word(1);
        wire.word(installed);
        for (size_t index = 0; index < catalog->count; ++index) {
            const auto &face = catalog->faces[index];
            // C9 already renders the standalone system-ui stack with the
            // platform default; it has no packaged face to install.
            if (face.source_len == 0) continue;
            wire.word(face.stack);
            wire.word(face.weight);
            wire.word(face.italic);
            wire.text(face.family, face.family_len);
            wire.text(face.source, face.source_len);
        }
        const auto input = wire.view(env);
        if (!input) return false;
        env->CallVoidMethod(value.engine, fonts_method, input);
        return !env->ExceptionCheck();
    });
}

void wake(void *context) {
    auto &value = *static_cast<Session *>(context);
    value.wake_calls.fetch_add(1, std::memory_order_relaxed);
    JNIEnv *env = nullptr;
    bool attached = false;
    const auto status = vm->GetEnv(reinterpret_cast<void **>(&env), JNI_VERSION_1_6);
    if (status == JNI_EDETACHED) {
#ifdef __ANDROID__
        attached = vm->AttachCurrentThread(&env, nullptr) == JNI_OK;
#else
        attached = vm->AttachCurrentThread(reinterpret_cast<void **>(&env), nullptr) == JNI_OK;
#endif
        if (!attached) {
            std::lock_guard<std::mutex> guard(value.wake_mutex);
            value.wake_failed = true;
            return;
        }
    } else if (status != JNI_OK) {
        std::lock_guard<std::mutex> guard(value.wake_mutex);
        value.wake_failed = true;
        return;
    }
    // Boot can already carry a measurement/font exception. CheckJNI forbids
    // invoking Java while that exception is pending; the owner call returns it.
    if (env->ExceptionCheck()) {
        if (attached) vm->DetachCurrentThread();
        return;
    }
    // The callback only posts to the UI loop. Rust invokes it under the
    // executor's retirement guard, so it must never synchronously reenter Rust.
    env->CallVoidMethod(value.engine, wake_method);
    if (env->ExceptionCheck() && value.owner != std::this_thread::get_id()) {
        const auto error = env->ExceptionOccurred();
        env->ExceptionClear();
        const auto retained = static_cast<jthrowable>(env->NewGlobalRef(error));
        env->DeleteLocalRef(error);
        {
            std::lock_guard<std::mutex> guard(value.wake_mutex);
            if (retained && !value.wake_exception) value.wake_exception = retained;
            else if (retained) env->DeleteGlobalRef(retained);
            else value.wake_failed = true;
        }
        if (env->ExceptionCheck()) env->ExceptionClear();
    }
    if (attached) vm->DetachCurrentThread();
}

bool input(JNIEnv *env, Session &value, jbyteArray payload, uint32_t &count) {
    count = payload ? uint32_t(env->GetArrayLength(payload)) : 0;
    if (env->ExceptionCheck()) return false;
    auto *bytes = exact_android_in(value.runtime, count);
    if (!bytes && count != 0) {
        fail(env, state_error, "Exact Android rejected the input buffer");
        return false;
    }
    if (count != 0)
        env->GetByteArrayRegion(payload, 0, jsize(count), reinterpret_cast<jbyte *>(bytes));
    return !env->ExceptionCheck();
}

// Borrowed native output: the presenter consumes this before any next native
// transaction (including close), never retains it in a node or worker.
jobject output(JNIEnv *env, Session &value, uint32_t count) {
    if (env->ExceptionCheck()) return nullptr;
    if (count > uint32_t(std::numeric_limits<jint>::max())) {
        fail(env, state_error, "Exact Android transaction exceeds ByteBuffer capacity");
        return nullptr;
    }
    const auto *bytes = exact_android_out(value.runtime);
    static uint8_t empty = 0;
    if (!bytes && count != 0) {
        fail(env, state_error, "Exact Android returned an invalid output buffer");
        return nullptr;
    }
    bytes = bytes ? bytes : &empty;
    // The Rust output Vec retains its allocation across turns. A wrapper can
    // cover the greatest observed length at this address; limit names this turn.
    if (value.output_wrapper &&
        (value.output_address != bytes || value.output_capacity < count)) {
        env->DeleteGlobalRef(value.output_wrapper);
        value.output_wrapper = nullptr;
    }
    if (!value.output_wrapper) {
        const auto local = env->NewDirectByteBuffer(const_cast<uint8_t *>(bytes), count);
        if (!local) return nullptr;
        ++value.output_allocations;
        value.output_wrapper = env->NewGlobalRef(local);
        env->DeleteLocalRef(local);
        if (!value.output_wrapper) return nullptr;
        value.output_address = bytes;
        value.output_capacity = count;
    }
    if (!window(env, value.output_wrapper, count, value.output_window_calls)) return nullptr;
    return env->NewLocalRef(value.output_wrapper);
}

template <typename F> jobject transaction(JNIEnv *env, jlong handle, F operation) {
    return boundary(env, jobject(nullptr), [&]() -> jobject {
        auto *value = session(env, handle);
        if (!value) return nullptr;
        ++value->transactions;
        const auto count = operation(*value);
        value->output_bytes += count;
        return output(env, *value, count);
    });
}

jlong create(JNIEnv *env, jclass, jobject engine) {
    return boundary(env, jlong(0), [&]() -> jlong {
        if (!engine || !env->IsInstanceOf(engine, text_class)) {
            fail(env, argument_error, "Exact requires a TextEngine");
            return 0;
        }
        auto value = std::make_unique<Session>();
        value->engine = env->NewGlobalRef(engine);
        if (!value->engine) return 0;
        value->runtime = exact_android_create();
        if (value->runtime == 0) {
            value->release(env);
            fail(env, state_error, "Exact Android could not create a session");
            return 0;
        }
        const auto handle = jlong(value->runtime);
        auto *context = value.get();
        try {
            const auto inserted = sessions.emplace(handle, nullptr);
            inserted.first->second = std::move(value);
        } catch (...) {
            exact_android_destroy(context->runtime);
            context->release(env);
            throw;
        }
        exact_android_set_measure(context->runtime, measure, context);
        exact_android_set_fonts(context->runtime, fonts, context);
        exact_android_set_wake(context->runtime, wake, context);
        return handle;
    });
}

void close(JNIEnv *env, jclass, jlong handle) {
    boundary(env, false, [&]() {
        // Close must remain available after a callback failure. Wrong-thread
        // calls still fail before touching Rust.
        const auto found = sessions.find(handle);
        if (found == sessions.end()) {
            fail(env, state_error, "Exact session is closed or belongs to another thread");
            return false;
        }
        auto &value = *found->second;
        // Executor retirement waits for callbacks and clears its wake before
        // returning. Posted Java work checks the closed handle before pumping.
        exact_android_destroy(value.runtime);
        value.release(env);
        sessions.erase(found);
        return true;
    });
}

jobject boot(JNIEnv *env, jclass, jlong handle, jfloat width, jfloat height, jbyteArray initial_press) {
    return transaction(env, handle, [&](Session &s) {
        if (!initial_press) return exact_android_boot(s.runtime, width, height);
        uint32_t count;
        return input(env, s, initial_press, count)
            ? exact_android_boot_initial(s.runtime, width, height, count) : 0;
    });
}
jobject dispatch(JNIEnv *env, jclass, jlong handle, jint view, jint kind, jbyteArray payload, jdouble now) {
    return transaction(env, handle, [&](Session &s) {
        uint32_t count;
        return input(env, s, payload, count) ? exact_android_dispatch(s.runtime, uint32_t(view), uint32_t(kind), count, now) : 0;
    });
}
jobject resize(JNIEnv *env, jclass, jlong handle, jfloat width, jfloat height) {
    return transaction(env, handle, [&](Session &s) { return exact_android_resize(s.runtime, width, height); });
}
#define EXACT_TIME_CALL(name) \
    jobject name(JNIEnv *env, jclass, jlong handle, jdouble now) { \
        return transaction(env, handle, [&](Session &s) { return exact_android_##name(s.runtime, now); }); \
    }
EXACT_TIME_CALL(frame)
EXACT_TIME_CALL(advance)
EXACT_TIME_CALL(tick)
EXACT_TIME_CALL(pump)
#undef EXACT_TIME_CALL
jobject painted(JNIEnv *env, jclass, jlong handle) {
    return transaction(env, handle, [&](Session &s) { return exact_android_painted(s.runtime); });
}
jobject preferences(JNIEnv *env, jclass, jlong handle, jint bits) {
    return transaction(env, handle, [&](Session &s) { return exact_android_preferences(s.runtime, uint32_t(bits)); });
}
jobject insets(JNIEnv *env, jclass, jlong handle, jfloat top, jfloat right, jfloat bottom, jfloat left) {
    return transaction(env, handle, [&](Session &s) { return exact_android_insets(s.runtime, top, right, bottom, left); });
}
jobject intrinsic(JNIEnv *env, jclass, jlong handle, jint view, jfloat width, jfloat height) {
    return transaction(env, handle, [&](Session &s) { return exact_android_intrinsic(s.runtime, uint32_t(view), width, height); });
}
jobject intrinsics(JNIEnv *env, jclass, jlong handle, jbyteArray payload) {
    return transaction(env, handle, [&](Session &s) {
        uint32_t count;
        return input(env, s, payload, count) ? exact_android_intrinsics(s.runtime, count) : 0;
    });
}
jobject controlQuery(JNIEnv *env, jclass, jlong handle, jint view, jint kind) {
    return transaction(env, handle, [&](Session &s) {
        return exact_android_control_query(s.runtime, uint32_t(view), uint32_t(kind));
    });
}
void scrolled(JNIEnv *env, jclass, jlong handle, jbyteArray positions) {
    boundary(env, false, [&]() {
        auto *s = session(env, handle);
        if (!s) return false;
        const auto count = positions ? env->GetArrayLength(positions) : 0;
        if (env->ExceptionCheck()) return false;
        if (count % 20 != 0) { fail(env, argument_error, "Truncated native scroll facts"); return false; }
        ++s->transactions;
        s->scroll_input.resize(size_t(count));
        if (count) env->GetByteArrayRegion(positions, 0, count, reinterpret_cast<jbyte *>(s->scroll_input.data()));
        if (env->ExceptionCheck()) return false;
        for (jint offset = 0; offset < count; offset += 20) {
            const auto *r = s->scroll_input.data() + offset;
            const uint32_t id = uint32_t(r[0]) | uint32_t(r[1]) << 8 | uint32_t(r[2]) << 16 | uint32_t(r[3]) << 24;
            double left, top;
            std::memcpy(&left, r + 4, sizeof(left));
            std::memcpy(&top, r + 12, sizeof(top));
            if (!exact_android_scrolled(s->runtime, id, left, top)) {
                fail(env, argument_error, "Invalid native scroll facts"); return false;
            }
        }
        return true;
    });
}
jobject collectionFeedback(JNIEnv *env, jclass, jlong handle, jbyteArray payload, jdouble now) {
    return transaction(env, handle, [&](Session &s) {
        uint32_t count;
        return input(env, s, payload, count) ? exact_android_collection_feedback(s.runtime, count, now) : 0;
    });
}
jobject agent(JNIEnv *env, jclass, jlong handle, jbyteArray payload) {
    return transaction(env, handle, [&](Session &s) {
        uint32_t count;
        return input(env, s, payload, count) ? exact_android_agent(s.runtime, count) : 0;
    });
}

// Benchmark observation only; called outside the measured hot loop. Counts
// describe app transactions/callbacks and wrapper objects, not JVM heap bytes.
jlongArray bridgeStats(JNIEnv *env, jclass, jlong handle) {
    return boundary(env, jlongArray(nullptr), [&]() -> jlongArray {
        auto *value = session(env, handle);
        if (!value) return nullptr;
        const jlong counts[] = {
            jlong(value->transactions), jlong(value->output_bytes),
            jlong(value->output_allocations), jlong(value->measure_calls),
            jlong(value->measure_wire.allocations), jlong(value->font_calls),
            jlong(value->fonts_wire.allocations), jlong(value->wake_calls.load(std::memory_order_relaxed)),
            jlong(value->output_window_calls + value->measure_wire.window_calls + value->fonts_wire.window_calls)
        };
        const auto output = env->NewLongArray(9);
        if (output) env->SetLongArrayRegion(output, 0, 9, counts);
        return output;
    });
}

jclass keep_class(JNIEnv *env, const char *name) {
    const auto local = env->FindClass(name);
    if (!local) return nullptr;
    const auto global = static_cast<jclass>(env->NewGlobalRef(local));
    env->DeleteLocalRef(local);
    return global;
}

// Kotlin declares @JvmStatic external methods on com.exact.android.Native.
// Registration catches signature drift when the carrier loads, before boot.
#define NATIVE(name, signature) {const_cast<char *>(#name), const_cast<char *>(signature), reinterpret_cast<void *>(name)}
JNINativeMethod methods[] = {
    NATIVE(create, "(Lcom/exact/android/TextEngine;)J"),
    NATIVE(close, "(J)V"),
    NATIVE(boot, "(JFF[B)Ljava/nio/ByteBuffer;"),
    NATIVE(dispatch, "(JII[BD)Ljava/nio/ByteBuffer;"),
    NATIVE(resize, "(JFF)Ljava/nio/ByteBuffer;"),
    NATIVE(frame, "(JD)Ljava/nio/ByteBuffer;"),
    NATIVE(advance, "(JD)Ljava/nio/ByteBuffer;"),
    NATIVE(tick, "(JD)Ljava/nio/ByteBuffer;"),
    NATIVE(pump, "(JD)Ljava/nio/ByteBuffer;"),
    NATIVE(painted, "(J)Ljava/nio/ByteBuffer;"),
    NATIVE(preferences, "(JI)Ljava/nio/ByteBuffer;"),
    NATIVE(insets, "(JFFFF)Ljava/nio/ByteBuffer;"),
    NATIVE(intrinsic, "(JIFF)Ljava/nio/ByteBuffer;"),
    NATIVE(intrinsics, "(J[B)Ljava/nio/ByteBuffer;"),
    NATIVE(agent, "(J[B)Ljava/nio/ByteBuffer;"),
    NATIVE(controlQuery, "(JII)Ljava/nio/ByteBuffer;"),
    NATIVE(collectionFeedback, "(J[BD)Ljava/nio/ByteBuffer;"),
    NATIVE(scrolled, "(J[B)V"),
    NATIVE(bridgeStats, "(J)[J")
};
#undef NATIVE
} // namespace

extern "C" JNIEXPORT jint JNICALL JNI_OnLoad(JavaVM *machine, void *) {
    JNIEnv *env = nullptr;
    if (machine->GetEnv(reinterpret_cast<void **>(&env), JNI_VERSION_1_6) != JNI_OK) return JNI_ERR;
    vm = machine;
    text_class = keep_class(env, "com/exact/android/TextEngine");
    if (!text_class) return JNI_ERR;
    state_error = keep_class(env, "java/lang/IllegalStateException");
    if (!state_error) return JNI_ERR;
    argument_error = keep_class(env, "java/lang/IllegalArgumentException");
    if (!argument_error) return JNI_ERR;
    memory_error = keep_class(env, "java/lang/OutOfMemoryError");
    if (!memory_error) return JNI_ERR;
    measure_method = env->GetMethodID(text_class, "measure", "(Ljava/nio/ByteBuffer;I)Ljava/nio/ByteBuffer;");
    if (!measure_method) return JNI_ERR;
    fonts_method = env->GetMethodID(text_class, "fonts", "(Ljava/nio/ByteBuffer;)V");
    if (!fonts_method) return JNI_ERR;
    wake_method = env->GetMethodID(text_class, "wake", "()V");
    if (!wake_method) return JNI_ERR;
    const auto buffer_class = env->FindClass("java/nio/Buffer");
    if (!buffer_class) return JNI_ERR;
    position_method = env->GetMethodID(buffer_class, "position", "(I)Ljava/nio/Buffer;");
    if (!position_method) {
        env->DeleteLocalRef(buffer_class);
        return JNI_ERR;
    }
    limit_method = env->GetMethodID(buffer_class, "limit", "(I)Ljava/nio/Buffer;");
    env->DeleteLocalRef(buffer_class);
    if (!limit_method) return JNI_ERR;
    const auto native_class = env->FindClass("com/exact/android/Native");
    if (!native_class) return JNI_ERR;
    const auto result = env->RegisterNatives(native_class, methods, sizeof(methods) / sizeof(methods[0]));
    env->DeleteLocalRef(native_class);
    return result == JNI_OK ? JNI_VERSION_1_6 : JNI_ERR;
}

extern "C" JNIEXPORT void JNICALL JNI_OnUnload(JavaVM *machine, void *) {
    JNIEnv *env = nullptr;
    if (machine->GetEnv(reinterpret_cast<void **>(&env), JNI_VERSION_1_6) != JNI_OK) return;
    for (const auto type : {text_class, state_error, argument_error, memory_error})
        if (type) env->DeleteGlobalRef(type);
}
