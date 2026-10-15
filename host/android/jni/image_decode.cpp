#include <android/asset_manager_jni.h>
#include <android/data_space.h>
#include <android/hardware_buffer_jni.h>
#include <android/imagedecoder.h>
#include <dlfcn.h>
#include <jni.h>
#include <pthread.h>
#include <stdint.h>
#include <string.h>

// Public API30 decoder symbols are optional: the host still supports API29.
namespace {
struct Api {
    int (*create)(const void*, size_t, AImageDecoder**);
    void (*destroy)(AImageDecoder*);
    const AImageDecoderHeaderInfo* (*header)(const AImageDecoder*);
    int32_t (*width)(const AImageDecoderHeaderInfo*);
    int32_t (*height)(const AImageDecoderHeaderInfo*);
    int32_t (*space)(const AImageDecoderHeaderInfo*);
    int (*format)(AImageDecoder*, int32_t);
    int (*size)(AImageDecoder*, int32_t, int32_t);
    int (*crop)(AImageDecoder*, ARect);
    int (*decode)(AImageDecoder*, void*, size_t, size_t);
    bool ready;
};
Api api{};
pthread_once_t once = PTHREAD_ONCE_INIT;
void load() {
    // Keep this SDK library alive while worker calls can use its function table.
    void* lib = dlopen("libjnigraphics.so", RTLD_NOW | RTLD_LOCAL);
    if (!lib) return;
#define SYMBOL(member, name) api.member = reinterpret_cast<decltype(api.member)>(dlsym(lib, name))
    SYMBOL(create, "AImageDecoder_createFromBuffer"); SYMBOL(destroy, "AImageDecoder_delete");
    SYMBOL(header, "AImageDecoder_getHeaderInfo"); SYMBOL(width, "AImageDecoderHeaderInfo_getWidth");
    SYMBOL(height, "AImageDecoderHeaderInfo_getHeight"); SYMBOL(space, "AImageDecoderHeaderInfo_getDataSpace");
    SYMBOL(format, "AImageDecoder_setAndroidBitmapFormat"); SYMBOL(size, "AImageDecoder_setTargetSize");
    SYMBOL(crop, "AImageDecoder_setCrop"); SYMBOL(decode, "AImageDecoder_decodeImage");
#undef SYMBOL
    api.ready = api.create && api.destroy && api.header && api.width && api.height && api.space &&
        api.format && api.size && api.crop && api.decode;
}
// Conservative SDR subset. Any orientation/ICC/XMP/MPF/gainmap metadata stays
// on ImageDecoder's existing path rather than guessing its color/rotation rules.
bool plainJpeg(const uint8_t* bytes, size_t count) {
    if (count < 4 || bytes[0] != 255 || bytes[1] != 216) return false;
    bool frame = false;
    size_t at = 2;
    while (at + 2 <= count) {
        if (bytes[at++] != 255) return false;
        while (at < count && bytes[at] == 255) ++at;
        if (at == count) return false;
        unsigned marker = bytes[at++];
        if (marker == 218) return frame;
        if (marker == 217 || marker == 0) return false;
        if (marker == 1 || (marker >= 208 && marker <= 215)) continue;
        if (at + 2 > count) return false;
        size_t size = (size_t(bytes[at]) << 8) | bytes[at + 1];
        if (size < 2 || size > count - at) return false;
        if (marker >= 225 && marker <= 239) return false;
        if (marker >= 192 && marker <= 207 && marker != 196 && marker != 200 && marker != 204) {
            if ((marker != 192 && marker != 193 && marker != 194) || size < 8 ||
                bytes[at + 2] != 8 || bytes[at + 7] != 3) return false;
            frame = true;
        }
        at += size;
    }
    return false;
}
struct Asset {
    AAsset* value;
    ~Asset() { if (value) AAsset_close(value); }
};
struct Decoder {
    AImageDecoder* value = nullptr;
    ~Decoder() { if (value) api.destroy(value); }
};
struct Buffer {
    AHardwareBuffer* value = nullptr;
    ~Buffer() { if (value) AHardwareBuffer_release(value); }
};
}

extern "C" JNIEXPORT jobject JNICALL
Java_com_exact_android_NativeImageDecode_decode(JNIEnv* env, jclass klass, jobject manager,
        jstring source, jint offeredWidth, jint offeredHeight, jstring fit, jboolean cropAllowed,
        jintArray natural) {
    if (offeredWidth <= 0 || offeredHeight <= 0 || env->GetArrayLength(natural) != 2) return nullptr;
    pthread_once(&once, load);
    if (!api.ready) return nullptr;
    const char* path = env->GetStringUTFChars(source, nullptr);
    if (!path) return nullptr;
    bool ascii = true;
    for (const unsigned char* p = reinterpret_cast<const unsigned char*>(path); *p; ++p)
        if (*p >= 128) ascii = false;
    AAssetManager* assets = AAssetManager_fromJava(env, manager);
    Asset asset{ascii && assets ? AAssetManager_open(assets, path, AASSET_MODE_BUFFER) : nullptr};
    env->ReleaseStringUTFChars(source, path);
    if (!asset.value) return nullptr;
    const auto* bytes = static_cast<const uint8_t*>(AAsset_getBuffer(asset.value));
    const off64_t length = AAsset_getLength64(asset.value);
    if (!bytes || length < 0 || !plainJpeg(bytes, static_cast<size_t>(length))) return nullptr;
    Decoder decoder;
    if (api.create(bytes, static_cast<size_t>(length), &decoder.value) != 0) return nullptr;
    const AImageDecoderHeaderInfo* info = api.header(decoder.value);
    if (!info || api.space(info) != ADATASPACE_SRGB) return nullptr;
    const jint dims[] = {api.width(info), api.height(info)};
    // Reuse the one Kotlin ImageDecodePlan authority, including crop parity.
    jmethodID planMethod = env->GetStaticMethodID(klass, "plan", "(IIIILjava/lang/String;Z)[I");
    if (!planMethod) return nullptr;
    auto plan = static_cast<jintArray>(env->CallStaticObjectMethod(klass, planMethod,
        dims[0], dims[1], offeredWidth, offeredHeight, fit, cropAllowed));
    if (env->ExceptionCheck() || !plan) return nullptr;
    if (env->GetArrayLength(plan) != 6) { env->DeleteLocalRef(plan); return nullptr; }
    jint p[6]; env->GetIntArrayRegion(plan, 0, 6, p); env->DeleteLocalRef(plan);
    if (env->ExceptionCheck() || p[0] <= 0 || p[1] <= 0 || p[2] < 0 || p[3] < 0 ||
        p[4] <= 0 || p[5] <= 0 || int64_t(p[2]) + p[4] > p[0] || int64_t(p[3]) + p[5] > p[1]) return nullptr;
    if (api.format(decoder.value, ANDROID_BITMAP_FORMAT_RGBA_8888) != 0 ||
        api.size(decoder.value, p[0], p[1]) != 0) return nullptr;
    if ((p[2] || p[3] || p[4] != p[0] || p[5] != p[1]) &&
        api.crop(decoder.value, ARect{p[2], p[3], p[2] + p[4], p[3] + p[5]}) != 0) return nullptr;
    AHardwareBuffer_Desc desc{};
    desc.width = uint32_t(p[4]); desc.height = uint32_t(p[5]); desc.layers = 1;
    desc.format = AHARDWAREBUFFER_FORMAT_R8G8B8X8_UNORM;
    desc.usage = AHARDWAREBUFFER_USAGE_CPU_READ_RARELY | AHARDWAREBUFFER_USAGE_CPU_WRITE_RARELY |
        AHARDWAREBUFFER_USAGE_GPU_SAMPLED_IMAGE;
    Buffer buffer;
    if (AHardwareBuffer_allocate(&desc, &buffer.value) != 0) return nullptr;
    AHardwareBuffer_describe(buffer.value, &desc);
    const uint64_t stride = uint64_t(desc.stride) * 4;
    if (stride < uint64_t(p[4]) * 4 || stride > SIZE_MAX / uint64_t(p[5])) return nullptr;
    void* pixels = nullptr;
    if (AHardwareBuffer_lock(buffer.value, AHARDWAREBUFFER_USAGE_CPU_WRITE_RARELY, -1, nullptr, &pixels) != 0) return nullptr;
    const int decoded = api.decode(decoder.value, pixels, size_t(stride), size_t(stride * p[5]));
    const int unlocked = AHardwareBuffer_unlock(buffer.value, nullptr);
    if (decoded != 0 || unlocked != 0) return nullptr;
    env->SetIntArrayRegion(natural, 0, 2, dims);
    if (env->ExceptionCheck()) return nullptr;
    // Java HardwareBuffer acquires its own ref; Buffer releases our ref once.
    return AHardwareBuffer_toHardwareBuffer(env, buffer.value);
}
