#include <jni.h>
#include <cstdlib>

// Benchmark launch configuration remains outside the production host API.
extern "C" JNIEXPORT void JNICALL
Java_dev_exact_heavybench_C9Config_setLive(JNIEnv *env, jclass, jboolean live) {
    if (setenv("BENCH_LIVE", live ? "1" : "0", 1) == 0) return;
    const auto type = env->FindClass("java/lang/IllegalStateException");
    if (type) { env->ThrowNew(type, "Cannot set Heavy List live configuration"); env->DeleteLocalRef(type); }
}
