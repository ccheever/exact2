// The executor's C++: a lean Hermes runtime (bytecode only — it cannot be
// handed source), one module evaluated once, and calls into `exact.answer`
// with strings. The one binding installed is `console` (LLP 1027 D10,
// ruled 2026-09-03): it reaches nothing outside the process.
//
// @ref LLP 1027 D3 (the executor) / D10 (what the module can use)
#include <hermes/hermes.h>
#include <jsi/jsi.h>

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

// (ctx, op, a, b, out) -> 0 with `out` null (undefined) or a malloc'd string.
typedef int (*HostFn)(void *ctx, uint32_t op, const char *a, const char *b, char **out);

struct State {
  std::unique_ptr<facebook::hermes::HermesRuntime> rt;
  std::vector<std::string> log;
  HostFn host;
  void *ctx;
};

constexpr size_t kLogLines = 256;

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

int fail(char **out, const std::string &message, int code) {
  if (out) *out = dup(message);
  return code;
}

}  // namespace

extern "C" {

void *exact_js_create(uint32_t max_heap_bytes, HostFn host, void *ctx) {
  try {
    auto gc = ::hermes::vm::GCConfig::Builder().withMaxHeapSize(max_heap_bytes).build();
    auto config = ::hermes::vm::RuntimeConfig::Builder()
                      .withEnableEval(false)
                      .withMicrotaskQueue(true)
                      .withGCConfig(gc)
                      .build();
    auto rt = facebook::hermes::makeHermesRuntimeNoThrow(config);
    if (!rt) return nullptr;
    auto *state = new State{std::move(rt), {}, host, ctx};
    install_console(state);
    install_host(state);
    return state;
  } catch (...) {
    return nullptr;
  }
}

int exact_js_load(void *h, const uint8_t *data, size_t len, char **out) {
  auto *state = static_cast<State *>(h);
  try {
    auto buffer = std::make_shared<OwnedBytes>(std::vector<uint8_t>(data, data + len));
    state->rt->evaluateJavaScript(buffer, "app.hbc");
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
  jsi::Runtime &rt = *state->rt;
  try {
    jsi::Function fn = rt.global().getPropertyAsFunction(rt, name);
    jsi::Value result = fn.call(rt, jsi::String::createFromUtf8(rt, a),
                                jsi::String::createFromUtf8(rt, b),
                                jsi::String::createFromUtf8(rt, c));
    if (result.isUndefined()) return fail(out, "", 0);
    if (!result.isString()) {
      return fail(out, result.isObject() && result.getObject(rt).hasProperty(rt, "then")
                           ? "returned a promise"
                           : "did not return a string",
                  3);
    }
    return fail(out, result.getString(rt).utf8(rt), 0);
  } catch (const jsi::JSError &e) {
    return fail(out, e.getMessage(), 1);
  } catch (const std::exception &e) {
    return fail(out, e.what(), 2);
  }
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

void exact_js_destroy(void *h) { delete static_cast<State *>(h); }

}  // extern "C"
