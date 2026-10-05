#pragma once
// JSI-only bindings. Compile ibex2_jsi.cc with the embedding engine's JSI headers.
// @ref LLP 0068#1-the-shape — one standard library, caller-owned engine and loop
#include <jsi/jsi.h>
#include <cstddef>
#include <cstdint>
#include <memory>
#include <string>
#include <vector>

// Opaque Rust-owned endowment handle. Context::bindings_ptr() produces a
// borrowed handle; ibex2_bindings_adopt produces an adopted handle whose state
// identity matches an owning runtime. Release every adopted handle with
// ibex2_bindings_destroy; the Context retains and releases its borrowed handle.
// In particular, a grant pointer is not an install handle.
struct Ibex2Bindings;

struct Ibex2AbiValue {
  int32_t tag;
  double number;
  const unsigned char *data;
  size_t len;
};

enum : int32_t {
  IBEX2_TAG_UNDEFINED = 0,
  IBEX2_TAG_NULL = 1,
  IBEX2_TAG_BOOL = 2,
  IBEX2_TAG_NUMBER = 3,
  IBEX2_TAG_STRING = 4,
  IBEX2_TAG_BYTES = 5,
};


namespace ibex2::jsi_adapter {
namespace jsi = facebook::jsi;
using Groups = uint16_t;
inline constexpr Groups GROUP_PURE = 1u << 0;
inline constexpr Groups GROUP_CONSOLE = 1u << 1;
inline constexpr Groups GROUP_TIMERS = 1u << 2;
inline constexpr Groups GROUP_ABORT = 1u << 3;
inline constexpr Groups GROUP_CRYPTO = 1u << 4;
inline constexpr Groups GROUP_FETCH = 1u << 5;
inline constexpr Groups GROUP_STORAGE = 1u << 6;
inline constexpr Groups GROUP_ENV = 1u << 7;
inline constexpr Groups GROUP_SECRETS = 1u << 8;
inline constexpr Groups GROUP_KV = 1u << 9;
inline constexpr Groups GROUP_INTL = 1u << 10;

// The dependency/availability table used by Adapter::install. Exposed so an
// embedder can mechanically compare its public group-selection rules.
void validate_groups(Groups groups);
// The script names Adapter::install expects, in installation order. This is
// exposed with validate_groups so cross-language embedders can mechanically
// verify that their compiler inputs stay in lockstep with the adapter.
std::vector<const char*> expected_scripts(Groups groups);

struct CompiledScript {
  const char* name;
  const uint8_t* bytes;
  size_t len;
};

Ibex2AbiValue to_abi(jsi::Runtime&, const jsi::Value&, std::vector<std::string>&);
jsi::Value from_abi(jsi::Runtime&, Ibex2AbiValue&);
struct HostCallResult {
  int status;
  jsi::Value value;
};
HostCallResult call_host_result(jsi::Runtime&, const void*, uint32_t,
                                const jsi::Value*, size_t);

// Shared by every native closure installed through Adapter. The closure asks
// for the borrowed Rust state at call time, after checking detach, so keeping
// a JavaScript function alive cannot keep or later dereference that borrow.
class Lifetime {
public:
  const void* require(jsi::Runtime&) const;
private:
  friend class Adapter;
  explicit Lifetime(const void* state) : state_(state) {}
  void detach() { alive_ = false; state_ = nullptr; }
  bool alive_ = true;
  const void* state_;
};

jsi::Function make_host_binding(jsi::Runtime&, const char*, uint32_t, const void*);
void set_binding(jsi::Runtime&, jsi::Object&, const char*, uint32_t, const void*);

// All methods, including detach/destruction, run on the runtime's owner thread.
// The runtime and borrowed Rust queue must outlive detach. One adapter owns the
// queue's task-id namespace. The caller owns checkpoints, scheduling and timers.
// Construct before application code, then run the precompiled HARDEN_SOURCE
// before application code uses storage. SQLite refuses mutable or replaced
// intrinsics, including methods changed before a later freeze.
// Retained JavaScript bindings fail closed after detach; they never dereference
// a destroyed adapter. Detach clears all JSI roots before the runtime is destroyed.
class Adapter {
public:
  // `bytecode_version` is the owning engine's supported HBC version when it
  // exposes one. Zero keeps the JSI-only fallback at magic validation.
  Adapter(jsi::Runtime&, const void* borrowed_queue,
          uint32_t bytecode_version = 0);
  ~Adapter();
  Adapter(const Adapter&) = delete;
  Adapter& operator=(const Adapter&) = delete;
  void detach();
  // Internal companion installers (the Linux Intl shims) capture the same
  // token as the core group installers.
  std::shared_ptr<Lifetime> lifetime() const;
  // Update only an already-captured intrinsic property's expected identity
  // after Ibex's trusted bootstrap replaces that property. Every other
  // captured identity remains anchored to runtime construction.
  void accept_trusted_intrinsic_property(jsi::Object, const char* name);
  // Install exactly `groups`. `scripts` must be the compiled results of
  // bindings::scripts(groups), in that order, from the compiler belonging to
  // this runtime's engine. `bindings` is the opaque endowment made from
  // Host::endow and must carry the same runtime state given to this adapter.
  // Missing dependencies, wrong order, and a second install are refused. The
  // call neither drives nor waits on the runtime.
  // @ref LLP 0057.000#50-three-doors-one-implementation — door 2 installs into a caller-owned runtime and returns
  void install(Groups groups, const Ibex2Bindings* bindings,
               const CompiledScript* scripts, size_t script_count);
  jsi::Function async_binding(const char* name, uint32_t op, const void* grants);
  // Endowed values built from the factories retained by install().
  jsi::Function fetch(const void* grants);
  jsi::Object storage(const void* grants);
  // sqlite_factory is the completion value of precompiled bindings/sqlite.js.
  // This returns frozen {fs, sqlite}; it never modifies the global object.
  jsi::Object storage(const void* grants, const jsi::Function& sqlite_factory);
  // Takes/releases the ABI payload even if no promise is awaiting this id.
  void settle(uint64_t task_id, Ibex2AbiValue&, bool is_error);
  // Takes at most one storage completion. No timers or microtask checkpoints.
  // Returns true if a task was delivered; throws for a non-settlement task.
  bool deliver_one();
private:
  struct State;
  jsi::Runtime* runtime_;
  std::shared_ptr<State> state_;
};
} // namespace ibex2::jsi_adapter
