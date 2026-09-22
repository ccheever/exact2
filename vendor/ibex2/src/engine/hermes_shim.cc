// Minimal C ABI over stock JSI for the Ibex 2 spike.
//
// This file deliberately uses ONLY the public Hermes/JSI embedding surface —
// makeHermesRuntime, RuntimeConfig, evaluateJavaScript, host functions, and
// the time-limit monitor. If anything here ever needs a symbol the carried
// patch series adds, that is the signal LLP 0060 D3 has been broken and the
// fork has grown back.
//
// @ref LLP 0060#1-the-decision — D4: eval is closed at construction, not latched after boot
// @ref LLP 0058#1-what-an-engine-must-provide — requirement 2, host functions over primitives

#include <hermes/hermes.h>
#include <jsi/jsi.h>
#include "../../include/ibex2_jsi.h"
#include <jsi/instrumentation.h>

#include <chrono>
#include <cstdint>
#include <cstdlib>
#include <unordered_map>
#include <vector>
#include <cstring>
#include <memory>
#include <string>

// Reports an error that escaped a callback as a console error (Rust side);
// declared here because the helper that calls it sits above the extern block.
extern "C" void ibex2_report_uncaught(const char *message);

using namespace facebook;
using namespace ibex2::jsi_adapter;
#if defined(__linux__)
namespace ibex2::intl_number_format {
void install(jsi::Runtime &, const void *);
}
namespace ibex2::intl_case {
void install(jsi::Runtime &, const void *);
}
namespace ibex2::intl_datetime {
std::vector<jsi::Value> factory_arguments(jsi::Runtime &, const void *);
}
#endif
extern "C" void ibex2_host_release(Ibex2AbiValue *);

extern "C" const void *ibex2_queue_create();
extern "C" void *ibex2_response_owner_create(const void *queue, double handle);
extern "C" void ibex2_response_owner_destroy(void *owner);
extern "C" size_t ibex2_grants_env_count(const void *grants);
extern "C" int ibex2_grants_env_at(const void *grants, size_t index,
                                   char **out_name, char **out_value);
extern "C" void ibex2_string_free(char *value);
extern "C" void ibex2_queue_destroy(const void *queue);
extern "C" void ibex2_grants_destroy(const void *grants);
extern "C" void ibex2_end_drive(const void *queue);

namespace {

struct ResponseOwner final : jsi::NativeState {
  void *owner;
  explicit ResponseOwner(void *value) : owner(value) {}
  ~ResponseOwner() override { ibex2_response_owner_destroy(owner); }
};

struct Ibex2Runtime {
  // The concrete engine, not the JSI interface: the time-limit monitor is
  // Hermes's own (hermes-interfaces.h), reached through HermesRuntime.
  std::unique_ptr<facebook::hermes::HermesRuntime> runtime;
  std::unique_ptr<Adapter> bindings;
  // This runtime's own completion queue. Per-runtime so two runtimes in one
  // process cannot take each other's completions (task::Pump C5).
  const void *queue = nullptr;
  // Loaded modules, by resolved specifier. Held here rather than on the global
  // object: a module registry reachable from JavaScript would let any module
  // read any other's exports without requiring it (LLP 0062 R1).
  // What `require` returns for each loaded module, keyed by resolved
  // specifier. A VALUE, not an object: `module.exports = 'text'` or `= 42` is
  // legal CommonJS, and a registry of objects silently handed back the empty
  // original in its place.
  std::unordered_map<std::string, std::shared_ptr<jsi::Value>> modules;
  // Grant sets handed to module bindings, released when the runtime is. One
  // entry per DISTINCT set: Rust interns them, so equal sets are one pointer.
  std::vector<const void *> module_grants;
  // The capability bindings, built once per grant set and shared by every
  // module that holds that set. Keyed by the interned grant pointer. Frozen,
  // so a module cannot alter what another module with the same authority
  // receives — sharing changes nothing about authority (the binding carries
  // the grant either way) and must change nothing about integrity.
  // Declared after `runtime` so it is destroyed before it.
  struct SharedBindings {
    jsi::Value fetch;
    jsi::Value fs;
    jsi::Value process;
    jsi::Value sqlite;
  };
  std::unordered_map<const void *, SharedBindings> shared;
  // The fetch factory: `bindings/fetch.js`'s completion value, held here and
  // never on the global object. Called once per grant set with the raw async
  // binding; what it returns is what a module receives as `fetch`. Undefined
  // until the bindings are installed, in which case the raw binding is used.
  jsi::Value make_fetch;
  jsi::Value make_sqlite;
  // The one deadline: armed by ibex2_hermes_set_deadline, consulted at every
  // entrance until cleared. A steady-clock point, so what an entrance is
  // given is the time left of the same deadline — never a fresh budget.
  bool deadline_armed = false;
  std::chrono::steady_clock::time_point deadline;
};

// An error that escaped a callback, to the console at error level with its
// stack, so the application hears about it.
void report_uncaught(const jsi::JSError &err) {
  std::string text = err.getMessage();
  const std::string &stack = err.getStack();
  if (!stack.empty()) {
    text += "\n";
    text += stack;
  }
  ibex2_report_uncaught(text.c_str());
}

// `Object.freeze(value)`, for a binding shared between modules.
void freeze(jsi::Runtime &rt, const jsi::Value &value) {
  auto object_ctor = rt.global().getPropertyAsObject(rt, "Object");
  object_ctor.getPropertyAsFunction(rt, "freeze").call(rt, jsi::Value(rt, value));
}

// Copy a std::string out to a malloc'd C string the Rust side owns and frees
// through ibex2_hermes_free_string.
char *dup_c_string(const std::string &value) {
  char *out = static_cast<char *>(std::malloc(value.size() + 1));
  if (out == nullptr) {
    return nullptr;
  }
  std::memcpy(out, value.c_str(), value.size() + 1);
  return out;
}

// What an entrance reports. Shared by eval, eval_bytes, drain_microtasks,
// pump, and run_entry, and mirrored by `checked` in hermes.rs.
constexpr int IBEX2_STATUS_OK = 0;
constexpr int IBEX2_STATUS_THREW = 1;
constexpr int IBEX2_STATUS_DEADLINE = 2;
constexpr int IBEX2_STATUS_INVALID = -1;

// Milliseconds from `now` until `at`, rounded up so the watch never fires
// early, and never zero: the door has already refused an entrance whose
// deadline has passed, so a sub-millisecond remainder is a one-millisecond
// watch.
uint32_t millis_until(std::chrono::steady_clock::time_point at,
                      std::chrono::steady_clock::time_point now) {
  auto remaining = at - now;
  auto whole = std::chrono::duration_cast<std::chrono::milliseconds>(remaining);
  if (whole < remaining) {
    whole += std::chrono::milliseconds(1);
  }
  auto count = whole.count();
  if (count < 1) {
    return 1;
  }
  if (count > static_cast<decltype(count)>(UINT32_MAX)) {
    return UINT32_MAX;
  }
  return static_cast<uint32_t>(count);
}

// One entrance into JavaScript, held to the runtime's deadline.
//
// @ref LLP 0058.000.000#8-tasks-microtasks-timers-and-callbacks — one deadline per runtime, held at every entrance; the engine fires it, the adapter owns it
//
// At the door: no deadline, run freely; deadline passed, refuse without
// running anything. Otherwise the pin's time-limit monitor is asked to watch
// for at least the time that is left, rounded up to a millisecond (the
// monitor counts in milliseconds; the clock below, not the monitor, decides
// the outcome), and the interpreter's async-break check — emitted at every
// loop back-edge and every return of evaluated source, per RuntimeConfig's
// AsyncBreakCheckInEval, which ibex2_hermes_create pins on — raises the
// engine's timeout, which JavaScript cannot catch (Runtime::raiseTimeoutError
// is an uncatchable error in the pinned source). On the way out the watch is
// always released, and if the deadline passed during the entrance the
// outcome is the deadline, whatever JavaScript was doing: classified by the
// clock, never by the text of what was thrown, because text is JavaScript's
// to forge.
//
// The monitor fires once and leaves its request pending when nothing was at
// a check — bytecode compiled without checks, or a fire that landed between
// the last check and the return. Left alone, that request would stop the
// runtime's next entrance for a deadline that is no longer armed. So an
// entrance that ends past its deadline runs one empty program, whose return
// check consumes whatever is pending, before it returns. That is what makes a
// runtime whose deadline fired reusable once the deadline is cleared.
class Entrance {
public:
  explicit Entrance(Ibex2Runtime &rt) : rt_(rt) {
    if (!rt_.deadline_armed) {
      return;
    }
    auto now = std::chrono::steady_clock::now();
    if (now >= rt_.deadline) {
      refused_ = true;
      return;
    }
    rt_.runtime->watchTimeLimit(millis_until(rt_.deadline, now));
    watching_ = true;
  }
  ~Entrance() {
    if (!watching_) {
      return;
    }
    rt_.runtime->unwatchTimeLimit();
    if (expired()) {
      flush_pending_break();
    }
  }
  Entrance(const Entrance &) = delete;
  Entrance &operator=(const Entrance &) = delete;

  bool refused() const { return refused_; }
  bool expired() const {
    return rt_.deadline_armed &&
           std::chrono::steady_clock::now() >= rt_.deadline;
  }
  // The status to report for an outcome JavaScript produced.
  int status(int outcome) const {
    return expired() ? IBEX2_STATUS_DEADLINE : outcome;
  }

private:
  void flush_pending_break() {
    try {
      auto buffer = std::make_shared<jsi::StringBuffer>(std::string("0"));
      rt_.runtime->evaluateJavaScript(buffer, "<deadline>");
    } catch (const jsi::JSError &) {
    } catch (const std::exception &) {
    }
  }

  Ibex2Runtime &rt_;
  bool refused_ = false;
  bool watching_ = false;
};

// The drive flag, released on every way out of a pump. A cycle the deadline
// stopped used to be able to leave it set, and a set flag refuses every cycle
// after it as nested.
class DriveGuard {
public:
  explicit DriveGuard(const void *queue) : queue_(queue) {}
  ~DriveGuard() { ibex2_end_drive(queue_); }
  DriveGuard(const DriveGuard &) = delete;
  DriveGuard &operator=(const DriveGuard &) = delete;

private:
  const void *queue_;
};

// A microtask checkpoint that reports what it cannot finish. A job that
// throws out of the queue — only an engine-raised error can, since Promise
// reactions and queueMicrotask catch their own — is reported as an uncaught
// error and the drain resumes behind it: the engine retires a job before it
// runs it, so every retry makes progress. Before this a job's throw unwound
// through the C ABI into Rust. The deadline is the one thing that ends a
// checkpoint early; returns false when it did.
bool checkpoint(Ibex2Runtime &rt, const Entrance &entrance) {
  for (;;) {
    try {
      rt.runtime->drainMicrotasks();
      return true;
    } catch (const jsi::JSError &err) {
      if (entrance.expired()) {
        return false;
      }
      report_uncaught(err);
    } catch (const std::exception &err) {
      if (entrance.expired()) {
        return false;
      }
      ibex2_report_uncaught(err.what());
    }
  }
}

} // namespace

extern "C" {

/// Create a runtime. `enable_eval == 0` closes JavaScript `eval` and the
/// Function constructor at construction time — LLP 0060 D4. Host-driven
/// evaluation through ibex2_hermes_eval is a separate path and stays available,
/// which is the whole point: the runtime can still run prepared code while
/// JavaScript cannot compile source of its own.
void *ibex2_hermes_create(int enable_eval) {
  // The engine microtask queue is OFF by default in stock Hermes. Ibex 2 needs
  // it on: it is what makes the job queue explicit and drainable, which is the
  // whole basis of the LLP 0058 §3 adapter. Without it there is no defined
  // point at which "microtasks have finished" is true.
  // The async break check is pinned for the same reason, not trusted to the
  // default (on, in this pin): it is what the deadline's stop depends on.
  // Without it evaluated source carries no check at its loop back-edges and
  // returns, `while (true) {}` runs until the process is killed, and an
  // effect that try/catches fails open — silently, since the monitor would
  // still fire and nothing would be there to hear it.
  auto config = ::hermes::vm::RuntimeConfig::Builder()
                    .withEnableEval(enable_eval != 0)
                    .withMicrotaskQueue(true)
                    .withAsyncBreakCheckInEval(true)
                    .build();
  // Fully qualified: `using namespace facebook` makes bare `hermes` ambiguous
  // between ::hermes (the VM namespace) and facebook::hermes (the JSI one).
  auto runtime = facebook::hermes::makeHermesRuntimeNoThrow(config);
  if (!runtime) {
    return nullptr;
  }
  auto *handle = new Ibex2Runtime();
  handle->runtime = std::move(runtime);
  handle->queue = ibex2_queue_create();
  handle->bindings = std::make_unique<Adapter>(*handle->runtime, handle->queue);
  return handle;
}

void ibex2_hermes_destroy(void *handle) {
  auto *rt = static_cast<Ibex2Runtime *>(handle);
  if (rt != nullptr) {
    // Workers may still hold the queue; releasing our reference is enough,
    // and the last holder frees it.
    for (const void *grants : rt->module_grants) {
      ibex2_grants_destroy(grants);
    }
    // Never watched between entrances, so this is belt and braces: the
    // monitor keys on the VM's address, and the VM is about to go. Teardown
    // runs no JavaScript, so there is nothing here for a deadline to stop.
    rt->runtime->unwatchTimeLimit();
    rt->bindings->detach();
    ibex2_queue_destroy(rt->queue);
  }
  delete rt;
}

/// Evaluate `source` as the host. Returns 0 on success and 1 if JavaScript
/// threw, with `*out` the result or the error message; 2 if the deadline
/// passed before or during the evaluation, with nothing in `*out` to trust.
int ibex2_hermes_eval(void *handle, const char *source, char **out) {
  auto *rt = static_cast<Ibex2Runtime *>(handle);
  if (rt == nullptr || rt->runtime == nullptr || source == nullptr) {
    return IBEX2_STATUS_INVALID;
  }
  Entrance entrance(*rt);
  if (entrance.refused()) {
    return IBEX2_STATUS_DEADLINE;
  }
  try {
    auto buffer = std::make_shared<jsi::StringBuffer>(std::string(source));
    jsi::Value value = rt->runtime->evaluateJavaScript(buffer, "<spike>");
    if (out != nullptr) {
      *out = dup_c_string(value.toString(*rt->runtime).utf8(*rt->runtime));
    }
    return entrance.status(IBEX2_STATUS_OK);
  } catch (const jsi::JSError &err) {
    if (entrance.expired()) {
      return IBEX2_STATUS_DEADLINE;
    }
    if (out != nullptr) {
      *out = dup_c_string(err.getMessage());
    }
    return IBEX2_STATUS_THREW;
  } catch (const std::exception &err) {
    if (entrance.expired()) {
      return IBEX2_STATUS_DEADLINE;
    }
    if (out != nullptr) {
      *out = dup_c_string(std::string(err.what()));
    }
    return IBEX2_STATUS_THREW;
  }
}

/// Install a host function that takes no arguments and returns a fixed string,
/// so the spike can prove the LLP 0058 requirement-2 path works on stock JSI
/// before any real capability is built on it.
int ibex2_hermes_install_probe(void *handle, const char *name,
                               const char *value) {
  auto *rt = static_cast<Ibex2Runtime *>(handle);
  if (rt == nullptr || rt->runtime == nullptr || name == nullptr ||
      value == nullptr) {
    return -1;
  }
  try {
    jsi::Runtime &runtime = *rt->runtime;
    std::string owned_value(value);
    auto prop = jsi::PropNameID::forUtf8(runtime, std::string(name));
    auto fn = jsi::Function::createFromHostFunction(
        runtime, prop, 0,
        [owned_value](jsi::Runtime &rt, const jsi::Value &,
                      const jsi::Value *, size_t) -> jsi::Value {
          return jsi::String::createFromUtf8(rt, owned_value);
        });
    runtime.global().setProperty(runtime, prop, std::move(fn));
    return 0;
  } catch (const std::exception &) {
    return 1;
  }
}

/// Install a host function that throws when called. Tests only: it is how
/// the pump's contract for a task that fails natively is held. `native != 0`
/// throws a std::runtime_error — which the pin translates into a JavaScript
/// error before the caller sees it — and `native == 0` throws a jsi::JSError.
/// Either way the message is `name` followed by " threw".
int ibex2_hermes_install_throwing_probe(void *handle, const char *name,
                                        int native) {
  auto *rt = static_cast<Ibex2Runtime *>(handle);
  if (rt == nullptr || rt->runtime == nullptr || name == nullptr) {
    return -1;
  }
  try {
    jsi::Runtime &runtime = *rt->runtime;
    std::string message = std::string(name) + " threw";
    auto prop = jsi::PropNameID::forUtf8(runtime, std::string(name));
    auto fn = jsi::Function::createFromHostFunction(
        runtime, prop, 0,
        [message, native](jsi::Runtime &rt, const jsi::Value &,
                          const jsi::Value *, size_t) -> jsi::Value {
          if (native != 0) {
            throw std::runtime_error(message);
          }
          throw jsi::JSError(rt, message);
        });
    runtime.global().setProperty(runtime, prop, std::move(fn));
    return 0;
  } catch (const std::exception &) {
    return 1;
  }
}

void ibex2_hermes_free_string(char *value) { std::free(value); }

} // extern "C"

// ---------------------------------------------------------------------------
// The host-call boundary (LLP 0059.000 §1), bridged onto stock JSI.
// ---------------------------------------------------------------------------

// Mirrors ibex2::boundary_abi::AbiValue. Kept in lockstep by the round-trip
// tests: any drift shows up as a wrong tag rather than silent corruption.
extern "C" int ibex2_async_begin(const void *queue, const void *grants,
                                 uint32_t op, const Ibex2AbiValue *argv,
                                 size_t argc, uint64_t task_id);
extern "C" int ibex2_wait_for_completion(const void *queue,
                                         unsigned long long timeout_ms);
extern "C" int ibex2_take_task(const void *queue, int *kind, unsigned long long *task_id,
                               Ibex2AbiValue *out, int *is_error);
extern "C" int ibex2_admit_due_timers(const void *queue);
extern "C" int ibex2_begin_drive(const void *queue);
extern "C" void ibex2_end_drive(const void *queue);
extern "C" int ibex2_loader_load(const void *state, const char *from,
                                 const char *specifier,
                                 Ibex2AbiValue *out_resolved,
                                 Ibex2AbiValue *out_source);
extern "C" const void *ibex2_loader_grants_for(const void *state,
                                               const char *specifier);
extern "C" double ibex2_millis_until_next_timer(const void *queue);
extern "C" int ibex2_response_field(const void *queue, double handle,
                                    uint32_t field, const Ibex2AbiValue *name,
                                    Ibex2AbiValue *out);


// ---------------------------------------------------------------------------
// The job-queue adapter (LLP 0058 §3 / OQ1).
//
// A delegating op returns a promise immediately, the work happens on another
// thread, and the completion is delivered here — on the JavaScript thread —
// by resolving the stored promise and then draining microtasks. The ordering
// contract is stated in Rust, in task::Pump::CONTRACT, and the tests hold this
// implementation to it.
// ---------------------------------------------------------------------------


namespace {

jsi::Function make_async_binding(jsi::Runtime &, const char *name,
                                 uint32_t op, Ibex2Runtime *owner,
                                 const void *grants) {
  return owner->bindings->async_binding(name, op, grants);
}

} // namespace

extern "C" {

int ibex2_hermes_collect_garbage(void *handle) {
  auto *rt = static_cast<Ibex2Runtime *>(handle);
  if (rt == nullptr) return 1;
  try {
    rt->runtime->instrumentation().collectGarbage("host requested collection");
    return 0;
  } catch (const std::exception &) {
    return 1;
  }
}

/// Run at most ONE host task, with a microtask checkpoint either side.
///
/// The depth-zero drive cycle of LLP 0058.000.000 §8, in order:
///
///   1. PreCheckpoint — drain microtasks already queued.
///   2. Admit timers that have come due into the one host-task FIFO.
///   3. Reserve at most the oldest ready task.
///   4. Run it.
///   5. PostCheckpoint — drain the microtasks it caused, before any next task.
///
/// `*out_ran` is 1 if a task ran and 0 otherwise. **One task per cycle**, so a
/// caller wanting to run the loop to quiescence calls this in a loop rather
/// than expecting one call to drain everything. Batching tasks would make the
/// order an application sees depend on how many happened to be ready at once.
///
/// Returns 0, or 2 if the deadline passed before or during the cycle — at the
/// door, in a checkpoint, or inside the task, which is then abandoned with no
/// post-checkpoint: nothing runs after the deadline. A callback or job that
/// throws for any other reason is a console error, never a status.
int ibex2_hermes_pump(void *handle, int *out_ran) {
  if (out_ran != nullptr) {
    *out_ran = 0;
  }
  auto *rt = static_cast<Ibex2Runtime *>(handle);
  if (rt == nullptr || rt->runtime == nullptr) {
    return IBEX2_STATUS_INVALID;
  }
  Entrance entrance(*rt);
  if (entrance.refused()) {
    return IBEX2_STATUS_DEADLINE;
  }
  // A drive request made while a cycle is running records a wakeup rather than
  // nesting a second host task inside project JavaScript (§8).
  if (ibex2_begin_drive(rt->queue) == 0) {
    return entrance.status(IBEX2_STATUS_OK);
  }
  DriveGuard drive(rt->queue);
  jsi::Runtime &runtime = *rt->runtime;

  // 1. PreCheckpoint.
  if (!checkpoint(*rt, entrance)) {
    return IBEX2_STATUS_DEADLINE;
  }

  // 2. Admission.
  ibex2_admit_due_timers(rt->queue);

  // 3. Reserve at most one.
  int kind = 0;
  unsigned long long task_id = 0;
  Ibex2AbiValue value{IBEX2_TAG_UNDEFINED, 0.0, nullptr, 0};
  int is_error = 0;
  if (ibex2_take_task(rt->queue, &kind, &task_id, &value, &is_error) == 0) {
    return entrance.status(IBEX2_STATUS_OK);
  }

  // 4. Run it.
  try {
    if (kind == 2) {
      jsi::Value fire = runtime.global().getProperty(runtime, "__ibex2_fire_timer");
      if (fire.isObject() && fire.getObject(runtime).isFunction(runtime)) {
        fire.getObject(runtime).getFunction(runtime).call(
            runtime, static_cast<double>(task_id));
      }
    } else {
      rt->bindings->settle(task_id, value, is_error != 0);
    }
  } catch (const jsi::JSError &err) {
    if (entrance.expired()) {
      return IBEX2_STATUS_DEADLINE;
    }
    // A throwing callback does not stop the tasks behind it, exactly as an
    // unhandled error in one task does not cancel the next — but it is
    // reported, as a console error, rather than lost.
    report_uncaught(err);
  } catch (const std::exception &err) {
    // The same for what the engine or the adapter throws natively — a
    // JSINativeException out of `call`, a settlement that cannot be built —
    // for parity with the checkpoint and every other entrance. Nothing
    // unwinds through this `extern "C"` boundary. (A host function's own
    // std::exception never arrives here: the pin translates it into a
    // JavaScript error inside the callback, so it is caught above.)
    if (entrance.expired()) {
      return IBEX2_STATUS_DEADLINE;
    }
    ibex2_report_uncaught(err.what());
  }

  // 5. PostCheckpoint.
  if (!checkpoint(*rt, entrance)) {
    return IBEX2_STATUS_DEADLINE;
  }
  if (out_ran != nullptr) {
    *out_ran = 1;
  }
  return entrance.status(IBEX2_STATUS_OK);
}

/// Block until a host task is ready, or the timeout elapses.
///
/// A real embedder calls this instead of spinning: it wakes when there is work
/// rather than burning a core to discover there is none.
int ibex2_hermes_wait(void *handle, unsigned long long timeout_ms) {
  auto *rt = static_cast<Ibex2Runtime *>(handle);
  if (rt == nullptr) {
    return 0;
  }
  return ibex2_wait_for_completion(rt->queue, timeout_ms);
}

/// Drain microtasks without delivering completions — the "JavaScript ran and
/// then yielded" step, used by the tests to observe ordering.
///
/// Returns 0; 1 if a job threw out of the queue, with the message in `*out`;
/// 2 if the deadline passed before or during the drain.
int ibex2_hermes_drain_microtasks(void *handle, char **out) {
  auto *rt = static_cast<Ibex2Runtime *>(handle);
  if (rt == nullptr || rt->runtime == nullptr) {
    return IBEX2_STATUS_INVALID;
  }
  Entrance entrance(*rt);
  if (entrance.refused()) {
    return IBEX2_STATUS_DEADLINE;
  }
  try {
    rt->runtime->drainMicrotasks();
    return entrance.status(IBEX2_STATUS_OK);
  } catch (const jsi::JSError &err) {
    if (entrance.expired()) {
      return IBEX2_STATUS_DEADLINE;
    }
    if (out != nullptr) {
      *out = dup_c_string(err.getMessage());
    }
    return IBEX2_STATUS_THREW;
  } catch (const std::exception &err) {
    if (entrance.expired()) {
      return IBEX2_STATUS_DEADLINE;
    }
    if (out != nullptr) {
      *out = dup_c_string(std::string(err.what()));
    }
    return IBEX2_STATUS_THREW;
  }
}

} // extern "C"

namespace {

/// A jsi::Buffer over bytes the caller owns for the duration of the call.
///
/// Safe for SOURCE, which Hermes parses and copies. **Not safe for bytecode**:
/// Hermes keeps the buffer and reads from it for as long as the module lives,
/// which is what makes HBC mmap-able and copy-free. Use OwnedBytes there.
class BorrowedBytes : public jsi::Buffer {
public:
  BorrowedBytes(const unsigned char *data, size_t len) : data_(data), len_(len) {}
  size_t size() const override { return len_; }
  const uint8_t *data() const override { return data_; }

private:
  const unsigned char *data_;
  size_t len_;
};

/// A jsi::Buffer that OWNS its bytes for as long as Hermes holds the buffer.
///
/// Required for bytecode. A borrowed buffer over a local vector produced a
/// module whose synchronous body ran correctly and whose callbacks later
/// executed against freed memory — the failure was silent, because the
/// bytecode was gone rather than wrong.
class OwnedBytes : public jsi::Buffer {
public:
  explicit OwnedBytes(std::vector<unsigned char> bytes)
      : bytes_(std::move(bytes)) {}
  size_t size() const override { return bytes_.size(); }
  const uint8_t *data() const override { return bytes_.data(); }

private:
  std::vector<unsigned char> bytes_;
};

std::shared_ptr<jsi::Value> load_module(jsi::Runtime &rt, Ibex2Runtime *owner,
                                        const std::string &from,
                                        const std::string &specifier);

// `require`, closed over the specifier of the module that holds it — so a
// relative path resolves against the right file, and a module cannot claim to
// be somewhere else to change what it can reach.
jsi::Function make_require(jsi::Runtime &runtime, Ibex2Runtime *owner,
                           const std::string &self) {
  return jsi::Function::createFromHostFunction(
      runtime, jsi::PropNameID::forAscii(runtime, "require"), 1,
      [owner, self](jsi::Runtime &rt, const jsi::Value &,
                    const jsi::Value *args, size_t count) -> jsi::Value {
        if (count < 1 || !args[0].isString()) {
          throw jsi::JSError(rt, "require expects a specifier");
        }
        auto exports =
            load_module(rt, owner, self, args[0].getString(rt).utf8(rt));
        return jsi::Value(rt, *exports);
      });
}

std::shared_ptr<jsi::Value> load_module(jsi::Runtime &rt, Ibex2Runtime *owner,
                                        const std::string &from,
                                        const std::string &specifier) {
  Ibex2AbiValue resolved{IBEX2_TAG_UNDEFINED, 0.0, nullptr, 0};
  Ibex2AbiValue source{IBEX2_TAG_UNDEFINED, 0.0, nullptr, 0};
  int status = ibex2_loader_load(owner->queue, from.c_str(), specifier.c_str(),
                                 &resolved, &source);
  std::string resolved_name(reinterpret_cast<const char *>(resolved.data),
                            resolved.len);
  if (status != 0) {
    ibex2_host_release(&resolved);
    ibex2_host_release(&source);
    throw jsi::JSError(rt, resolved_name);
  }
  // Bytes, not text: Hermes bytecode when the loader has a compiler, wrapped
  // source when it does not. Hermes detects the HBC magic, so the SAME call
  // handles both and the loader decides which without the shim caring.
  std::vector<unsigned char> module_bytes(source.data, source.data + source.len);
  ibex2_host_release(&resolved);
  ibex2_host_release(&source);

  // A cycle returns the partial exports rather than recursing forever, which
  // is what CommonJS does and what makes mutually-importing modules terminate.
  auto existing = owner->modules.find(resolved_name);
  if (existing != owner->modules.end()) {
    return existing->second;
  }

  auto module = std::make_shared<jsi::Object>(rt);
  auto exports = std::make_shared<jsi::Object>(rt);
  module->setProperty(rt, "exports", jsi::Value(rt, *exports));
  // Registered BEFORE evaluation, so a cycle finds this entry.
  auto partial = std::make_shared<jsi::Value>(rt, *exports);
  owner->modules[resolved_name] = partial;

  // A function EXPRESSION, evaluated by the host. new Function cannot be used
  // — dynamic code is closed at construction (LLP 0060 D4) — which is why the
  // loader lives here and not in JavaScript. The wrapper text itself is built
  // in Rust, because it is what gets compiled and the artifact is keyed on it.
  // OwnedBytes, not BorrowedBytes: Hermes retains a bytecode buffer for the
  // life of the module.
  auto buffer = std::make_shared<OwnedBytes>(std::move(module_bytes));
  jsi::Value fn_value = rt.evaluateJavaScript(buffer, resolved_name);
  if (!fn_value.isObject() || !fn_value.getObject(rt).isFunction(rt)) {
    owner->modules.erase(resolved_name);
    throw jsi::JSError(rt, "module wrapper did not evaluate to a function");
  }

  // This module's own authority, captured now and carried by its binding for
  // the binding's whole life (LLP 0060 D1). A module granted nothing receives
  // a fetch that refuses everything — not an absent fetch, so the failure is a
  // denial rather than a TypeError.
  const void *grants =
      ibex2_loader_grants_for(owner->queue, resolved_name.c_str());

  // The bindings for this grant set — built the first time any module holds
  // it, shared by every module after. Before this, every load built a
  // `fetch`, ten `fs` methods, `process`, and `env` of its own: a dozen JSI
  // objects per module, used or not, which doubled the per-module load cost
  // (issues/20260829-per-module-bindings-doubled-load-cost.md). Authority is
  // unchanged — a binding carries its grant either way — and integrity is
  // kept by freezing what is shared.
  auto shared = owner->shared.find(grants);
  if (shared == owner->shared.end()) {
    if (grants != nullptr) {
      owner->module_grants.push_back(grants);
    }

    jsi::Value fetch_binding = jsi::Value(
        rt, make_async_binding(rt, "fetch", 101, owner, grants));
    if (owner->make_fetch.isObject() &&
        owner->make_fetch.getObject(rt).isFunction(rt)) {
      fetch_binding = owner->make_fetch.getObject(rt).getFunction(rt).call(
          rt, std::move(fetch_binding));
    }

    auto storage = owner->bindings->storage(
        grants, owner->make_sqlite.getObject(rt).getFunction(rt));
    auto fs = storage.getPropertyAsObject(rt, "fs");
    auto sqlite_binding = storage.getProperty(rt, "sqlite");

    // `process.env` is a SNAPSHOT of exactly the variables this grant set
    // names (LLP 0059.000 §3.8), not a live proxy and not the real
    // environment. There is no read-time check because there is nothing to
    // check: an ungranted variable is undefined because it is not in the
    // object. That is the whole capability model in one object — a package
    // reading AWS_SECRET_ACCESS_KEY finds undefined unless someone said
    // otherwise.
    jsi::Object process(rt);
    jsi::Object env(rt);
    size_t env_count = ibex2_grants_env_count(grants);
    for (size_t i = 0; i < env_count; ++i) {
      char *name = nullptr;
      char *value = nullptr;
      if (ibex2_grants_env_at(grants, i, &name, &value) == 0) {
        continue;
      }
      env.setProperty(rt, jsi::PropNameID::forUtf8(rt, std::string(name)),
                      jsi::String::createFromUtf8(rt, std::string(value)));
      ibex2_string_free(name);
      ibex2_string_free(value);
    }
    jsi::Value env_value(rt, env);
    freeze(rt, env_value);
    process.setProperty(rt, jsi::PropNameID::forAscii(rt, "env"), env_value);

    jsi::Value fs_value(rt, fs);
    jsi::Value process_value(rt, process);
    freeze(rt, fs_value);
    freeze(rt, process_value);
    freeze(rt, fetch_binding);
    shared = owner->shared
                 .emplace(grants, Ibex2Runtime::SharedBindings{
                                      std::move(fetch_binding),
                                      std::move(fs_value),
                                      std::move(process_value),
                                      std::move(sqlite_binding)})
                 .first;
  } else if (grants != nullptr) {
    // The same interned set: one reference is enough to keep it alive.
    ibex2_grants_destroy(grants);
  }

  // import.meta, per module. LLP 0023 §6 gives a file-backed module the
  // virtual `file:///project/...` URL; that namespace does not exist yet, so
  // this uses the same shape over the resolved specifier and will move to the
  // VFS when there is one.
  jsi::Object meta(rt);
  std::string module_url = "file:///project/" + resolved_name.substr(
      resolved_name.rfind("./", 0) == 0 ? 2 : 0);
  meta.setProperty(rt, jsi::PropNameID::forAscii(rt, "url"),
                   jsi::String::createFromUtf8(rt, module_url));

  fn_value.getObject(rt).getFunction(rt).call(
      rt, jsi::Value(rt, *module), jsi::Value(rt, *exports),
      jsi::Value(rt, make_require(rt, owner, resolved_name)),
      jsi::Value(rt, shared->second.fetch), jsi::Value(rt, shared->second.fs),
      jsi::Value(rt, shared->second.process), std::move(meta),
      jsi::Value(rt, shared->second.sqlite));

  // `module.exports = ...` replaces the value, so re-read it after running.
  // Whatever it is — object, function, string, number — is what `require`
  // returns; the only thing that must not be lost is the identity of the
  // original object when it was kept, so a cycle's partial view stays the
  // same object as the final one.
  auto final_exports =
      std::make_shared<jsi::Value>(rt, module->getProperty(rt, "exports"));
  owner->modules[resolved_name] = final_exports;
  return final_exports;
}

} // namespace

extern "C" {

/// Evaluate a buffer that may contain Hermes bytecode.
///
/// Hermes detects the HBC magic and takes the bytecode path, so this is the
/// same entry point as source with a different payload — which is exactly what
/// makes the comparison between them fair.
///
/// Same statuses as ibex2_hermes_eval. Bytecode carries only the break checks
/// it was compiled with: hermesc emits none by default, so such a program is
/// not stopped mid-way, but it is refused at the door and reported at exit
/// like any other entrance.
int ibex2_hermes_eval_bytes(void *handle, const unsigned char *data, size_t len,
                            char **out) {
  auto *rt = static_cast<Ibex2Runtime *>(handle);
  if (rt == nullptr || rt->runtime == nullptr || data == nullptr) {
    return IBEX2_STATUS_INVALID;
  }
  Entrance entrance(*rt);
  if (entrance.refused()) {
    return IBEX2_STATUS_DEADLINE;
  }

  try {
    // Copied, for the same reason load_module owns its bytes: anything the
    // evaluated code leaves behind outlives this call.
    auto buffer = std::make_shared<OwnedBytes>(
        std::vector<unsigned char>(data, data + len));
    jsi::Value value = rt->runtime->evaluateJavaScript(buffer, "<hbc>");
    if (out != nullptr) {
      *out = dup_c_string(value.isUndefined()
                              ? std::string("undefined")
                              : value.toString(*rt->runtime).utf8(*rt->runtime));
    }
    return entrance.status(IBEX2_STATUS_OK);
  } catch (const jsi::JSError &err) {
    if (entrance.expired()) {
      return IBEX2_STATUS_DEADLINE;
    }
    if (out != nullptr) {
      *out = dup_c_string(err.getMessage());
    }
    return IBEX2_STATUS_THREW;
  } catch (const std::exception &err) {
    if (entrance.expired()) {
      return IBEX2_STATUS_DEADLINE;
    }
    if (out != nullptr) {
      *out = dup_c_string(std::string(err.what()));
    }
    return IBEX2_STATUS_THREW;
  }
}

/// This runtime's Rust-side state, for callers that need it directly.
const void *ibex2_hermes_state(void *handle) {
  auto *rt = static_cast<Ibex2Runtime *>(handle);
  return rt == nullptr ? nullptr : rt->queue;
}

/// Load and run an entry module. Returns 0 on success, 1 on failure with the
/// message in `out_error` (Rust-released), 2 if the deadline passed.
///
/// The microtasks the module's body queued are drained through the same
/// checkpoint as `pump` and `drain_microtasks`: a job that throws out of the
/// queue is a console error and the drain resumes behind it, and only the
/// deadline ends the drain early. A failure to load or run the module body
/// itself is still the entry's own failure, reported through `out_error`.
int ibex2_hermes_run_entry(void *handle, const char *specifier,
                           char **out_error) {
  auto *rt = static_cast<Ibex2Runtime *>(handle);
  if (rt == nullptr || rt->runtime == nullptr) {
    return IBEX2_STATUS_INVALID;
  }
  Entrance entrance(*rt);
  if (entrance.refused()) {
    return IBEX2_STATUS_DEADLINE;
  }
  try {
    load_module(*rt->runtime, rt, "./", specifier);
    if (!checkpoint(*rt, entrance)) {
      return IBEX2_STATUS_DEADLINE;
    }
    return entrance.status(IBEX2_STATUS_OK);
  } catch (const jsi::JSError &e) {
    if (entrance.expired()) {
      return IBEX2_STATUS_DEADLINE;
    }
    if (out_error != nullptr) {
      *out_error = dup_c_string(e.getMessage());
    }
    return IBEX2_STATUS_THREW;
  } catch (const std::exception &e) {
    if (entrance.expired()) {
      return IBEX2_STATUS_DEADLINE;
    }
    if (out_error != nullptr) {
      *out_error = dup_c_string(std::string(e.what()));
    }
    return IBEX2_STATUS_THREW;
  }
}

/// Arm the runtime's one deadline, `remaining_nanos` from now on the steady
/// clock, replacing any deadline already armed. Every entrance from here to
/// ibex2_hermes_clear_deadline is refused once it has passed and stopped if it
/// passes while JavaScript runs; each is given what is left of this deadline,
/// never a fresh budget.
int ibex2_hermes_set_deadline(void *handle, uint64_t remaining_nanos) {
  auto *rt = static_cast<Ibex2Runtime *>(handle);
  if (rt == nullptr || rt->runtime == nullptr) {
    return IBEX2_STATUS_INVALID;
  }
  // Half the clock's range is still centuries; past that the sum would wrap.
  const uint64_t farthest =
      static_cast<uint64_t>(std::chrono::nanoseconds::max().count() / 2);
  auto remaining = std::chrono::nanoseconds(
      static_cast<int64_t>(remaining_nanos < farthest ? remaining_nanos : farthest));
  rt->deadline = std::chrono::steady_clock::now() + remaining;
  rt->deadline_armed = true;
  return IBEX2_STATUS_OK;
}

/// Disarm the deadline. A runtime whose deadline fired is usable again after
/// this; its JavaScript state is whatever the stop left.
int ibex2_hermes_clear_deadline(void *handle) {
  auto *rt = static_cast<Ibex2Runtime *>(handle);
  if (rt == nullptr || rt->runtime == nullptr) {
    return IBEX2_STATUS_INVALID;
  }
  rt->deadline_armed = false;
  return IBEX2_STATUS_OK;
}

} // extern "C"

extern "C" {

/// Install `fetch`, bound to the grants it will carry for its whole lifetime.
///
/// The grants are captured HERE, at install time, and handed back on every
/// call. That is LLP 0060 D1 made concrete: two runtimes — or two bindings —
/// can be given different authority for identical JavaScript, and neither can
/// reach the other's.
/// Evaluate `bindings/fetch.js` (as bytecode) and keep its completion value,
/// the fetch factory, off the global object.
int ibex2_hermes_install_fetch_factory(void *handle, const unsigned char *bytes,
                                       size_t len) {
  auto *rt = static_cast<Ibex2Runtime *>(handle);
  if (rt == nullptr || bytes == nullptr) {
    return 1;
  }
  try {
    auto buffer = std::make_shared<OwnedBytes>(
        std::vector<unsigned char>(bytes, bytes + len));
    jsi::Value factory = rt->runtime->evaluateJavaScript(buffer, "fetch.js");
    if (!factory.isObject() || !factory.getObject(*rt->runtime).isFunction(*rt->runtime)) {
      return 1;
    }
    rt->make_fetch = std::move(factory);
    return 0;
  } catch (const jsi::JSError &) {
    return 1;
  }
}

int ibex2_hermes_install_sqlite_factory(void *handle, const unsigned char *bytes,
                                        size_t len) {
  auto *rt = static_cast<Ibex2Runtime *>(handle);
  if (rt == nullptr || bytes == nullptr) return 1;
  try {
    auto buffer = std::make_shared<OwnedBytes>(
        std::vector<unsigned char>(bytes, bytes + len));
    auto factory = rt->runtime->evaluateJavaScript(buffer, "sqlite.js");
    if (!factory.isObject() || !factory.getObject(*rt->runtime).isFunction(*rt->runtime))
      return 1;
    rt->make_sqlite = std::move(factory);
    return 0;
  } catch (const jsi::JSError &) { return 1; }
}

int ibex2_hermes_accept_intl_intrinsics(void *handle) {
  auto *rt = static_cast<Ibex2Runtime *>(handle);
  if (rt == nullptr || rt->runtime == nullptr || rt->bindings == nullptr)
    return 1;
#if defined(__linux__)
  try {
    auto &runtime = *rt->runtime;
    auto global = runtime.global();
    auto accept = [&](const char *constructor, const char *property) {
      auto prototype = global.getPropertyAsObject(runtime, constructor)
                           .getPropertyAsObject(runtime, "prototype");
      rt->bindings->accept_trusted_intrinsic_property(std::move(prototype),
                                                       property);
    };
    // The Integrity snapshot predates trusted bytecode installation. Admit
    // exactly the standard methods the Linux Intl completion replaces; all
    // other captured identities stay anchored, so a pre-hardening mutation
    // elsewhere is still refused by SQLite.
    accept("Number", "toLocaleString");
    accept("BigInt", "toLocaleString");
    accept("String", "toLocaleLowerCase");
    accept("String", "toLocaleUpperCase");
    return 0;
  } catch (...) {
    return 1;
  }
#else
  return 0;
#endif
}

int ibex2_hermes_install_intl_datetime(void *handle,
                                       const unsigned char *bytes,
                                       size_t len) {
  auto *rt = static_cast<Ibex2Runtime *>(handle);
  if (rt == nullptr || rt->runtime == nullptr || bytes == nullptr) return 1;
#if defined(__linux__)
  try {
    auto &runtime = *rt->runtime;
    auto buffer = std::make_shared<OwnedBytes>(
        std::vector<unsigned char>(bytes, bytes + len));
    auto value = runtime.evaluateJavaScript(buffer, "intl_datetime.js");
    if (!value.isObject() || !value.getObject(runtime).isFunction(runtime))
      return 1;
    auto arguments =
        ibex2::intl_datetime::factory_arguments(runtime, rt->queue);
    value.getObject(runtime).getFunction(runtime).call(
        runtime, static_cast<const jsi::Value *>(arguments.data()),
        arguments.size());
    return 0;
  } catch (...) {
    return 1;
  }
#else
  (void)len;
  return 1;
#endif
}

int ibex2_hermes_install_fetch(void *handle, const void *grants) {
  auto *rt = static_cast<Ibex2Runtime *>(handle);
  if (rt == nullptr || rt->runtime == nullptr) {
    return -1;
  }
  try {
    jsi::Runtime &runtime = *rt->runtime;
    jsi::Object global = runtime.global();
    global.setProperty(
        runtime, jsi::PropNameID::forAscii(runtime, "__ibex2_fetch"),
        make_async_binding(runtime, "__ibex2_fetch", 101, rt, grants));

    return 0;
  } catch (const std::exception &) {
    return 1;
  }
}

/// Install the pure tier: console, btoa/atob, and a raw host-call escape hatch.
int ibex2_hermes_install_stdlib(void *handle) {
  auto *rt = static_cast<Ibex2Runtime *>(handle);
  if (rt == nullptr || rt->runtime == nullptr) {
    return -1;
  }
  try {
    jsi::Runtime &runtime = *rt->runtime;
    jsi::Object global = runtime.global();

    jsi::Object console(runtime);
    set_binding(runtime, console, "log", 1, rt->queue);
    set_binding(runtime, console, "info", 2, rt->queue);
    set_binding(runtime, console, "debug", 3, rt->queue);
    set_binding(runtime, console, "warn", 4, rt->queue);
    set_binding(runtime, console, "error", 5, rt->queue);
    global.setProperty(runtime, jsi::PropNameID::forAscii(runtime, "console"),
                       std::move(console));

    // `btoa`/`atob` are the engine's own (Tier E): Hermes provides both
    // natively and identically, so the Rust ones behind ops 10/11 are not
    // bound — they stay for a Rust consumer of the standard library.
    set_binding(runtime, global, "__ibex2_random_uuid", 70, rt->queue);
    set_binding(runtime, global, "__ibex2_get_random_values", 71, rt->queue);
    set_binding(runtime, global, "__ibex2_fetch_control", 72, rt->queue);
    global.setProperty(runtime, "__ibex2_response_own",
        jsi::Function::createFromHostFunction(runtime,
            jsi::PropNameID::forAscii(runtime, "__ibex2_response_own"), 2,
            [rt](jsi::Runtime &r, const jsi::Value &, const jsi::Value *args,
                 size_t count) -> jsi::Value {
              if (count != 2 || !args[0].isNumber() || !args[1].isObject())
                throw jsi::JSError(r, "response owner needs a handle and a body");
              auto body = args[1].getObject(r);
              body.setNativeState(r, std::make_shared<ResponseOwner>(
                  ibex2_response_owner_create(rt->queue, args[0].asNumber())));
              auto weak = std::make_shared<jsi::WeakObject>(r, body);
              return jsi::Function::createFromHostFunction(r,
                  jsi::PropNameID::forAscii(r, "responseBody"), 0,
                  [weak](jsi::Runtime &r, const jsi::Value &,
                         const jsi::Value *, size_t) -> jsi::Value {
                    return weak->lock(r);
                  });
            }));
    global.setProperty(runtime, "__ibex2_response_read",
        make_async_binding(runtime, "__ibex2_response_read", 102, rt, nullptr));
    set_binding(runtime, global, "__ibex2_text_encode", 20, rt->queue);
    set_binding(runtime, global, "__ibex2_text_decode", 21, rt->queue);
    set_binding(runtime, global, "__ibex2_text_encode_into", 22, rt->queue);
    set_binding(runtime, global, "__ibex2_url_parse", 30, rt->queue);
    set_binding(runtime, global, "__ibex2_url_set", 32, rt->queue);
    set_binding(runtime, global, "__ibex2_search_params_normalize", 29, rt->queue);
    set_binding(runtime, global, "__ibex2_search_params_get", 31, rt->queue);
    set_binding(runtime, global, "__ibex2_search_params_get_all", 33, rt->queue);
    set_binding(runtime, global, "__ibex2_search_params_has", 34, rt->queue);
    set_binding(runtime, global, "__ibex2_search_params_set", 35, rt->queue);
    set_binding(runtime, global, "__ibex2_search_params_append", 36, rt->queue);
    set_binding(runtime, global, "__ibex2_search_params_delete", 37, rt->queue);
    set_binding(runtime, global, "__ibex2_search_params_sort", 38, rt->queue);
    set_binding(runtime, global, "__ibex2_search_params_entries", 39, rt->queue);

    // The ops behind the Headers class. Rust owns the semantics; the class
    // shape is in bindings/headers.js.
    jsi::Object headers(runtime);
    set_binding(runtime, headers, "create", 40, rt->queue);
    set_binding(runtime, headers, "append", 41, rt->queue);
    set_binding(runtime, headers, "set", 42, rt->queue);
    set_binding(runtime, headers, "get", 43, rt->queue);
    set_binding(runtime, headers, "has", 44, rt->queue);
    set_binding(runtime, headers, "remove", 45, rt->queue);
    set_binding(runtime, headers, "count", 46, rt->queue);
    set_binding(runtime, headers, "nameAt", 47, rt->queue);
    set_binding(runtime, headers, "valueAt", 48, rt->queue);
    set_binding(runtime, headers, "validName", 49, rt->queue);
    set_binding(runtime, headers, "validValue", 50, rt->queue);
    set_binding(runtime, headers, "free", 51, rt->queue);
    global.setProperty(runtime,
                       jsi::PropNameID::forAscii(runtime, "__ibex2_headers"),
                       std::move(headers));

    // Response accessors, in the UNGATED tier: a response crosses as a handle,
    // and the handle is the authority. Reading a status off one you already
    // hold conveys nothing further, and handles only come from a granted fetch.
    // They were installed by install_fetch, which the per-module loader never
    // calls — so a module with a granted fetch could not read its response.
    auto field_fn = jsi::Function::createFromHostFunction(
        runtime, jsi::PropNameID::forAscii(runtime, "__ibex2_response_field"), 3,
        [rt](jsi::Runtime &r, const jsi::Value &, const jsi::Value *args,
             size_t count) -> jsi::Value {
          if (count < 2) {
            throw jsi::JSError(r, "response field needs a handle and a field id");
          }
          std::vector<std::string> owned;
          Ibex2AbiValue name{IBEX2_TAG_UNDEFINED, 0.0, nullptr, 0};
          if (count >= 3) {
            name = to_abi(r, args[2], owned);
          }
          Ibex2AbiValue out{IBEX2_TAG_UNDEFINED, 0.0, nullptr, 0};
          int status = ibex2_response_field(
              rt->queue, args[0].asNumber(),
              static_cast<uint32_t>(args[1].asNumber()),
              count >= 3 ? &name : nullptr, &out);
          jsi::Value result = from_abi(r, out);
          ibex2_host_release(&out);
          if (status != 0) {
            throw jsi::JSError(r, result.isString()
                                      ? result.getString(r).utf8(r)
                                      : std::string("response read failed"));
          }
          return result;
        });
    global.setProperty(runtime,
                       jsi::PropNameID::forAscii(runtime, "__ibex2_response_field"),
                       std::move(field_fn));
    set_binding(runtime, global, "__ibex2_timer_set", 60, rt->queue);
    set_binding(runtime, global, "__ibex2_timer_set_repeating", 61, rt->queue);
    set_binding(runtime, global, "__ibex2_timer_clear", 62, rt->queue);
    set_binding(runtime, global, "__ibex2_performance_now", 63, rt->queue);

#if defined(__linux__)
    ibex2::intl_number_format::install(runtime, rt->queue);
    ibex2::intl_case::install(runtime, rt->queue);
#endif

    return 0;
  } catch (const std::exception &) {
    return 1;
  }
}

/// The echo op: a delegating operation with no transport behind it, which
/// holds the adapter to its ordering contract. Tests only — it was in the
/// standard library and so on every program's global object, which is one
/// more name than R5 allows.
int ibex2_hermes_install_async_echo(void *handle) {
  auto *rt = static_cast<Ibex2Runtime *>(handle);
  if (rt == nullptr) {
    return 1;
  }
  try {
    auto &runtime = *rt->runtime;
    runtime.global().setProperty(
        runtime, jsi::PropNameID::forAscii(runtime, "__ibex2_async_echo"),
        make_async_binding(runtime, "__ibex2_async_echo", 100, rt, nullptr));
    return 0;
  } catch (const std::exception &) {
    return 1;
  }
}

} // extern "C"
