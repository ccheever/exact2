// The executor's C++: a lean Hermes runtime (bytecode only — it cannot be
// handed source), one module evaluated once, and calls into `exact.answer`
// with strings. Storage uses Ibex2 bindings on this same runtime and the
// caller owns every completion and microtask checkpoint (LLP 1027 D9/D10).
//
// @ref LLP 1027 D3 (the executor) / D10 (what the module can use)
#include <hermes/hermes.h>
#include <jsi/jsi.h>
#include <ibex2_jsi.h>
#include <jsi/instrumentation.h>

#include <algorithm>
#include <cstdint>
#include <cstdlib>
#include <cstring>
#include <memory>
#include <string>
#include <vector>

using namespace facebook;

namespace {

// Bytecode must outlive the evaluation that loaded it: Hermes keeps
// pointers into the buffer for as long as the code can run.
class OwnedBytes : public jsi::Buffer {
public:
  explicit OwnedBytes(std::vector<uint8_t> bytes) : bytes_(std::move(bytes)) {}
  size_t size() const override { return bytes_.size(); }
  const uint8_t *data() const override { return bytes_.data(); }

private:
  std::vector<uint8_t> bytes_;
};

// Bytecode its caller keeps until the runtime is destroyed (engine.rs: the
// binary's own, or the module's, which the engine holds), read in place.
// Hermes refuses a buffer that is not aligned for its file header; such a
// buffer is copied (`OwnedBytes`).
class BorrowedBytes : public jsi::Buffer {
public:
  BorrowedBytes(const uint8_t *data, size_t size) : data_(data), size_(size) {}
  size_t size() const override { return size_; }
  const uint8_t *data() const override { return data_; }

private:
  const uint8_t *data_;
  size_t size_;
};

// (ctx, op, a, b, out) -> 0 with `out` null (undefined) or a malloc'd string.
typedef int (*HostFn)(void *ctx, uint32_t op, const char *a, const char *b, char **out);
// (ctx, op, a, bytes, len, out): the host door for byte input (LLP 1069.005:
// a digest's data, a key's handle and a message to sign). Same result rules.
typedef int (*BytesFn)(void *ctx, uint32_t op, const char *a, const uint8_t *bytes, size_t len,
                       char **out);

struct CapturedString {
  std::string path;
  std::u16string value;
};

struct State {
  std::unique_ptr<facebook::hermes::HermesRuntime> rt;
  std::vector<std::string> log;
  HostFn host;
  BytesFn bytes;
  void *ctx;
  // Destroy the adapter (and its JSI roots) before the runtime.
  std::unique_ptr<ibex2::jsi_adapter::Adapter> adapter;
  const void *grants;
  size_t capture_limit;
  size_t capture_bytes = 0;
  std::vector<CapturedString> captures;
  // A capture of this reply was made before a collection that ran after its
  // answer began (`__exact_capture_string`'s third argument).
  bool captures_held = false;
  // Collections so far, Hermes' own and these: what the prelude stamps an
  // answer with when it begins (`__exact_gc_epoch`).
  int64_t gc_epoch = 0;
  // At the last full collection (collect_garbage): the heap's total
  // allocation then and what it held, all of it live.
  int64_t collected_at = 0;
  int64_t live = 0;
};

constexpr size_t kLogLines = 256;

// A full collection, after which the heap holds only what is live.
void collect_garbage(State *state, const char *cause) {
  auto &instrumentation = state->rt->instrumentation();
  instrumentation.collectGarbage(cause);
  auto info = instrumentation.getHeapInfo(false);
  state->collected_at = info["hermes_totalAllocatedBytes"];
  state->live = info["hermes_allocatedBytes"];
  state->gc_epoch = info["hermes_numCollections"] + info["hermes_yg_numCollections"];
}

// Hermes' young generation keeps every page it has touched (exact_js_harden),
// and Hades lets it grow over a session to its whole 4 MB segment: each young
// collection ends it at a share of the segment that starts at a half and grows
// by a tenth with every collection under 10 ms. A module that allocates a
// little per call collects here instead, between calls, once it has allocated
// half as much as it held live at the last full collection and at least
// kCollectEvery since. Its young generation then stays near its largest
// call's allocation plus kCollectEvery: euxy's boot (a 430 KB call, then a
// 760 KB one) kept 1.2 MB of it touched at 512 KB, and keeps 0.8 MB. A full
// collection's cost follows the live heap, so this is at most one per half
// that much allocation. The total counts what Hermes' own young collections
// already reclaimed, so after one this collects early, never late.
constexpr int64_t kCollectEvery = 256 << 10;

void collect_if_due(State *state) {
  auto info = state->rt->instrumentation().getHeapInfo(false);
  int64_t total = info["hermes_totalAllocatedBytes"];
  // Hermes' own collections within the call count too: one may have moved
  // an answer's value to the old generation.
  state->gc_epoch = info["hermes_numCollections"] + info["hermes_yg_numCollections"];
  if (total - state->collected_at >= std::max(kCollectEvery, state->live / 2))
    collect_garbage(state, "between calls");
}

void clear_captures(State *state) {
  state->captures.clear();
  state->capture_bytes = 0;
  state->captures_held = false;
}

// The prelude captures and removes this binding before app initialization.
// Strings stay UTF-16 here: converting through JSI UTF-8 would replace lone
// surrogates which the existing JSON/serde seam rejects.
void install_capture(State *state) {
  auto &rt = *state->rt;
  rt.global().setProperty(rt, "__exact_capture_string",
      jsi::Function::createFromHostFunction(rt,
          jsi::PropNameID::forAscii(rt, "__exact_capture_string"), 3,
          [state](jsi::Runtime &rt, const jsi::Value &, const jsi::Value *args,
                  size_t count) -> jsi::Value {
            if (count < 2 || count > 3 || !args[0].isString() || !args[1].isString())
              throw jsi::JSError(rt, "result capture requires a path and string");
            if (count == 3 && args[2].isBool() && args[2].getBool()) state->captures_held = true;
            auto path = args[0].getString(rt).utf8(rt);
            auto value = args[1].getString(rt).utf16(rt);
            const auto remaining = state->capture_limit - state->capture_bytes;
            if (path.size() > remaining || value.size() > (remaining-path.size())/sizeof(char16_t))
              throw jsi::JSError(rt, "result string captures exceed the module heap limit");
            state->capture_bytes += path.size() + value.size()*sizeof(char16_t);
            state->captures.push_back({std::move(path), std::move(value)});
            return jsi::Value::undefined();
          }));
  // The collections as of the last call's end, read when an answer begins.
  rt.global().setProperty(rt, "__exact_gc_epoch",
      jsi::Function::createFromHostFunction(rt,
          jsi::PropNameID::forAscii(rt, "__exact_gc_epoch"), 0,
          [state](jsi::Runtime &, const jsi::Value &, const jsi::Value *, size_t) -> jsi::Value {
            return jsi::Value(static_cast<double>(state->gc_epoch));
          }));
}

char *dup(const std::string &s) {
  char *p = static_cast<char *>(std::malloc(s.size() + 1));
  std::memcpy(p, s.c_str(), s.size() + 1);
  return p;
}

std::string render(jsi::Runtime &rt, const jsi::Value &v) {
  if (v.isObject()) {
    jsi::Function stringify =
        rt.global().getPropertyAsObject(rt, "JSON").getPropertyAsFunction(rt, "stringify");
    jsi::Value text = stringify.call(rt, v);
    if (text.isString()) return text.getString(rt).utf8(rt);
  }
  return v.toString(rt).utf8(rt);
}

void install_console(State *state) {
  jsi::Runtime &rt = *state->rt;
  jsi::Object console(rt);
  for (const char *level : {"log", "info", "warn", "error", "debug"}) {
    console.setProperty(
        rt, level,
        jsi::Function::createFromHostFunction(
            rt, jsi::PropNameID::forAscii(rt, level), 1,
            [state](jsi::Runtime &rt, const jsi::Value &, const jsi::Value *args,
                    size_t count) -> jsi::Value {
              std::string line;
              for (size_t i = 0; i < count; i++) {
                if (i) line += ' ';
                line += render(rt, args[i]);
              }
              if (state->log.size() >= kLogLines) state->log.erase(state->log.begin());
              state->log.push_back(std::move(line));
              return jsi::Value::undefined();
            }));
  }
  rt.global().setProperty(rt, "console", console);
}

// `__exact_host(op, a, b)`: the one door into Rust — the prelude takes it
// off the global object and keeps it in a closure (LLP 1027 D1a, D10).
void install_host(State *state) {
  jsi::Runtime &rt = *state->rt;
  rt.global().setProperty(
      rt, "__exact_host",
      jsi::Function::createFromHostFunction(
          rt, jsi::PropNameID::forAscii(rt, "__exact_host"), 3,
          [state](jsi::Runtime &rt, const jsi::Value &, const jsi::Value *args,
                  size_t count) -> jsi::Value {
            if (count < 3 || !args[0].isNumber() || !args[1].isString() || !args[2].isString()) {
              throw jsi::JSError(rt, "__exact_host(op, a, b) takes a number and two strings");
            }
            std::string a = args[1].getString(rt).utf8(rt);
            std::string b = args[2].getString(rt).utf8(rt);
            char *out = nullptr;
            int status = state->host(state->ctx, static_cast<uint32_t>(args[0].getNumber()),
                                     a.c_str(), b.c_str(), &out);
            std::string text = out ? std::string(out) : std::string();
            if (out) std::free(out);
            if (status != 0) throw jsi::JSError(rt, text);
            if (!out) return jsi::Value::undefined();
            return jsi::String::createFromUtf8(rt, text);
          }));
}

// `__exact_bytes(op, a, arrayBuffer)`: the door for bytes, which the string
// door cannot carry (LLP 1069.005 D1). The prelude takes it too.
void install_bytes(State *state) {
  jsi::Runtime &rt = *state->rt;
  rt.global().setProperty(
      rt, "__exact_bytes",
      jsi::Function::createFromHostFunction(
          rt, jsi::PropNameID::forAscii(rt, "__exact_bytes"), 3,
          [state](jsi::Runtime &rt, const jsi::Value &, const jsi::Value *args,
                  size_t count) -> jsi::Value {
            if (count < 3 || !args[0].isNumber() || !args[1].isString() || !args[2].isObject() ||
                !args[2].getObject(rt).isArrayBuffer(rt)) {
              throw jsi::JSError(rt, "__exact_bytes(op, a, bytes) takes a number, a string and an ArrayBuffer");
            }
            std::string a = args[1].getString(rt).utf8(rt);
            auto buffer = args[2].getObject(rt).getArrayBuffer(rt);
            char *out = nullptr;
            int status = state->bytes(state->ctx, static_cast<uint32_t>(args[0].getNumber()),
                                      a.c_str(), buffer.data(rt), buffer.size(rt), &out);
            std::string text = out ? std::string(out) : std::string();
            if (out) std::free(out);
            if (status != 0) throw jsi::JSError(rt, text);
            if (!out) return jsi::Value::undefined();
            return jsi::String::createFromUtf8(rt, text);
          }));
}

int fail(char **out, const std::string &message, int code) {
  if (out) *out = dup(message);
  return code;
}

}  // namespace

extern "C" {

void *exact_js_create(uint32_t max_heap_bytes, HostFn host, BytesFn bytes, void *ctx,
                      const void *queue, const Ibex2Bindings *bindings,
                      const void *grants,
                      const ibex2::jsi_adapter::CompiledScript *scripts,
                      size_t script_count) {
  try {
    auto gc = ::hermes::vm::GCConfig::Builder().withMaxHeapSize(max_heap_bytes).build();
    auto config = ::hermes::vm::RuntimeConfig::Builder()
                      .withEnableEval(false)
                      .withMicrotaskQueue(true)
                      .withGCConfig(gc)
                      .build();
    auto rt = facebook::hermes::makeHermesRuntimeNoThrow(config);
    if (!rt) return nullptr;
    auto state = std::make_unique<State>(State{std::move(rt), {}, host, bytes, ctx, nullptr,
                                               grants, max_heap_bytes, 0, {}});
    install_console(state.get());
    install_host(state.get());
    install_bytes(state.get());
    install_capture(state.get());
    state->adapter = std::make_unique<ibex2::jsi_adapter::Adapter>(*state->rt, queue);
    ibex2::jsi_adapter::InstallOptions options;
    options.abort_hooks = "__exact_ibex2_abort_hooks";
    options.defer_intrinsic_snapshot = true;
    constexpr auto groups = ibex2::jsi_adapter::GROUP_PURE |
                            ibex2::jsi_adapter::GROUP_CRYPTO |
                            ibex2::jsi_adapter::GROUP_ABORT
// Android defines __linux__ too, but its Hermes bundle has no Intl (Unicode is
// ICU's, Intl absent) and ibex2 builds no Intl shims there.
#if (defined(__linux__) && !defined(__ANDROID__)) || defined(_WIN32)
                            | ibex2::jsi_adapter::GROUP_INTL
#endif
                            ;
    state->adapter->install_with(groups, bindings, scripts, script_count, options);
    return state.release();
  } catch (...) {
    return nullptr;
  }
}

int exact_js_load(void *h, const uint8_t *data, size_t len, char **out) {
  auto *state = static_cast<State *>(h);
  try {
    std::shared_ptr<jsi::Buffer> buffer;
    if (reinterpret_cast<uintptr_t>(data) % 16 == 0)
      buffer = std::make_shared<BorrowedBytes>(data, len);
    else
      buffer = std::make_shared<OwnedBytes>(std::vector<uint8_t>(data, data + len));
    state->rt->evaluateJavaScript(buffer, "app.hbc");
    return 0;
  } catch (const jsi::JSError &e) {
    return fail(out, e.getMessage(), 1);
  } catch (const std::exception &e) {
    return fail(out, e.what(), 2);
  }
}

// Trusted initialization only: no application code or microtasks run here.
int exact_js_install_storage(void *h, const uint8_t *sqlite, size_t sqlite_len, char **out) {
  auto *state = static_cast<State *>(h);
  auto &rt = *state->rt;
  try {
    auto factory_bytes = std::make_shared<OwnedBytes>(
        std::vector<uint8_t>(sqlite, sqlite + sqlite_len));
    auto factory = rt.evaluateJavaScript(factory_bytes, "storage-sqlite.hbc")
                       .getObject(rt).getFunction(rt);
    rt.global().setProperty(rt, "__exact_storage", state->adapter->storage(state->grants, factory));
    rt.global().getPropertyAsFunction(rt, "__exact_install_storage").call(rt);
    // The prelude captures storage in its closure; no capability-bearing
    // temporary may remain when hardening locks global property descriptors.
    auto reflect = rt.global().getPropertyAsObject(rt, "Reflect");
    auto remove = reflect.getPropertyAsFunction(rt, "deleteProperty");
    for (const char *name : {"__exact_storage", "__exact_install_storage"}) {
      if (!remove.call(rt, rt.global(), jsi::String::createFromUtf8(rt, name)).getBool())
        return fail(out, "storage initialization could not remove its temporary global", 1);
    }
    return 0;
  } catch (const jsi::JSError &e) {
    return fail(out, e.getMessage(), 1);
  } catch (const std::exception &e) {
    return fail(out, e.what(), 2);
  }
}

// Complete the trusted bootstrap on every runtime, whether it has storage or
// not. Application bytecode is evaluated only after this succeeds.
int exact_js_harden(void *h, const uint8_t *harden, size_t harden_len, char **out) {
  auto *state = static_cast<State *>(h);
  try {
    state->adapter->capture_intrinsics();
    // Hermes' young generation is one heap segment that keeps every page it
    // has touched, so its footprint is the furthest point allocation reaches
    // between two collections. The runtime and prelude allocate about
    // 0.5 MB (much of the runtime's own in the old generation), the freeze's
    // guard and harden.js about 0.45 MB each (Adapter::harden collects
    // between them), nearly all of it garbage, and an app's first screens
    // often as much again: collecting between the steps lets each reuse the
    // pages of the one before, so boot touches about as much young
    // generation as its largest step. A collection takes 0.15 to 0.3 ms here
    // (iPhone simulator).
    state->rt->instrumentation().collectGarbage("bootstrap");
    ibex2::jsi_adapter::CompiledScript script{"harden", harden, harden_len};
    state->adapter->harden(script);
    collect_garbage(state, "hardened");
    return 0;
  } catch (const jsi::JSError &e) {
    return fail(out, e.getMessage(), 1);
  } catch (const std::exception &e) {
    return fail(out, e.what(), 2);
  }
}

// Settle one native storage promise; the caller decides when to checkpoint.
int exact_js_deliver_storage_one(void *h, bool *delivered, char **out) {
  auto *state = static_cast<State *>(h);
  try {
    *delivered = state->adapter && state->adapter->deliver_one();
    return 0;
  } catch (const jsi::JSError &e) {
    return fail(out, e.getMessage(), 1);
  } catch (const std::exception &e) {
    return fail(out, e.what(), 2);
  }
}

// `exact[name]` rendered as text: a string as is, a number or bool by
// `toString`, `undefined` as the empty string (`out` set, status 0).
int exact_js_string(void *h, const char *name, char **out) {
  auto *state = static_cast<State *>(h);
  jsi::Runtime &rt = *state->rt;
  try {
    jsi::Value exact = rt.global().getProperty(rt, "exact");
    if (!exact.isObject()) return fail(out, "the module defines no `exact`", 1);
    jsi::Value v = exact.getObject(rt).getProperty(rt, name);
    if (v.isUndefined() || v.isNull()) return fail(out, "", 0);
    return fail(out, v.toString(rt).utf8(rt), 0);
  } catch (const jsi::JSError &e) {
    return fail(out, e.getMessage(), 1);
  } catch (const std::exception &e) {
    return fail(out, e.what(), 2);
  }
}

// `globalThis[name](a, b, c)` — one of the prelude's three seam functions —
// with three strings; the result must be a string.
int exact_js_call(void *h, const char *name, const char *a, const char *b, const char *c,
                  char **out) {
  auto *state = static_cast<State *>(h);
  clear_captures(state);
  jsi::Runtime &rt = *state->rt;
  try {
    jsi::Function fn = rt.global().getPropertyAsFunction(rt, name);
    jsi::Value result = fn.call(rt, jsi::String::createFromUtf8(rt, a),
                                jsi::String::createFromUtf8(rt, b),
                                jsi::String::createFromUtf8(rt, c));
    if (result.isUndefined()) { clear_captures(state); return fail(out, "", 0); }
    if (!result.isString()) {
      clear_captures(state);
      return fail(out, result.isObject() && result.getObject(rt).hasProperty(rt, "then")
                           ? "returned a promise"
                           : "did not return a string",
                  3);
    }
    int status = fail(out, result.getString(rt).utf8(rt), 0);
    // A captured string (64 Ki characters or more) whose answer outlived a
    // collection may be old generation now, promoted while its answer was
    // pending, and once sent it is garbage that no later collection frees
    // while the module is idle (euxy's QR path: 376 KB). One made and sent
    // with no collection between, as a frame's answer is, waits for
    // `collect_if_due`.
    if (state->captures_held)
      collect_garbage(state, "held large reply");
    else
      collect_if_due(state);
    return status;
  } catch (const jsi::JSError &e) {
    clear_captures(state);
    return fail(out, e.getMessage(), 1);
  } catch (const std::exception &e) {
    clear_captures(state);
    return fail(out, e.what(), 2);
  }
}

// Borrowed until the next call/clear; Rust copies/decodes before returning.
size_t exact_js_capture_count(void *h) {
  return static_cast<State *>(h)->captures.size();
}
bool exact_js_capture(void *h, size_t index, const char **path, size_t *path_len,
                      const uint16_t **value, size_t *value_len) {
  const auto &captures = static_cast<State *>(h)->captures;
  if (index >= captures.size()) return false;
  const auto &capture = captures[index];
  *path = capture.path.data(); *path_len = capture.path.size();
  *value = reinterpret_cast<const uint16_t *>(capture.value.data());
  *value_len = capture.value.size();
  return true;
}
void exact_js_clear_captures(void *h) { clear_captures(static_cast<State *>(h)); }

// Stop the running execution, or the next to start, from any thread (LLP
// 1048.000 D10): Hermes raises an uncatchable timeout at the next async break
// check, which bytecode compiled with -emit-async-break-check has in every
// loop and function. The caller keeps the runtime alive through this call.
void exact_js_interrupt(void *h) {
  static_cast<State *>(h)->rt->asyncTriggerTimeout();
}

// Run every queued microtask: the continuations of resolved fetches.
int exact_js_drain(void *h, char **out) {
  auto *state = static_cast<State *>(h);
  try {
    state->rt->drainMicrotasks();
    return 0;
  } catch (const jsi::JSError &e) {
    return fail(out, e.getMessage(), 1);
  } catch (const std::exception &e) {
    return fail(out, e.what(), 2);
  }
}

// The console lines since the last take, newline-separated.
void exact_js_take_log(void *h, char **out) {
  auto *state = static_cast<State *>(h);
  std::string joined;
  for (size_t i = 0; i < state->log.size(); i++) {
    if (i) joined += '\n';
    joined += state->log[i];
  }
  state->log.clear();
  *out = dup(joined);
}

void exact_js_free(char *p) { std::free(p); }

void exact_js_destroy(void *h) {
  auto *state = static_cast<State *>(h);
  if (state->adapter) state->adapter->detach();
  delete state;
}

}  // extern "C"
