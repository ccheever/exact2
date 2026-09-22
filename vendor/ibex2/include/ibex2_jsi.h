#pragma once
// JSI-only bindings. Compile ibex2_jsi.cc with the embedding engine's JSI headers.
// @ref LLP 0068#1-the-shape — one standard library, caller-owned engine and loop
#include <jsi/jsi.h>
#include <cstddef>
#include <cstdint>
#include <memory>
#include <string>
#include <vector>

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
Ibex2AbiValue to_abi(jsi::Runtime&, const jsi::Value&, std::vector<std::string>&);
jsi::Value from_abi(jsi::Runtime&, Ibex2AbiValue&);
struct HostCallResult {
  int status;
  jsi::Value value;
};
HostCallResult call_host_result(jsi::Runtime&, const void*, uint32_t,
                                const jsi::Value*, size_t);
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
  Adapter(jsi::Runtime&, const void* borrowed_queue);
  ~Adapter();
  Adapter(const Adapter&) = delete;
  Adapter& operator=(const Adapter&) = delete;
  void detach();
  // Update only an already-captured intrinsic property's expected identity
  // after Ibex's trusted bootstrap replaces that property. Every other
  // captured identity remains anchored to runtime construction.
  void accept_trusted_intrinsic_property(jsi::Object, const char* name);
  jsi::Function async_binding(const char* name, uint32_t op, const void* grants);
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
