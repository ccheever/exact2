// JNI only owns a public main Handle; the existing Rust host owns its render loop.
#include <jni.h>
#include <android/native_window_jni.h>
#include <atomic>
#include <cmath>
#include <cstdlib>
#include <cstdint>
#include <thread>
#include <unordered_map>

extern "C" {
void *heavy_main_start(void *, uint32_t, uint32_t, float);
void heavy_main_stop(void *);
bool heavy_main_ready(const void *);
uint64_t heavy_main_frames(const void *);
void heavy_main_scroll(const void *, float);
void heavy_main_touch(const void *, int32_t, float, float);
}
namespace {
thread_local std::unordered_map<jlong, void *> owners;
std::atomic<jlong> next_id{1};
void fail(JNIEnv *env, const char *type, const char *message) {
    if (env->ExceptionCheck()) return;
    const auto cls = env->FindClass(type);
    if (cls) { env->ThrowNew(cls, message); env->DeleteLocalRef(cls); }
}
void *owner(JNIEnv *env, jlong handle) {
    const auto found = owners.find(handle);
    if (found == owners.end()) {
        fail(env, "java/lang/IllegalStateException", "Main surface is closed or belongs to another thread");
        return nullptr;
    }
    return found->second;
}
bool environment(JNIEnv *env, const char *name, jstring input) {
    if (!input) { fail(env, "java/lang/IllegalArgumentException", "Missing asset/cache directory"); return false; }
    const auto text = env->GetStringUTFChars(input, nullptr);
    if (!text) return false;
    const bool okay = setenv(name, text, 1) == 0;
    env->ReleaseStringUTFChars(input, text);
    if (!okay) fail(env, "java/lang/IllegalStateException", "Cannot configure native painter environment");
    return okay;
}
jlong start(JNIEnv *env, jclass, jobject surface, jint w, jint h, jfloat scale, jstring assets, jstring cache, jboolean live) {
    if (!surface || w <= 0 || h <= 0 || !std::isfinite(scale) || scale <= 0) {
        fail(env, "java/lang/IllegalArgumentException", "Main requires a finite positive surface"); return 0;
    }
    if (!environment(env, "EXACT_ASSETS", assets) || !environment(env, "EXACT_CACHE", cache)) return 0;
    if (setenv("BENCH_LIVE", live ? "1" : "0", 1) != 0) {
        fail(env, "java/lang/IllegalStateException", "Cannot configure Heavy List live mode"); return 0;
    }
    auto *window = ANativeWindow_fromSurface(env, surface);
    if (!window) return 0;
    // Public Launch transfers this acquired native-window reference to its render thread.
    const auto raw = heavy_main_start(window, uint32_t(w), uint32_t(h), scale);
    if (!raw) { ANativeWindow_release(window); return 0; }
    const auto id = next_id.fetch_add(1, std::memory_order_relaxed);
    owners.emplace(id, raw);
    return id;
}
void close(JNIEnv *env, jclass, jlong id) {
    auto *raw = owner(env, id);
    if (!raw) return;
    owners.erase(id);
    heavy_main_stop(raw);
}
jboolean ready(JNIEnv *env, jclass, jlong id) {
    auto *raw = owner(env, id); return raw && heavy_main_ready(raw);
}
jlong frames(JNIEnv *env, jclass, jlong id) {
    auto *raw = owner(env, id); return raw ? jlong(heavy_main_frames(raw)) : 0;
}
void scroll(JNIEnv *env, jclass, jlong id, jfloat dy) {
    auto *raw = owner(env, id);
    if (!std::isfinite(dy)) { fail(env, "java/lang/IllegalArgumentException", "Nonfinite scroll"); return; }
    if (raw) heavy_main_scroll(raw, dy);
}
void touch(JNIEnv *env, jclass, jlong id, jint action, jfloat x, jfloat y) {
    auto *raw = owner(env, id);
    if (action < 0 || action > 3 || !std::isfinite(x) || !std::isfinite(y)) {
        fail(env, "java/lang/IllegalArgumentException", "Invalid touch"); return;
    }
    if (raw) heavy_main_touch(raw, action, x, y);
}
#define METHOD(name, signature) {const_cast<char *>(#name), const_cast<char *>(signature), reinterpret_cast<void *>(name)}
JNINativeMethod methods[] = {
    METHOD(start, "(Landroid/view/Surface;IIFLjava/lang/String;Ljava/lang/String;Z)J"),
    METHOD(close, "(J)V"), METHOD(ready, "(J)Z"), METHOD(frames, "(J)J"),
    METHOD(scroll, "(JF)V"), METHOD(touch, "(JIFF)V")
};
#undef METHOD
}
extern "C" JNIEXPORT jint JNICALL JNI_OnLoad(JavaVM *vm, void *) {
    JNIEnv *env = nullptr;
    if (vm->GetEnv(reinterpret_cast<void **>(&env), JNI_VERSION_1_6) != JNI_OK) return JNI_ERR;
    const auto cls = env->FindClass("dev/exact/heavybench/MainNative");
    if (!cls) return JNI_ERR;
    const auto result = env->RegisterNatives(cls, methods, sizeof(methods) / sizeof(methods[0]));
    env->DeleteLocalRef(cls);
    return result == 0 ? JNI_VERSION_1_6 : JNI_ERR;
}
