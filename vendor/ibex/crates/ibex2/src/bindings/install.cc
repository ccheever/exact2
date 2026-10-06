// Engine-independent JSI adapter; no Hermes ownership, loader or event loop.
// @ref LLP 0067#3-the-check — captured authority, one Rust boundary
#include "../../include/ibex2_jsi.h"
#if __has_include(<hermes/BCGen/HBC/BytecodeFileFormat.h>)
#include <hermes/BCGen/HBC/BytecodeFileFormat.h>
#define IBEX2_HAS_HERMES_BYTECODE_FILE_FORMAT 1
#endif
#include <cstring>
#include <cmath>
#include <unordered_map>
#include <unordered_set>
#include <stdexcept>

extern "C" int ibex2_host_call(const void*, uint32_t, const Ibex2AbiValue*, size_t, Ibex2AbiValue*);
extern "C" void ibex2_host_release(Ibex2AbiValue*);
extern "C" int ibex2_async_begin(const void*, const void*, uint32_t, const Ibex2AbiValue*, size_t, uint64_t);
extern "C" int ibex2_take_task(const void*, int*, unsigned long long*, Ibex2AbiValue*, int*);
extern "C" const void* ibex2_grants_retain(const void*);
extern "C" void ibex2_grants_destroy(const void*);
extern "C" const void* ibex2_bindings_state(const Ibex2Bindings*);
extern "C" const void* ibex2_bindings_grants(const Ibex2Bindings*);
extern "C" void* ibex2_sqlite_owner_create(const void*, double, int);
extern "C" void ibex2_sqlite_owner_destroy(void*);
extern "C" void* ibex2_response_owner_create(const void*, double);
extern "C" void ibex2_response_owner_destroy(void*);
extern "C" void* ibex2_crypto_key_owner_create(const void*, double);
extern "C" void ibex2_crypto_key_owner_destroy(void*);
extern "C" int ibex2_response_field(const void*, double, uint32_t,
                                    const Ibex2AbiValue*, Ibex2AbiValue*);
extern "C" size_t ibex2_grants_env_count(const void*);
extern "C" int ibex2_grants_env_at(const void*, size_t, char**, char**);
extern "C" void ibex2_string_free(char*);
extern "C" void ibex2_report_uncaught(const char*);
extern "C" void* ibex2_subscription_create(const void*, uint64_t*);
extern "C" void ibex2_subscription_destroy(void*);
extern "C" int ibex2_websocket_supported();
extern "C" uint64_t ibex2_websocket_open(const void*, const void*, uint64_t,
    const uint8_t*, size_t, const uint8_t*, size_t, size_t, char**);
extern "C" void* ibex2_websocket_owner_create(const void*, uint64_t);
extern "C" void ibex2_websocket_owner_destroy(void*);
extern "C" int ibex2_websocket_send(const void*, uint64_t, int,
    const uint8_t*, size_t, char**);
extern "C" int ibex2_websocket_close(const void*, uint64_t, int,
    const uint8_t*, size_t, char**);
extern "C" int ibex2_websocket_ready_state(const void*, uint64_t);
extern "C" size_t ibex2_websocket_buffered_amount(const void*, uint64_t);

#if defined(IBEX2_JSI_HAS_INTL)
namespace ibex2::intl_number_format {
void install(facebook::jsi::Runtime&,
             std::shared_ptr<ibex2::jsi_adapter::Lifetime>);
}
namespace ibex2::intl_case {
void install(facebook::jsi::Runtime&,
             std::shared_ptr<ibex2::jsi_adapter::Lifetime>);
}
namespace ibex2::intl_datetime {
std::vector<facebook::jsi::Value> factory_arguments(facebook::jsi::Runtime&,
    std::shared_ptr<ibex2::jsi_adapter::Lifetime>);
}
#if defined(_WIN32)
extern "C" int ibex2_intl_os_icu_available();
#endif
#endif

namespace ibex2::jsi_adapter {
const void* Lifetime::require(jsi::Runtime& rt) const {
  if (!alive_ || state_ == nullptr)
    throw jsi::JSError(rt, "Ibex2 bindings are detached");
  return state_;
}

// Convert a JS argument. Strings are decoded into `owned`, which the caller
// keeps alive for the duration of the host call so the span stays valid.
Ibex2AbiValue to_abi(jsi::Runtime &rt, const jsi::Value &value,
                     std::vector<std::string> &owned) {
  Ibex2AbiValue out{IBEX2_TAG_UNDEFINED, 0.0, nullptr, 0};
  if (value.isUndefined()) {
    return out;
  }
  if (value.isNull()) {
    out.tag = IBEX2_TAG_NULL;
    return out;
  }
  if (value.isBool()) {
    out.tag = IBEX2_TAG_BOOL;
    out.number = value.getBool() ? 1.0 : 0.0;
    return out;
  }
  if (value.isNumber()) {
    out.tag = IBEX2_TAG_NUMBER;
    out.number = value.getNumber();
    return out;
  }
  if (value.isObject() && value.getObject(rt).isArrayBuffer(rt)) {
    auto buffer = value.getObject(rt).getArrayBuffer(rt);
    out.tag = IBEX2_TAG_BYTES;
    out.data = buffer.data(rt);
    out.len = buffer.size(rt);
    return out;
  }
  // A TYPED ARRAY, which is what application code actually passes: a Uint8Array
  // is not an ArrayBuffer, and handling only the latter made
  // `fs.writeFile(path, new TextEncoder().encode(text))` stringify its payload
  // and write an empty file. The view's offset matters — a subarray shares its
  // buffer with the whole, so reading from the buffer's start would send the
  // wrong bytes.
  if (value.isObject() && value.getObject(rt).isTypedArray(rt)) {
    auto view = value.getObject(rt).getTypedArray(rt);
    auto buffer = view.buffer(rt);
    out.tag = IBEX2_TAG_BYTES;
    out.data = buffer.data(rt) + view.byteOffset(rt);
    out.len = view.byteLength(rt);
    return out;
  }
  // Everything else stringifies, which is what console does with its arguments.
  owned.push_back(value.toString(rt).utf8(rt));
  const std::string &text = owned.back();
  out.tag = IBEX2_TAG_STRING;
  out.data = reinterpret_cast<const unsigned char *>(text.data());
  out.len = text.size();
  return out;
}

// A byte result IS the JavaScript ArrayBuffer's storage — no copy, no second
// allocation. Rust allocated it, ownership transfers here, and the engine frees
// it back through the boundary when the ArrayBuffer is collected. That is the
// outbound half of LLP 0059.000 §1.2.
//
// The destructor is the whole mechanism: Hermes holds this shared_ptr for as
// long as the ArrayBuffer is reachable, so the Rust allocation outlives every
// JavaScript reference to it and is released exactly once.
class RustBytes : public jsi::MutableBuffer {
public:
  explicit RustBytes(Ibex2AbiValue value) : value_(value) {}
  ~RustBytes() override { ibex2_host_release(&value_); }

  RustBytes(const RustBytes &) = delete;
  RustBytes &operator=(const RustBytes &) = delete;

  size_t size() const override { return value_.len; }
  uint8_t *data() override {
    return const_cast<uint8_t *>(value_.data);
  }

private:
  Ibex2AbiValue value_;
};

// Engines may retain bytecode storage for as long as evaluated functions are
// live, so installation copies the caller's span into an owned JSI buffer.
class CompiledBytes : public jsi::Buffer {
public:
  CompiledBytes(const uint8_t* data, size_t len) : bytes_(data, data + len) {}
  size_t size() const override { return bytes_.size(); }
  const uint8_t* data() const override { return bytes_.data(); }
private:
  std::vector<uint8_t> bytes_;
};

// Convert a result. For bytes this TAKES OWNERSHIP and clears `value`, so the
// caller's release becomes a no-op — the RustBytes destructor releases instead,
// when the engine is done with the buffer. Strings still copy: Hermes owns its
// own string representation and there is no way to hand it one (§1.2).
jsi::Value from_abi(jsi::Runtime &rt, Ibex2AbiValue &value) {
  switch (value.tag) {
  case IBEX2_TAG_NULL:
    return jsi::Value::null();
  case IBEX2_TAG_BOOL:
    return jsi::Value(value.number != 0.0);
  case IBEX2_TAG_NUMBER:
    return jsi::Value(value.number);
  case IBEX2_TAG_STRING: {
    std::string text(reinterpret_cast<const char *>(value.data), value.len);
    return jsi::String::createFromUtf8(rt, text);
  }
  case IBEX2_TAG_BYTES: {
    Ibex2AbiValue owned = value;
    value = Ibex2AbiValue{IBEX2_TAG_UNDEFINED, 0.0, nullptr, 0};
    return jsi::Value(rt,
                      jsi::ArrayBuffer(rt, std::make_shared<RustBytes>(owned)));
  }
  default:
    return jsi::Value::undefined();
  }
}

// The result-bearing half is also used by bindings that need to translate a
// Rust-owned error kind into the matching JavaScript error constructor.
HostCallResult call_host_result(jsi::Runtime& rt, const void* state,
                                uint32_t op, const jsi::Value* args,
                                size_t count) {
  std::vector<std::string> owned;
  owned.reserve(count);
  std::vector<Ibex2AbiValue> abi;
  abi.reserve(count);
  for (size_t i = 0; i < count; ++i) {
    abi.push_back(to_abi(rt, args[i], owned));
  }
  Ibex2AbiValue out{IBEX2_TAG_UNDEFINED, 0.0, nullptr, 0};
  int status = ibex2_host_call(state, op,
                               abi.empty() ? nullptr : abi.data(),
                               abi.size(), &out);
  struct Release { Ibex2AbiValue& value; ~Release() { ibex2_host_release(&value); } } release{out};
  jsi::Value result = from_abi(rt, out);
  return HostCallResult{status, std::move(result)};
}

// All ordinary synchronous bindings share the same conversion and release
// path. Their public contracts report a generic host-call failure.
static jsi::Value call_host(jsi::Runtime& rt, const void* state, uint32_t op,
                           const jsi::Value* args, size_t count) {
  auto result = call_host_result(rt, state, op, args, count);
  if (result.status != 0) {
    // The Rust error taxonomy becomes a JS throw here, so failures are
    // identical on every platform (LLP 0057 §3).
    throw jsi::JSError(rt, result.value.isString()
                               ? result.value.getString(rt).utf8(rt)
                               : std::string("host call failed"));
  }
  return std::move(result.value);
}

// One host function per op, so JavaScript sees ordinary callables while every
// one of them funnels through the single ibex2_host_call surface.
jsi::Function make_host_binding(jsi::Runtime &runtime, const char *name,
                                uint32_t op, const void *state) {
  auto prop = jsi::PropNameID::forUtf8(runtime, std::string(name));
  return jsi::Function::createFromHostFunction(
      runtime, prop, 1,
      [op, state](jsi::Runtime &rt, const jsi::Value &, const jsi::Value *args,
                  size_t count) -> jsi::Value {
        return call_host(rt, state, op, args, count);
      });
}

void set_binding(jsi::Runtime &rt, jsi::Object &target, const char *name,
                 uint32_t op, const void *state) {
  target.setProperty(rt, jsi::PropNameID::forUtf8(rt, std::string(name)),
                     make_host_binding(rt, name, op, state));
}

// Capture before application code. A later freeze alone is insufficient: an
// application could replace WeakMap.prototype.set first, then freeze it. Pin
// descriptors as well as identities so that cannot expose private SQL handles.
struct Integrity {
  struct Property {
    jsi::Value key, value, get, set;
  };
  struct Intrinsic {
    jsi::Object object;
    jsi::Value prototype;
    std::vector<Property> properties;
  };
  std::vector<std::pair<std::string, jsi::Value>> globals;
  std::vector<Intrinsic> intrinsics;
  jsi::Function descriptor, names, symbols, prototype, frozen, same;
  bool validated = false;

  explicit Integrity(jsi::Runtime& rt)
      : descriptor(rt.global().getPropertyAsObject(rt, "Object").getPropertyAsFunction(rt, "getOwnPropertyDescriptor")),
        names(rt.global().getPropertyAsObject(rt, "Object").getPropertyAsFunction(rt, "getOwnPropertyNames")),
        symbols(rt.global().getPropertyAsObject(rt, "Object").getPropertyAsFunction(rt, "getOwnPropertySymbols")),
        prototype(rt.global().getPropertyAsObject(rt, "Object").getPropertyAsFunction(rt, "getPrototypeOf")),
        frozen(rt.global().getPropertyAsObject(rt, "Object").getPropertyAsFunction(rt, "isFrozen")),
        same(rt.global().getPropertyAsObject(rt, "Object").getPropertyAsFunction(rt, "is")) {
    for (const char* name : {"Object", "Function", "Array", "Promise", "WeakMap",
          "Reflect", "Number", "BigInt", "Uint8Array", "ArrayBuffer", "Error",
          "TypeError", "RangeError", "String", "JSON", "Symbol", "Map", "Set"}) {
      auto value = rt.global().getProperty(rt, name);
      if (!value.isObject()) throw jsi::JSError(rt, "Ibex2 SQLite requires standard intrinsics");
      globals.emplace_back(name, jsi::Value(rt, value));
      capture(rt, value.getObject(rt));
      auto d = descriptor.call(rt, value, "prototype");
      if (d.isObject()) {
        auto p = d.getObject(rt).getProperty(rt, "value");
        if (p.isObject()) capture(rt, p.getObject(rt));
      }
    }
  }

  std::vector<jsi::Value> keys(jsi::Runtime& rt, const jsi::Object& object) {
    std::vector<jsi::Value> result;
    for (const auto* function : {&names, &symbols}) {
      auto array = function->call(rt, object).getObject(rt).getArray(rt);
      for (size_t i = 0; i < array.size(rt); ++i) result.push_back(array.getValueAtIndex(rt, i));
    }
    return result;
  }

  void capture(jsi::Runtime& rt, jsi::Object object) {
    for (const auto& item : intrinsics)
      if (jsi::Object::strictEquals(rt, object, item.object)) return;
    auto parent = prototype.call(rt, object);
    Intrinsic item{std::move(object), jsi::Value(rt, parent), {}};
    for (auto& key : keys(rt, item.object)) {
      auto d = descriptor.call(rt, item.object, key).getObject(rt);
      item.properties.push_back(Property{std::move(key), d.getProperty(rt, "value"),
          d.getProperty(rt, "get"), d.getProperty(rt, "set")});
    }
    intrinsics.push_back(std::move(item));
    if (parent.isObject()) capture(rt, parent.getObject(rt));
  }

  bool equal(jsi::Runtime& rt, const jsi::Value& a, const jsi::Value& b) {
    return same.call(rt, a, b).getBool();
  }

  void require(jsi::Runtime& rt) {
    if (validated) return;
    auto refuse = [&]() {
      throw jsi::JSError(rt, "Ibex2 SQLite requires unchanged, hardened intrinsics: create bindings before application code and run HARDEN_SOURCE before using them");
    };
    for (const auto& [name, value] : globals) {
      auto d = descriptor.call(rt, rt.global(), jsi::String::createFromUtf8(rt, name));
      if (!d.isObject()) refuse();
      auto property = d.getObject(rt);
      if (property.getProperty(rt, "writable").isUndefined()
          || property.getProperty(rt, "writable").getBool()
          || property.getProperty(rt, "configurable").getBool()
          || !equal(rt, property.getProperty(rt, "value"), value)) refuse();
    }
    for (const auto& item : intrinsics) {
      if (!frozen.call(rt, item.object).getBool()
          || !equal(rt, prototype.call(rt, item.object), item.prototype)
          || keys(rt, item.object).size() != item.properties.size()) refuse();
      for (const auto& property : item.properties) {
        auto raw = descriptor.call(rt, item.object, property.key);
        if (!raw.isObject()) refuse();
        auto d = raw.getObject(rt);
        for (const auto& pair : {std::pair<const char*, const jsi::Value*>{"value", &property.value},
                                 {"get", &property.get}, {"set", &property.set}}) {
          if (!equal(rt, d.getProperty(rt, pair.first), *pair.second)) refuse();
          if (pair.second->isObject() && !frozen.call(rt, *pair.second).getBool()) refuse();
        }
      }
    }
    validated = true;
  }

  void accept_property(jsi::Runtime& rt, const jsi::Object& object,
                       const char* name) {
    if (validated)
      throw jsi::JSError(rt, "cannot replace an intrinsic after validation");
    auto key = jsi::Value(rt, jsi::String::createFromUtf8(rt, name));
    for (auto& item : intrinsics) {
      if (!jsi::Object::strictEquals(rt, object, item.object)) continue;
      for (auto& property : item.properties) {
        if (!equal(rt, property.key, key)) continue;
        auto raw = descriptor.call(rt, item.object, key);
        if (!raw.isObject())
          throw jsi::JSError(rt, "trusted intrinsic replacement is absent");
        auto current = raw.getObject(rt);
        property.value = current.getProperty(rt, "value");
        property.get = current.getProperty(rt, "get");
        property.set = current.getProperty(rt, "set");
        return;
      }
      throw jsi::JSError(rt, "trusted intrinsic property was not captured");
    }
    throw jsi::JSError(rt, "trusted intrinsic object was not captured");
  }
};

// The reflective intrinsics the trusted-bootstrap reachability walk uses,
// captured when install_with publishes an object -- before the embedder's
// bootstrap runs between install and harden -- so that bootstrap cannot change
// what the walk sees by replacing Object.getOwnPropertyNames or Set.prototype.
struct Reachability {
  jsi::Function names, symbols, descriptor, prototype, set, has, add;
  explicit Reachability(jsi::Runtime& rt)
      : names(object_function(rt, "getOwnPropertyNames")),
        symbols(object_function(rt, "getOwnPropertySymbols")),
        descriptor(object_function(rt, "getOwnPropertyDescriptor")),
        prototype(object_function(rt, "getPrototypeOf")),
        set(rt.global().getPropertyAsFunction(rt, "Set")),
        has(set_method(rt, "has")),
        add(set_method(rt, "add")) {}
  static jsi::Function object_function(jsi::Runtime& rt, const char* name) {
    return rt.global().getPropertyAsObject(rt, "Object").getPropertyAsFunction(rt, name);
  }
  static jsi::Function set_method(jsi::Runtime& rt, const char* name) {
    return rt.global().getPropertyAsObject(rt, "Set")
        .getPropertyAsObject(rt, "prototype").getPropertyAsFunction(rt, name);
  }
};

enum class InstallStatus { Fresh, Installed, Spent };

struct WebSocketOwner final : jsi::NativeState {
  void* connection;
  void* subscription;
  WebSocketOwner(void* connection_value, void* subscription_value)
      : connection(connection_value), subscription(subscription_value) {}
  ~WebSocketOwner() override {
    unsubscribe_events();
    if (connection != nullptr) {
      ibex2_websocket_owner_destroy(connection);
      connection = nullptr;
    }
  }
  void unsubscribe_events() {
    if (subscription != nullptr) {
      ibex2_subscription_destroy(subscription);
      subscription = nullptr;
    }
  }
};

struct Adapter::State {
  struct BootstrapOutput {
    std::string kind;
    std::string name;
    std::vector<std::pair<std::string, jsi::Value>> identities;
  };
  struct Pending { jsi::Function resolve; jsi::Function reject; };
  struct EventSubscription {
    void* rust;
    jsi::Function callback;
    uint64_t websocket = 0;
    std::shared_ptr<jsi::WeakObject> weak_owner;
    jsi::Value strong_owner;
    bool listener_keepalive = false;
    std::weak_ptr<WebSocketOwner> native_owner;
  };
  const void* queue;
  uint64_t next_task_id = 1;
  std::unordered_map<uint64_t, Pending> pending;
  std::unordered_map<uint64_t, EventSubscription> subscriptions;
  bool alive = true;
  InstallStatus install_status = InstallStatus::Fresh;
  Groups groups = 0;
  uint32_t bytecode_version;
  std::shared_ptr<Lifetime> lifetime;
  jsi::Value fetch_factory;
  jsi::Value websocket_factory;
  jsi::Value blob_helpers;
  jsi::Value sqlite_factory;
  jsi::Value event_reporter;
  jsi::Value trusted_event_dispatch;
  jsi::Value event_listener_query;
  jsi::Value event_listener_change_hook;
  jsi::Value rejection_unhandled;
  jsi::Value rejection_handled;
  // `Object.isFrozen` as it was at construction; a jsi::Value so detach() can
  // release it with the other roots.
  jsi::Value intrinsic_frozen;
  // %Object.prototype%, %Function.prototype%, and %Array.prototype% as they were
  // when the adapter was constructed, before any bootstrap ran. A bootstrap can
  // replace the globals that name them, but not these identities, so capture
  // can tell whether harden.js has already run.
  jsi::Value freeze_witnesses;
  std::unique_ptr<Integrity> integrity;
  bool intrinsic_snapshot_deferred = false;
  bool hardened = false;
  // Trusted-bootstrap globals and the identities published under them (the
  // object first, then each member), kept so the harden guard can prove that
  // none of them is still reachable from what harden.js freezes.
  std::vector<BootstrapOutput> bootstrap_outputs;
  std::unique_ptr<Reachability> reachability;
  State(jsi::Runtime& rt, const void* value, uint32_t version,
        std::shared_ptr<Lifetime> lifetime_value)
      : queue(value), bytecode_version(version),
        lifetime(std::move(lifetime_value)),
        intrinsic_frozen(rt.global().getPropertyAsObject(rt, "Object")
                             .getPropertyAsFunction(rt, "isFrozen")),
        freeze_witnesses(jsi::Array::createWithElements(
            rt,
            rt.global().getPropertyAsObject(rt, "Object").getProperty(rt, "prototype"),
            rt.global().getPropertyAsObject(rt, "Function").getProperty(rt, "prototype"),
            rt.global().getPropertyAsObject(rt, "Array").getProperty(rt, "prototype"))),
        integrity(std::make_unique<Integrity>(rt)) {}
  const void* require(jsi::Runtime& rt) const {
    return lifetime->require(rt);
  }
};

Adapter::Adapter(jsi::Runtime& rt, const void* queue,
                 uint32_t bytecode_version)
    : runtime_(&rt),
      state_(std::make_shared<State>(
          rt, queue, bytecode_version,
          std::shared_ptr<Lifetime>(new Lifetime(queue)))) {
  if (!queue) throw std::invalid_argument("Ibex2 bindings require runtime state");
}
Adapter::~Adapter() { detach(); }
std::shared_ptr<Lifetime> Adapter::lifetime() const {
  return state_->lifetime;
}
void Adapter::accept_trusted_intrinsic_property(jsi::Object object,
                                                const char* name) {
  if (!runtime_)
    throw std::logic_error("Ibex2 bindings are detached");
  if (!state_->integrity)
    throw std::logic_error(
        "the intrinsic snapshot is deferred; capture the complete baseline instead");
  state_->integrity->accept_property(*runtime_, object, name);
}
void Adapter::capture_intrinsics() {
  if (!runtime_) throw std::logic_error("Ibex2 bindings are detached");
  if (state_->install_status != InstallStatus::Installed)
    throw std::logic_error(
        "a deferred intrinsic snapshot can be captured only after installation");
  if (!state_->intrinsic_snapshot_deferred)
    throw std::logic_error("the intrinsic snapshot was not deferred");
  if (state_->integrity)
    throw std::logic_error("the deferred intrinsic snapshot was already captured");
  // Never consult the current globals here: a bootstrap could replace `Array`
  // with a function whose `prototype` accessor returns a fresh, unfrozen object.
  // The witnesses are the intrinsics captured at construction.
  if (state_->hardened)
    throw std::logic_error(
        "the deferred intrinsic snapshot cannot be captured after intrinsics are frozen");
  auto is_frozen = state_->intrinsic_frozen.asObject(*runtime_).asFunction(*runtime_);
  auto witnesses = state_->freeze_witnesses.asObject(*runtime_).asArray(*runtime_);
  for (size_t i = 0; i < witnesses.size(*runtime_); ++i) {
    if (is_frozen.call(*runtime_, witnesses.getValueAtIndex(*runtime_, i))
            .getBool())
      throw std::logic_error(
          "the deferred intrinsic snapshot cannot be captured after intrinsics are frozen");
  }
  state_->integrity = std::make_unique<Integrity>(*runtime_);
}
void Adapter::detach() {
  if (!state_->alive) return;
  state_->alive = false;
  for (auto& entry : state_->subscriptions) {
    if (auto owner = entry.second.native_owner.lock())
      owner->unsubscribe_events();
    else
      ibex2_subscription_destroy(entry.second.rust);
  }
  state_->subscriptions.clear();
  state_->lifetime->detach();
  state_->pending.clear();
  state_->fetch_factory = jsi::Value::undefined();
  state_->websocket_factory = jsi::Value::undefined();
  state_->blob_helpers = jsi::Value::undefined();
  state_->sqlite_factory = jsi::Value::undefined();
  state_->event_reporter = jsi::Value::undefined();
  state_->trusted_event_dispatch = jsi::Value::undefined();
  state_->event_listener_query = jsi::Value::undefined();
  state_->event_listener_change_hook = jsi::Value::undefined();
  state_->rejection_unhandled = jsi::Value::undefined();
  state_->rejection_handled = jsi::Value::undefined();
  state_->intrinsic_frozen = jsi::Value::undefined();
  state_->freeze_witnesses = jsi::Value::undefined();
  state_->integrity.reset();
  state_->bootstrap_outputs.clear();
  state_->reachability.reset();
  state_->queue = nullptr;
  runtime_ = nullptr;
}

namespace {
void freeze(jsi::Runtime&, const jsi::Object&);

constexpr Groups kKnownGroups = GROUP_PURE | GROUP_CONSOLE | GROUP_TIMERS |
    GROUP_ABORT | GROUP_CRYPTO | GROUP_FETCH | GROUP_STORAGE | GROUP_ENV |
    GROUP_SECRETS | GROUP_KV | GROUP_INTL | GROUP_EVENTS | GROUP_BLOB |
    GROUP_WEBSOCKET;

bool has(Groups groups, Groups group) { return (groups & group) == group; }

void validate_groups_impl(Groups groups) {
  if ((groups & ~kKnownGroups) != 0)
    throw std::invalid_argument("unknown Ibex2 binding group bit");
  struct Requirement { Groups group; Groups required; };
  constexpr Requirement requirements[] = {
      {GROUP_TIMERS, GROUP_CONSOLE},
      {GROUP_ABORT, GROUP_PURE},
      {GROUP_CRYPTO, GROUP_PURE},
      {GROUP_FETCH, GROUP_PURE | GROUP_ABORT},
      {GROUP_EVENTS, GROUP_PURE},
      {GROUP_WEBSOCKET, GROUP_PURE | GROUP_EVENTS},
      {GROUP_BLOB, GROUP_PURE},
  };
  for (const auto& requirement : requirements) {
    if (has(groups, requirement.group) && !has(groups, requirement.required))
      throw std::invalid_argument("Ibex2 binding group is missing a dependency");
  }
#if !defined(IBEX2_JSI_HAS_INTL)
  if (has(groups, GROUP_INTL))
    throw std::invalid_argument("Ibex2 INTL bindings are unavailable in this build");
#elif defined(_WIN32)
  // @ref LLP 0057.000#511-windows-intl-uses-the-os-icu — the same probe as
  // Groups::validate: the OS must export the ICU the shims call.
  if (has(groups, GROUP_INTL) && ibex2_intl_os_icu_available() == 0)
    throw std::invalid_argument(
        "Ibex2 INTL bindings are unavailable: this Windows lacks the OS ICU "
        "they use (icu.dll from Windows 10 version 2004 or later)");
#endif
}

std::vector<const char*> expected_scripts_impl(Groups groups) {
  std::vector<const char*> result;
  if (has(groups, GROUP_PURE)) result.push_back("headers");
  if (has(groups, GROUP_TIMERS)) result.push_back("timers");
  if (has(groups, GROUP_PURE)) {
    result.push_back("url");
    result.push_back("domexception");
  }
  if (has(groups, GROUP_CRYPTO)) result.push_back("crypto");
  if (has(groups, GROUP_EVENTS)) result.push_back("events");
  if (has(groups, GROUP_ABORT)) result.push_back("abort");
  if (has(groups, GROUP_BLOB)) result.push_back("blob");
  if (has(groups, GROUP_WEBSOCKET)) result.push_back("websocket");
#if defined(IBEX2_JSI_HAS_INTL)
  if (has(groups, GROUP_INTL)) {
    result.push_back("intl_number_format");
    result.push_back("intl_case");
    result.push_back("intl_datetime");
  }
#endif
  if (has(groups, GROUP_FETCH)) result.push_back("fetch");
  if (has(groups, GROUP_STORAGE)) result.push_back("sqlite");
  if (has(groups, GROUP_PURE)) result.push_back("structured_clone");
  return result;
}

constexpr uint8_t kHermesBytecodeMagic[] = {
    0xc6, 0x1f, 0xbc, 0x03, 0xc1, 0x03, 0x19, 0x1f};

// The installed Hermes bundle exposes only public headers, not the internal
// file-format header. Full source builds use sizeof directly; the fallback is
// the selected pin's packed, cache-aligned BytecodeFileHeader size. Keep the
// assertion so a source-header build makes a pin change fail loudly.
#if defined(IBEX2_HAS_HERMES_BYTECODE_FILE_FORMAT)
constexpr size_t kHermesBytecodeHeaderSize =
    sizeof(::hermes::hbc::BytecodeFileHeader);
static_assert(kHermesBytecodeHeaderSize == 128,
              "update the installed-header BytecodeFileHeader size");
#else
constexpr size_t kHermesBytecodeHeaderSize = 128;
#endif

uint32_t bytecode_version(const CompiledScript& script) {
  return static_cast<uint32_t>(script.bytes[8]) |
      (static_cast<uint32_t>(script.bytes[9]) << 8) |
      (static_cast<uint32_t>(script.bytes[10]) << 16) |
      (static_cast<uint32_t>(script.bytes[11]) << 24);
}

uint32_t bytecode_declared_length(const CompiledScript& script) {
  return static_cast<uint32_t>(script.bytes[32]) |
      (static_cast<uint32_t>(script.bytes[33]) << 8) |
      (static_cast<uint32_t>(script.bytes[34]) << 16) |
      (static_cast<uint32_t>(script.bytes[35]) << 24);
}

// Header checks shared by install() and harden(): only precompiled Hermes
// bytecode of this runtime's version is ever evaluated by the adapter.
void validate_bytecode(const CompiledScript& script, uint32_t expected_version) {
  if (script.len < kHermesBytecodeHeaderSize)
    throw std::invalid_argument(
        "Ibex2 binding payload has a truncated Hermes bytecode header");
  if (std::memcmp(script.bytes, kHermesBytecodeMagic,
                  sizeof(kHermesBytecodeMagic)) != 0)
    throw std::invalid_argument("Ibex2 binding payload is not Hermes bytecode");
  if (bytecode_declared_length(script) != script.len)
    throw std::invalid_argument(
        "Ibex2 binding bytecode declared length does not match its buffer");
  if (expected_version != 0 && bytecode_version(script) != expected_version)
    throw std::invalid_argument(
        "Ibex2 binding bytecode version does not match the runtime");
}

struct ResponseOwner final : jsi::NativeState {
  void* owner;
  explicit ResponseOwner(void* value) : owner(value) {}
  ~ResponseOwner() override { ibex2_response_owner_destroy(owner); }
};

struct CryptoKeyOwner final : jsi::NativeState {
  void* owner;
  explicit CryptoKeyOwner(void* value) : owner(value) {}
  ~CryptoKeyOwner() override { ibex2_crypto_key_owner_destroy(owner); }
};

// Each opt-in fetch-primitives object is its own handle domain. The Rust
// registries remain runtime-wide, so these sets are the native, unforgeable
// proof that a handle crossed this exact bootstrap object. Handles are never
// recycled by RuntimeState, and terminal/released entries are removed here.
struct FetchPrimitiveHandles {
  std::unordered_set<uint64_t> responses;
  std::unordered_set<uint64_t> controls;
  std::unordered_set<uint64_t> headers;
};

[[noreturn]] void throw_primitive_error(jsi::Runtime& rt, const char* constructor,
                                        const std::string& message) {
  auto error = rt.global().getPropertyAsFunction(rt, constructor)
      .callAsConstructor(rt, message);
  throw jsi::JSError(rt, std::move(error));
}

[[noreturn]] void throw_primitive_type_error(jsi::Runtime& rt,
                                             const std::string& message) {
  throw_primitive_error(rt, "TypeError", message);
}

[[noreturn]] void throw_primitive_range_error(jsi::Runtime& rt,
                                              const std::string& message) {
  throw_primitive_error(rt, "RangeError", message);
}

// Every numeric argument crosses through here before any conversion: a
// non-number is a TypeError; NaN, an infinity, a fraction, or a value outside
// [min, max] is a RangeError. Only then is the double converted, so no
// out-of-range floating-to-integer conversion (undefined behavior) can happen.
uint64_t primitive_integer(jsi::Runtime& rt, const jsi::Value& value,
                           double min, double max, const char* what) {
  if (!value.isNumber())
    throw_primitive_type_error(rt, std::string(what) + " must be a number");
  const double number = value.getNumber();
  if (!std::isfinite(number) || std::floor(number) != number ||
      number < min || number > max)
    throw_primitive_range_error(
        rt, std::string(what) + " must be an integer from " +
                std::to_string(static_cast<uint64_t>(min)) + " to " +
                std::to_string(static_cast<uint64_t>(max)));
  return static_cast<uint64_t>(number);
}

constexpr double kMaxSafeInteger = 9007199254740991.0;

// The only accepted spelling of a trusted-bootstrap global. Restricting it to
// an ASCII identifier makes the Rust string, this C++ string, and the
// JavaScript property key the same bytes, so publication, collision checks,
// and the harden guard can never disagree about which property they mean.
bool is_ascii_identifier(const std::string& name) {
  auto start = [](char c) {
    return (c >= 'A' && c <= 'Z') || (c >= 'a' && c <= 'z') || c == '_' ||
        c == '$';
  };
  if (name.empty() || !start(name[0])) return false;
  for (char c : name)
    if (!start(c) && !(c >= '0' && c <= '9')) return false;
  return true;
}

// Every lookup of a chosen name goes through this one key construction.
jsi::PropNameID bootstrap_output_key(jsi::Runtime& rt, const std::string& name) {
  return jsi::PropNameID::forAscii(rt, name);
}

// A handle is an integer from 1 to 2^53 - 1 (Number.MAX_SAFE_INTEGER).
uint64_t primitive_handle(jsi::Runtime& rt, const jsi::Value& value,
                          const char* what) {
  return primitive_integer(rt, value, 1.0, kMaxSafeInteger, what);
}

// A well-formed handle that this primitives object did not create, or has
// already seen released, is a TypeError.
void require_owned(jsi::Runtime& rt, uint64_t handle,
                   const std::unordered_set<uint64_t>& owned, const char* what) {
  if (owned.find(handle) == owned.end())
    throw_primitive_type_error(
        rt, std::string(what) +
                " was not created by these fetch primitives or was already released");
}

uint64_t require_primitive_handle(
    jsi::Runtime& rt, const jsi::Value& value,
    const std::unordered_set<uint64_t>& owned, const char* what) {
  const uint64_t handle = primitive_handle(rt, value, what);
  require_owned(rt, handle, owned, what);
  return handle;
}

// The response fields the ABI defines (ibex2_response_field); 8 cancels.
bool known_response_field(uint64_t field) {
  switch (field) {
    case 0: case 1: case 2: case 3: case 5: case 7: case 8: return true;
    default: return false;
  }
}

jsi::Function make_group_binding(jsi::Runtime& rt, const char* name,
                                 uint32_t op,
                                 std::shared_ptr<Lifetime> lifetime) {
  auto prop = jsi::PropNameID::forUtf8(rt, std::string(name));
  return jsi::Function::createFromHostFunction(
      rt, prop, 1,
      [op, lifetime = std::move(lifetime)](
          jsi::Runtime& r, const jsi::Value&, const jsi::Value* args,
          size_t count) -> jsi::Value {
        const void* state = lifetime->require(r);
        return call_host(r, state, op, args, count);
      });
}

void set_group_binding(jsi::Runtime& rt, jsi::Object& target,
                       const char* name, uint32_t op,
                       const std::shared_ptr<Lifetime>& lifetime) {
  target.setProperty(rt, jsi::PropNameID::forUtf8(rt, std::string(name)),
                     make_group_binding(rt, name, op, lifetime));
}

void install_console(jsi::Runtime& rt,
                     const std::shared_ptr<Lifetime>& lifetime) {
  jsi::Object console(rt);
  set_group_binding(rt, console, "log", 1, lifetime);
  set_group_binding(rt, console, "info", 2, lifetime);
  set_group_binding(rt, console, "debug", 3, lifetime);
  set_group_binding(rt, console, "warn", 4, lifetime);
  set_group_binding(rt, console, "error", 5, lifetime);
  rt.global().setProperty(rt, "console", std::move(console));
}

void install_events(jsi::Runtime& rt,
                    const std::shared_ptr<Lifetime>& lifetime) {
  auto report = jsi::Function::createFromHostFunction(
      rt, jsi::PropNameID::forAscii(rt, "__ibex2_report_error"), 1,
      [lifetime](jsi::Runtime& r, const jsi::Value&,
                 const jsi::Value* args, size_t count) -> jsi::Value {
        lifetime->require(r);
        std::string message = count == 0 ? "uncaught error"
            : args[0].toString(r).utf8(r);
        ibex2_report_uncaught(message.c_str());
        return jsi::Value::undefined();
      });
  rt.global().setProperty(rt, "__ibex2_report_error", std::move(report));
}

void install_pure(jsi::Runtime& rt,
                  const std::shared_ptr<Lifetime>& lifetime) {
  auto global = rt.global();
  set_group_binding(rt, global, "__ibex2_text_encode", 20, lifetime);
  set_group_binding(rt, global, "__ibex2_text_decode", 21, lifetime);
  set_group_binding(rt, global, "__ibex2_text_encode_into", 22, lifetime);
  set_group_binding(rt, global, "__ibex2_url_parse", 30, lifetime);
  set_group_binding(rt, global, "__ibex2_url_set", 32, lifetime);
  set_group_binding(rt, global, "__ibex2_search_params_normalize", 29, lifetime);
  set_group_binding(rt, global, "__ibex2_search_params_get", 31, lifetime);
  set_group_binding(rt, global, "__ibex2_search_params_get_all", 33, lifetime);
  set_group_binding(rt, global, "__ibex2_search_params_has", 34, lifetime);
  set_group_binding(rt, global, "__ibex2_search_params_set", 35, lifetime);
  set_group_binding(rt, global, "__ibex2_search_params_append", 36, lifetime);
  set_group_binding(rt, global, "__ibex2_search_params_delete", 37, lifetime);
  set_group_binding(rt, global, "__ibex2_search_params_sort", 38, lifetime);
  set_group_binding(rt, global, "__ibex2_search_params_entries", 39, lifetime);

  jsi::Object headers(rt);
  set_group_binding(rt, headers, "create", 40, lifetime);
  set_group_binding(rt, headers, "append", 41, lifetime);
  set_group_binding(rt, headers, "set", 42, lifetime);
  set_group_binding(rt, headers, "get", 43, lifetime);
  set_group_binding(rt, headers, "has", 44, lifetime);
  set_group_binding(rt, headers, "remove", 45, lifetime);
  set_group_binding(rt, headers, "count", 46, lifetime);
  set_group_binding(rt, headers, "nameAt", 47, lifetime);
  set_group_binding(rt, headers, "valueAt", 48, lifetime);
  set_group_binding(rt, headers, "validName", 49, lifetime);
  set_group_binding(rt, headers, "validValue", 50, lifetime);
  set_group_binding(rt, headers, "free", 51, lifetime);
  global.setProperty(rt, "__ibex2_headers", std::move(headers));
}

void install_timers(jsi::Runtime& rt,
                    const std::shared_ptr<Lifetime>& lifetime) {
  auto global = rt.global();
  set_group_binding(rt, global, "__ibex2_timer_set", 60, lifetime);
  set_group_binding(rt, global, "__ibex2_timer_set_repeating", 61, lifetime);
  set_group_binding(rt, global, "__ibex2_timer_clear", 62, lifetime);
  set_group_binding(rt, global, "__ibex2_performance_now", 63, lifetime);
}

void install_crypto(jsi::Runtime& rt,
                    const std::shared_ptr<Lifetime>& lifetime) {
  auto global = rt.global();
  set_group_binding(rt, global, "__ibex2_random_uuid", 70, lifetime);
  set_group_binding(rt, global, "__ibex2_get_random_values", 71, lifetime);
  jsi::Object subtle(rt);
  set_group_binding(rt, subtle, "digest", 160, lifetime);
  set_group_binding(rt, subtle, "importKey", 161, lifetime);
  set_group_binding(rt, subtle, "exportKey", 162, lifetime);
  set_group_binding(rt, subtle, "generateKey", 163, lifetime);
  set_group_binding(rt, subtle, "sign", 164, lifetime);
  set_group_binding(rt, subtle, "verify", 165, lifetime);
  set_group_binding(rt, subtle, "encrypt", 166, lifetime);
  set_group_binding(rt, subtle, "decrypt", 167, lifetime);
  set_group_binding(rt, subtle, "deriveBits", 168, lifetime);
  set_group_binding(rt, subtle, "deriveKey", 169, lifetime);
  subtle.setProperty(
      rt, "own",
      jsi::Function::createFromHostFunction(
          rt, jsi::PropNameID::forAscii(rt, "ownCryptoKey"), 2,
          [lifetime](jsi::Runtime& r, const jsi::Value&,
                     const jsi::Value* args, size_t count) -> jsi::Value {
            const void* state = lifetime->require(r);
            if (count != 2 || !args[0].isNumber() || !args[1].isObject())
              throw jsi::JSError(r, "CryptoKey owner needs a handle and object");
            void* owner = ibex2_crypto_key_owner_create(state, args[0].asNumber());
            if (owner == nullptr)
              throw jsi::JSError(r, "CryptoKey handle is released or unknown");
            args[1].getObject(r).setNativeState(
                r, std::make_shared<CryptoKeyOwner>(owner));
            return jsi::Value::undefined();
          }));
  global.setProperty(rt, "__ibex2_subtle", std::move(subtle));
}

void install_blob(jsi::Runtime& rt,
                  const std::shared_ptr<Lifetime>& lifetime) {
  auto global = rt.global();
  set_group_binding(rt, global, "__ibex2_multipart_boundary", 73, lifetime);
  set_group_binding(rt, global, "__ibex2_multipart_encode", 74, lifetime);
}

jsi::Function make_response_field(
    jsi::Runtime& rt, const std::shared_ptr<Lifetime>& lifetime,
    std::shared_ptr<FetchPrimitiveHandles> primitive_handles = nullptr) {
  return jsi::Function::createFromHostFunction(
      rt, jsi::PropNameID::forAscii(rt, "__ibex2_response_field"), 3,
      [lifetime, primitive_handles = std::move(primitive_handles)](
          jsi::Runtime& r, const jsi::Value&, const jsi::Value* args,
          size_t count) -> jsi::Value {
        const void* queue = lifetime->require(r);
        if (count < 2)
          throw_primitive_type_error(r, "response field needs a handle and a field id");
        // Shapes first, ownership second: nothing below converts an
        // unvalidated double.
        if (!args[0].isNumber())
          throw_primitive_type_error(r, "response handle must be a number");
        const uint64_t field =
            primitive_integer(r, args[1], 0.0, 4294967295.0, "response field id");
        uint64_t primitive_response = 0;
        if (primitive_handles != nullptr) {
          primitive_response = primitive_handle(r, args[0], "response handle");
          if (!known_response_field(field))
            throw_primitive_range_error(
                r, "unknown response field " + std::to_string(field));
          if (field == 3 && (count < 3 || !args[2].isString()))
            throw_primitive_type_error(r, "response header name must be a string");
          require_owned(r, primitive_response, primitive_handles->responses,
                        "response handle");
        }
        std::vector<std::string> owned;
        Ibex2AbiValue name{IBEX2_TAG_UNDEFINED, 0.0, nullptr, 0};
        if (count >= 3) name = to_abi(r, args[2], owned);
        Ibex2AbiValue out{IBEX2_TAG_UNDEFINED, 0.0, nullptr, 0};
        int status = ibex2_response_field(
            queue, args[0].getNumber(), static_cast<uint32_t>(field),
            count >= 3 ? &name : nullptr, &out);
        struct Release {
          Ibex2AbiValue& value;
          ~Release() { ibex2_host_release(&value); }
        } release{out};
        auto result = from_abi(r, out);
        if (status != 0) {
          // Arguments were validated above, so for the primitives the one
          // remaining failure is a row the ABI no longer has.
          if (primitive_handles != nullptr)
            primitive_handles->responses.erase(primitive_response);
          throw jsi::JSError(r, result.isString()
              ? result.getString(r).utf8(r) : std::string("response read failed"));
        }
        if (primitive_handles != nullptr && field == 8)
          primitive_handles->responses.erase(primitive_response);
        return result;
      });
}

void install_fetch(jsi::Runtime& rt, Adapter& adapter,
                   const std::shared_ptr<Lifetime>& lifetime) {
  auto global = rt.global();
  set_group_binding(rt, global, "__ibex2_fetch_control", 72, lifetime);
  global.setProperty(rt, "__ibex2_response_own",
      jsi::Function::createFromHostFunction(rt,
          jsi::PropNameID::forAscii(rt, "__ibex2_response_own"), 2,
          [lifetime](jsi::Runtime& r, const jsi::Value&,
                     const jsi::Value* args, size_t count) -> jsi::Value {
            const void* queue = lifetime->require(r);
            if (count != 2 || !args[0].isNumber() || !args[1].isObject())
              throw jsi::JSError(r, "response owner needs a handle and a body");
            auto body = args[1].getObject(r);
            body.setNativeState(r, std::make_shared<ResponseOwner>(
                ibex2_response_owner_create(queue, args[0].asNumber())));
            auto weak = std::make_shared<jsi::WeakObject>(r, body);
            return jsi::Function::createFromHostFunction(r,
                jsi::PropNameID::forAscii(r, "responseBody"), 0,
                [weak](jsi::Runtime& r, const jsi::Value&, const jsi::Value*,
                       size_t) -> jsi::Value { return weak->lock(r); });
          }));
  global.setProperty(rt, "__ibex2_response_read",
                     adapter.async_binding("__ibex2_response_read", 102, nullptr));
  global.setProperty(rt, "__ibex2_response_field",
                     make_response_field(rt, lifetime));
}

jsi::Object make_fetch_primitives(
    jsi::Runtime& rt, Adapter& adapter,
    const std::shared_ptr<Lifetime>& lifetime, const void* grants) {
  auto handles = std::make_shared<FetchPrimitiveHandles>();
  jsi::Object primitives(rt);
  auto raw_fetch = std::make_shared<jsi::Function>(
      adapter.async_binding("fetch", 101, grants));
  primitives.setProperty(
      rt, "fetch",
      jsi::Function::createFromHostFunction(
          rt, jsi::PropNameID::forAscii(rt, "fetch"), 6,
          [lifetime, handles, raw_fetch](
              jsi::Runtime& r, const jsi::Value&, const jsi::Value* args,
              size_t count) -> jsi::Value {
            const void* state = lifetime->require(r);
            // Exactly six arguments cross; a missing one is undefined.
            std::vector<jsi::Value> call;
            call.reserve(6);
            for (size_t i = 0; i < 6; ++i)
              call.push_back(i < count ? jsi::Value(r, args[i])
                                       : jsi::Value::undefined());
            // The headers handle is validated and adopted first, so once it
            // is accepted it is releasable through headersFree whatever this
            // call does next -- including throwing below.
            if (!call[4].isUndefined()) {
              const uint64_t header =
                  primitive_handle(r, call[4], "fetch headers handle");
              try {
                call_host(r, state, 46, &call[4], 1);
              } catch (const jsi::JSError&) {
                throw_primitive_type_error(r, "unknown fetch headers handle");
              }
              handles->headers.insert(header);
            }
            if (!call[0].isString())
              throw_primitive_type_error(r, "fetch url must be a string");
            if (!call[1].isUndefined() && !call[1].isString())
              throw_primitive_type_error(r, "fetch method must be a string or undefined");
            if (!call[2].isUndefined() && !call[2].isNull()) {
              const bool bytes = call[2].isObject() &&
                  (call[2].getObject(r).isArrayBuffer(r) ||
                   call[2].getObject(r).isTypedArray(r));
              if (!bytes)
                throw_primitive_type_error(
                    r, "fetch body must be undefined, null, an ArrayBuffer, or a typed array");
            }
            if (!call[3].isUndefined() && !call[3].isString())
              throw_primitive_type_error(r, "fetch redirect must be a string or undefined");
            if (!call[5].isUndefined())
              require_primitive_handle(r, call[5], handles->controls,
                                       "fetch control token");
            auto promise =
                raw_fetch->call(r, static_cast<const jsi::Value*>(call.data()),
                                call.size())
                    .getObject(r);
            auto own_response = jsi::Function::createFromHostFunction(
                r, jsi::PropNameID::forAscii(r, "ownFetchResponse"), 1,
                [lifetime, handles](
                    jsi::Runtime& r, const jsi::Value&,
                    const jsi::Value* args, size_t count) -> jsi::Value {
                  lifetime->require(r);
                  if (count != 1)
                    throw_primitive_type_error(r, "fetch returned no response handle");
                  const uint64_t response =
                      primitive_handle(r, args[0], "fetch response handle");
                  handles->responses.insert(response);
                  return jsi::Value(r, args[0]);
                });
            return promise.getPropertyAsFunction(r, "then").callWithThis(
                r, promise, std::move(own_response));
          }));
  primitives.setProperty(rt, "responseField",
      make_response_field(rt, lifetime, handles));
  auto raw_read = std::make_shared<jsi::Function>(
      adapter.async_binding("responseRead", 102, nullptr));
  primitives.setProperty(
      rt, "responseRead",
      jsi::Function::createFromHostFunction(
          rt, jsi::PropNameID::forAscii(rt, "responseRead"), 1,
          [lifetime, handles, raw_read](
              jsi::Runtime& r, const jsi::Value&, const jsi::Value* args,
              size_t count) -> jsi::Value {
            lifetime->require(r);
            if (count < 1)
              throw_primitive_type_error(r, "response handle is required");
            const uint64_t response = require_primitive_handle(
                r, args[0], handles->responses, "response handle");
            auto promise = raw_read->call(r, args, size_t{1}).getObject(r);
            auto complete = jsi::Function::createFromHostFunction(
                r, jsi::PropNameID::forAscii(r, "completeResponseRead"), 1,
                [lifetime, handles, response](
                    jsi::Runtime& r, const jsi::Value&,
                    const jsi::Value* values, size_t value_count) -> jsi::Value {
                  lifetime->require(r);
                  if (value_count != 1)
                    throw_primitive_type_error(r, "response read returned no value");
                  if (values[0].isNull()) handles->responses.erase(response);
                  return jsi::Value(r, values[0]);
                });
            auto fail = jsi::Function::createFromHostFunction(
                r, jsi::PropNameID::forAscii(r, "failResponseRead"), 1,
                [lifetime, handles, response](
                    jsi::Runtime& r, const jsi::Value&,
                    const jsi::Value* values, size_t value_count) -> jsi::Value {
                  lifetime->require(r);
                  handles->responses.erase(response);
                  if (value_count == 1)
                    throw jsi::JSError(r, jsi::Value(r, values[0]));
                  throw jsi::JSError(r, "response read failed");
                });
            return promise.getPropertyAsFunction(r, "then").callWithThis(
                r, promise, std::move(complete), std::move(fail));
          }));
  primitives.setProperty(
      rt, "fetchControl",
      jsi::Function::createFromHostFunction(
          rt, jsi::PropNameID::forAscii(rt, "fetchControl"), 2,
          [lifetime, handles](
              jsi::Runtime& r, const jsi::Value&, const jsi::Value* args,
              size_t count) -> jsi::Value {
            const void* state = lifetime->require(r);
            if (count < 1)
              throw_primitive_type_error(r, "fetch control action is required");
            // 0 allocates, 1 aborts, 2 releases; nothing else crosses.
            const uint64_t action =
                primitive_integer(r, args[0], 0.0, 2.0, "fetch control action");
            if (action == 0) {
              auto result = call_host(r, state, 72, args, 1);
              handles->controls.insert(
                  primitive_handle(r, result, "fetch control token"));
              return result;
            }
            if (count < 2)
              throw_primitive_type_error(r, "fetch control token is required");
            const uint64_t token = require_primitive_handle(
                r, args[1], handles->controls, "fetch control token");
            auto result = call_host(r, state, 72, args, 2);
            if (action == 2) handles->controls.erase(token);
            return result;
          }));
  primitives.setProperty(rt, "textEncode",
      make_group_binding(rt, "textEncode", 20, lifetime));
  primitives.setProperty(rt, "textDecode",
      make_group_binding(rt, "textDecode", 21, lifetime));
  primitives.setProperty(rt, "textEncodeInto",
      make_group_binding(rt, "textEncodeInto", 22, lifetime));
  primitives.setProperty(
      rt, "headersFree",
      jsi::Function::createFromHostFunction(
          rt, jsi::PropNameID::forAscii(rt, "headersFree"), 1,
          [lifetime, handles](
              jsi::Runtime& r, const jsi::Value&, const jsi::Value* args,
              size_t count) -> jsi::Value {
            const void* state = lifetime->require(r);
            if (count < 1)
              throw_primitive_type_error(r, "headers handle is required");
            const uint64_t header = require_primitive_handle(
                r, args[0], handles->headers, "headers handle");
            auto result = call_host(r, state, 51, args, 1);
            handles->headers.erase(header);
            return result;
          }));
  freeze(rt, primitives);
  return primitives;
}

// The members of the published object, in the order their identities are
// recorded after the object's own identity.
constexpr const char* kFetchPrimitiveMembers[] = {
    "fetch", "responseField", "responseRead", "fetchControl",
    "textEncode", "textDecode", "textEncodeInto", "headersFree"};

// Bounds the walk on a pathological graph; reaching it refuses (fails closed).
constexpr size_t kMaxReachabilityObjects = size_t{1} << 20;

// Walk what harden.js freezes and refuse if any recorded identity is in it.
//
// Coverage mirrors harden.js exactly: the global object's own properties
// (string and symbol keys), its prototype chain, and transitively every object
// or function reached through an own property -- a data property's value, or
// an accessor's getter and setter functions themselves -- plus each reached
// object's prototype. Descriptors are read with the captured
// Object.getOwnPropertyDescriptor, so no getter is ever invoked. A Set of
// visited objects makes the walk cycle-safe; the explicit stack keeps it off
// the native stack; kMaxReachabilityObjects bounds it.
//
// Not covered, because harden.js cannot see it either: values held only in
// closure scopes (including a getter's closure), in native state, in
// WeakMap/Map/Set entries, behind a Proxy whose traps conceal them, or in an
// object reachable only from somewhere other than the global object. Those
// stay the trusted bootstrap's obligation.
// @ref LLP 0068#opt-in-fetch-primitives-protocol — L1e: the harden guard proves the bootstrap handoff did not leave a path to the primitives
// @ref LLP 0068#opt-in-abort-hooks-protocol — I4 applies the same reachability proof to the abort hooks
void require_unreachable(
    jsi::Runtime& rt, const Reachability& walk,
    const std::vector<std::pair<std::string, jsi::Value>>& identities) {
  try {
    auto global = rt.global();
    auto seen = walk.set.callAsConstructor(rt).getObject(rt);
    std::vector<jsi::Value> pending;
    auto expand = [&](const jsi::Object& object) {
      for (const auto* list : {&walk.names, &walk.symbols}) {
        auto keys = list->call(rt, object).getObject(rt).getArray(rt);
        const size_t count = keys.size(rt);
        for (size_t i = 0; i < count; ++i) {
          auto raw = walk.descriptor.call(rt, object, keys.getValueAtIndex(rt, i));
          if (!raw.isObject()) continue;
          auto descriptor = raw.getObject(rt);
          // Classify by the descriptor record's OWN fields, read with the
          // captured getOwnPropertyDescriptor. `hasProperty`/`getProperty`
          // would consult Object.prototype, where bootstrap can plant a
          // `value` that hides an accessor's getter (or runs a getter).
          auto own = [&](const char* field) {
            return walk.descriptor.call(
                rt, descriptor, jsi::String::createFromAscii(rt, field));
          };
          auto value = own("value");
          if (value.isObject()) {
            pending.push_back(value.getObject(rt).getProperty(rt, "value"));
          } else {
            for (const char* field : {"get", "set"}) {
              auto accessor = own(field);
              if (accessor.isObject())
                pending.push_back(accessor.getObject(rt).getProperty(rt, "value"));
            }
          }
        }
      }
      pending.push_back(walk.prototype.call(rt, object));
    };
    walk.add.callWithThis(rt, seen, global);
    expand(global);
    size_t visited = 0;
    while (!pending.empty()) {
      jsi::Value value = std::move(pending.back());
      pending.pop_back();
      if (!value.isObject()) continue;
      if (walk.has.callWithThis(rt, seen, value).getBool()) continue;
      walk.add.callWithThis(rt, seen, value);
      if (++visited > kMaxReachabilityObjects)
        throw std::runtime_error(
            "refusing to harden: the global object graph is too large to "
            "prove trusted-bootstrap outputs unreachable");
      auto object = value.getObject(rt);
      for (const auto& [label, identity] : identities) {
        if (jsi::Object::strictEquals(rt, object, identity.getObject(rt)))
          throw std::runtime_error(
              "refusing to harden: " + label +
              " is still reachable from the global object");
      }
      expand(object);
    }
  } catch (const jsi::JSIException& error) {
    throw std::runtime_error(
        std::string("refusing to harden: could not prove trusted-bootstrap outputs "
                    "unreachable: ") + error.what());
  }
}

jsi::Object make_process(jsi::Runtime& rt, const void* grants) {
  jsi::Object env(rt);
  const size_t count = ibex2_grants_env_count(grants);
  for (size_t i = 0; i < count; ++i) {
    char* name = nullptr;
    char* value = nullptr;
    if (ibex2_grants_env_at(grants, i, &name, &value) == 0) continue;
    env.setProperty(rt, jsi::PropNameID::forUtf8(rt, std::string(name)),
                    jsi::String::createFromUtf8(rt, std::string(value)));
    ibex2_string_free(name);
    ibex2_string_free(value);
  }
  freeze(rt, env);
  jsi::Object process(rt);
  process.setProperty(rt, "env", std::move(env));
  freeze(rt, process);
  return process;
}

void remove_global(jsi::Runtime& rt, const jsi::Object& global,
                   const char* name) {
  rt.global().getPropertyAsObject(rt, "Reflect")
      .getPropertyAsFunction(rt, "deleteProperty")
      .call(rt, global, jsi::String::createFromUtf8(rt, name));
}

uint64_t websocket_handle(jsi::Runtime& rt, const jsi::Value& value) {
  if (!value.isNumber()) throw jsi::JSError(rt, "invalid WebSocket handle");
  double number = value.asNumber();
  uint64_t handle = static_cast<uint64_t>(number);
  if (number < 1 || number > 9007199254740991.0 || number != handle)
    throw jsi::JSError(rt, "invalid WebSocket handle");
  return handle;
}

void websocket_result(jsi::Runtime& rt, int status, char* error,
                      const char* fallback) {
  std::string message = error == nullptr ? fallback : std::string(error);
  if (error != nullptr) ibex2_string_free(error);
  if (status != 0) throw jsi::JSError(rt, message);
}
} // namespace

void validate_groups(Groups groups) { validate_groups_impl(groups); }

std::vector<const char*> expected_scripts(Groups groups) {
  validate_groups_impl(groups);
  return expected_scripts_impl(groups);
}

jsi::Object Adapter::websocket_hooks(const void* grants) {
  auto& rt = *runtime_;
  auto lifetime = state_->lifetime;
  std::weak_ptr<State> weak_state = state_;
  std::shared_ptr<const void> authority(
      ibex2_grants_retain(grants), ibex2_grants_destroy);
  jsi::Object hooks(rt);
  hooks.setProperty(rt, "supported", ibex2_websocket_supported() != 0);

  hooks.setProperty(rt, "open", jsi::Function::createFromHostFunction(
      rt, jsi::PropNameID::forAscii(rt, "open"), 4,
      [lifetime, weak_state, authority](
          jsi::Runtime& r, const jsi::Value&, const jsi::Value* args,
          size_t count) -> jsi::Value {
        const void* queue = lifetime->require(r);
        auto state = weak_state.lock();
        if (!state || !state->alive)
          throw jsi::JSError(r, "Ibex2 bindings are detached");
        if (count != 4 || !args[0].isObject() || !args[1].isObject() ||
            !args[1].getObject(r).isFunction(r) || !args[2].isString() ||
            !args[3].isString())
          throw jsi::JSError(r, "WebSocket open needs an owner and callback");
        auto owner_object = args[0].getObject(r);
        auto callback = args[1].getObject(r).getFunction(r);
        std::string url = args[2].getString(r).utf8(r);
        std::string protocols = args[3].getString(r).utf8(r);
        uint64_t subscription = 0;
        void* rust = ibex2_subscription_create(queue, &subscription);
        if (rust == nullptr)
          throw jsi::JSError(r, "could not create WebSocket subscription");
        char* error = nullptr;
        uint64_t handle = ibex2_websocket_open(
            queue, authority.get(), subscription,
            reinterpret_cast<const uint8_t*>(url.data()), url.size(),
            reinterpret_cast<const uint8_t*>(protocols.data()), protocols.size(),
            16u << 20, &error);
        if (handle == 0) {
          ibex2_subscription_destroy(rust);
          websocket_result(r, 1, error, "WebSocket open failed");
        }
        auto native = std::make_shared<WebSocketOwner>(
            ibex2_websocket_owner_create(queue, handle), rust);
        if (native->connection == nullptr) {
          throw jsi::JSError(r, "could not retain WebSocket");
        }
        try {
          owner_object.setNativeState(r, native);
          auto weak_owner = std::make_shared<jsi::WeakObject>(r, owner_object);
          state->subscriptions.emplace(
              subscription,
              State::EventSubscription{
                  nullptr, std::move(callback), handle, std::move(weak_owner),
                  jsi::Value::undefined(), false, native});
        } catch (...) {
          throw;
        }
        return jsi::Value(static_cast<double>(handle));
      }));

  hooks.setProperty(rt, "sendText", jsi::Function::createFromHostFunction(
      rt, jsi::PropNameID::forAscii(rt, "sendText"), 2,
      [lifetime, weak_state](jsi::Runtime& r, const jsi::Value&,
                 const jsi::Value* args, size_t count) -> jsi::Value {
        const void* queue = lifetime->require(r);
        if (count != 2 || !args[1].isString())
          throw jsi::JSError(r, "WebSocket send needs string data");
        uint64_t handle = websocket_handle(r, args[0]);
        std::string text = args[1].getString(r).utf8(r);
        char* error = nullptr;
        int status = ibex2_websocket_send(
            queue, handle, 0,
            reinterpret_cast<const uint8_t*>(text.data()), text.size(), &error);
        websocket_result(r, status, error, "WebSocket text send failed");
        if (auto state = weak_state.lock(); state && state->alive) {
          for (auto& entry : state->subscriptions) {
            if (entry.second.websocket == handle &&
                entry.second.weak_owner != nullptr) {
              auto owner = entry.second.weak_owner->lock(r);
              bool keep = entry.second.listener_keepalive ||
                  (ibex2_websocket_ready_state(queue, handle) == 1 &&
                   ibex2_websocket_buffered_amount(queue, handle) != 0);
              entry.second.strong_owner = keep && owner.isObject()
                  ? jsi::Value(r, owner) : jsi::Value::undefined();
              break;
            }
          }
        }
        return jsi::Value::undefined();
      }));

  hooks.setProperty(rt, "sendBinary", jsi::Function::createFromHostFunction(
      rt, jsi::PropNameID::forAscii(rt, "sendBinary"), 2,
      [lifetime, weak_state](jsi::Runtime& r, const jsi::Value&,
                 const jsi::Value* args, size_t count) -> jsi::Value {
        const void* queue = lifetime->require(r);
        if (count != 2) throw jsi::JSError(r, "WebSocket send needs data");
        uint64_t handle = websocket_handle(r, args[0]);
        std::vector<std::string> owned;
        auto bytes = to_abi(r, args[1], owned);
        if (bytes.tag != IBEX2_TAG_BYTES)
          throw jsi::JSError(r, "WebSocket binary data needs an ArrayBuffer or view");
        char* error = nullptr;
        int status = ibex2_websocket_send(
            queue, handle, 1, bytes.data, bytes.len, &error);
        websocket_result(r, status, error, "WebSocket binary send failed");
        if (auto state = weak_state.lock(); state && state->alive) {
          for (auto& entry : state->subscriptions) {
            if (entry.second.websocket == handle &&
                entry.second.weak_owner != nullptr) {
              auto owner = entry.second.weak_owner->lock(r);
              bool keep = entry.second.listener_keepalive ||
                  (ibex2_websocket_ready_state(queue, handle) == 1 &&
                   ibex2_websocket_buffered_amount(queue, handle) != 0);
              entry.second.strong_owner = keep && owner.isObject()
                  ? jsi::Value(r, owner) : jsi::Value::undefined();
              break;
            }
          }
        }
        return jsi::Value::undefined();
      }));

  hooks.setProperty(rt, "close", jsi::Function::createFromHostFunction(
      rt, jsi::PropNameID::forAscii(rt, "close"), 3,
      [lifetime](jsi::Runtime& r, const jsi::Value&,
                 const jsi::Value* args, size_t count) -> jsi::Value {
        const void* queue = lifetime->require(r);
        if (count != 3 || !args[2].isString())
          throw jsi::JSError(r, "WebSocket close needs its arguments");
        uint64_t handle = websocket_handle(r, args[0]);
        int code = static_cast<int>(args[1].asNumber());
        std::string reason = args[2].getString(r).utf8(r);
        char* error = nullptr;
        int status = ibex2_websocket_close(
            queue, handle, code,
            reinterpret_cast<const uint8_t*>(reason.data()), reason.size(), &error);
        websocket_result(r, status, error, "WebSocket close failed");
        return jsi::Value::undefined();
      }));

  hooks.setProperty(rt, "readyState", jsi::Function::createFromHostFunction(
      rt, jsi::PropNameID::forAscii(rt, "readyState"), 1,
      [lifetime](jsi::Runtime& r, const jsi::Value&,
                 const jsi::Value* args, size_t count) -> jsi::Value {
        const void* queue = lifetime->require(r);
        if (count != 1) throw jsi::JSError(r, "WebSocket state needs a handle");
        return jsi::Value(ibex2_websocket_ready_state(
            queue, websocket_handle(r, args[0])));
      }));

  hooks.setProperty(rt, "bufferedAmount", jsi::Function::createFromHostFunction(
      rt, jsi::PropNameID::forAscii(rt, "bufferedAmount"), 1,
      [lifetime](jsi::Runtime& r, const jsi::Value&,
                 const jsi::Value* args, size_t count) -> jsi::Value {
        const void* queue = lifetime->require(r);
        if (count != 1) throw jsi::JSError(r, "WebSocket amount needs a handle");
        return jsi::Value(static_cast<double>(ibex2_websocket_buffered_amount(
            queue, websocket_handle(r, args[0]))));
      }));

  hooks.setProperty(rt, "setKeepalive", jsi::Function::createFromHostFunction(
      rt, jsi::PropNameID::forAscii(rt, "setKeepalive"), 2,
      [lifetime, weak_state](jsi::Runtime& r, const jsi::Value&,
                            const jsi::Value* args, size_t count) -> jsi::Value {
        const void* queue = lifetime->require(r);
        if (count != 2 || !args[1].isBool())
          throw jsi::JSError(r, "WebSocket keepalive needs a handle and flag");
        uint64_t handle = websocket_handle(r, args[0]);
        auto state = weak_state.lock();
        if (!state || !state->alive)
          throw jsi::JSError(r, "Ibex2 bindings are detached");
        for (auto& entry : state->subscriptions) {
          if (entry.second.websocket != handle ||
              entry.second.weak_owner == nullptr) continue;
          auto owner = entry.second.weak_owner->lock(r);
          auto has_listener = [&](const char* type) {
            if (!owner.isObject() || !state->event_listener_query.isObject() ||
                !state->event_listener_query.getObject(r).isFunction(r))
              return false;
            auto result = state->event_listener_query.getObject(r)
                .getFunction(r).call(
                    r, owner, jsi::String::createFromAscii(r, type));
            return result.isBool() && result.getBool();
          };
          int ready = ibex2_websocket_ready_state(queue, handle);
          bool listeners = ready == 0
              ? has_listener("open") || has_listener("message") ||
                    has_listener("error") || has_listener("close")
              : ready == 1
              ? has_listener("message") || has_listener("error") ||
                    has_listener("close")
              : has_listener("error") || has_listener("close");
          entry.second.listener_keepalive = listeners;
          bool keep = entry.second.listener_keepalive ||
              (ready == 1 &&
               ibex2_websocket_buffered_amount(queue, handle) != 0);
          entry.second.strong_owner = keep && owner.isObject()
              ? jsi::Value(r, owner) : jsi::Value::undefined();
          break;
        }
        return jsi::Value::undefined();
      }));

  return hooks;
}

void Adapter::install(Groups groups, const Ibex2Bindings* bindings,
                      const CompiledScript* scripts, size_t script_count) {
  install_with(groups, bindings, scripts, script_count, InstallOptions{});
}

void Adapter::install_with(Groups groups, const Ibex2Bindings* bindings,
                           const CompiledScript* scripts, size_t script_count,
                           const InstallOptions& options) {
  if (!runtime_) throw std::logic_error("Ibex2 bindings are detached");
  auto& rt = *runtime_;
  state_->require(rt);
  if (state_->install_status == InstallStatus::Installed)
    throw std::logic_error("Ibex2 bindings are already installed");
  if (state_->install_status == InstallStatus::Spent)
    throw std::logic_error(
        "a previous Ibex2 binding installation failed; the Adapter is spent "
        "and the runtime must be discarded");

  // An Adapter has one installation attempt. Preflight failures have not
  // changed JavaScript, but consuming the attempt keeps retry behavior
  // deterministic. Once mutation starts, every failure below is additionally
  // reported as a terminal runtime failure.
  state_->install_status = InstallStatus::Spent;
  bool mutation_started = false;
  try {
    const void* endowed_state = ibex2_bindings_state(bindings);
    const void* grants = ibex2_bindings_grants(bindings);
    if (endowed_state == nullptr || grants == nullptr)
      throw std::invalid_argument("Ibex2 bindings require a live endowment");
    if (endowed_state != state_->queue)
      throw std::invalid_argument("Ibex2 bindings do not belong to this runtime state");
    validate_groups(groups);
    std::string fetch_primitives;
    std::string abort_hooks;
    std::unique_ptr<Reachability> reachability;
    auto read_bootstrap_name = [&](const char* option, const char* kind,
                                   Groups required) {
      if (option == nullptr) return std::string{};
      std::string name(option);
      if (name.empty())
        throw std::invalid_argument(
            std::string(kind) + " require a non-empty global name");
      if (!is_ascii_identifier(name))
        throw std::invalid_argument(
            std::string(kind) + " global name must be an ASCII JavaScript "
            "identifier ([A-Za-z_$][A-Za-z0-9_$]*)");
      if (!has(groups, required))
        throw std::invalid_argument(
            std::string(kind) + " require the " +
            (required == GROUP_FETCH ? "FETCH" : "ABORT") + " group");
      // hasProperty follows the global's prototype chain, so inherited names
      // such as `toString` or `__proto__` collide as well.
      if (rt.global().hasProperty(rt, bootstrap_output_key(rt, name)))
        throw std::invalid_argument(
            std::string(kind) + " global already exists");
      return name;
    };
    fetch_primitives = read_bootstrap_name(
        options.fetch_primitives, "fetch primitives", GROUP_FETCH);
    abort_hooks = read_bootstrap_name(
        options.abort_hooks, "abort hooks", GROUP_ABORT);
    if (!fetch_primitives.empty() && fetch_primitives == abort_hooks)
      throw std::invalid_argument(
          "trusted-bootstrap outputs require distinct global names");
    if (!fetch_primitives.empty() || !abort_hooks.empty())
      reachability = std::make_unique<Reachability>(rt);
    auto expected = expected_scripts(groups);
    if (script_count != expected.size() || (script_count != 0 && scripts == nullptr))
      throw std::invalid_argument("Ibex2 binding bytecode count does not match groups");
    jsi::Value published_abort_hooks = jsi::Value::undefined();
    for (size_t i = 0; i < script_count; ++i) {
      if (scripts[i].name == nullptr || scripts[i].bytes == nullptr ||
          std::strcmp(scripts[i].name, expected[i]) != 0)
        throw std::invalid_argument("Ibex2 binding bytecode is not in scripts() order");
      validate_bytecode(scripts[i], state_->bytecode_version);
    }

    // Validation above is deliberately complete before the first host function
    // or JavaScript global is installed: one bad payload refuses the whole door.
    mutation_started = true;
    if (has(groups, GROUP_CONSOLE)) install_console(rt, state_->lifetime);
    if (has(groups, GROUP_PURE)) install_pure(rt, state_->lifetime);
    if (has(groups, GROUP_TIMERS)) install_timers(rt, state_->lifetime);
    if (has(groups, GROUP_CRYPTO)) install_crypto(rt, state_->lifetime);
    if (has(groups, GROUP_EVENTS)) install_events(rt, state_->lifetime);
    if (has(groups, GROUP_BLOB)) install_blob(rt, state_->lifetime);
    if (has(groups, GROUP_FETCH)) install_fetch(rt, *this, state_->lifetime);
#if defined(IBEX2_JSI_HAS_INTL)
    if (has(groups, GROUP_INTL)) {
      ibex2::intl_number_format::install(rt, state_->lifetime);
      ibex2::intl_case::install(rt, state_->lifetime);
    }
#endif

    for (size_t i = 0; i < script_count; ++i) {
      const auto& script = scripts[i];
      if (std::strcmp(script.name, "websocket") == 0) {
        rt.global().setProperty(rt, "__ibex2_fire_trusted_event",
                                jsi::Value(rt, state_->trusted_event_dispatch));
        if (state_->blob_helpers.isObject())
          rt.global().setProperty(rt, "__ibex2_blob_helpers",
                                  state_->blob_helpers);
      }
      if (std::strcmp(script.name, "fetch") == 0 &&
          state_->blob_helpers.isObject())
        rt.global().setProperty(rt, "__ibex2_blob_helpers",
                                state_->blob_helpers);
      auto buffer = std::make_shared<CompiledBytes>(script.bytes, script.len);
      auto value = rt.evaluateJavaScript(buffer, std::string(script.name) + ".js");
      if (std::strcmp(script.name, "abort") == 0 && !abort_hooks.empty()) {
        auto hooks = rt.global().getProperty(rt, "__ibex2_abort");
        if (!hooks.isObject())
          throw jsi::JSError(rt, "abort binding did not publish its hooks");
        auto object = hooks.getObject(rt);
        for (const char* member : {"own", "subscribe"}) {
          auto function = object.getProperty(rt, member);
          if (!function.isObject() || !function.getObject(rt).isFunction(rt))
            throw jsi::JSError(rt, "abort binding published invalid hooks");
        }
        freeze(rt, object);
        published_abort_hooks = jsi::Value(rt, object);
      }
      if (std::strcmp(script.name, "blob") == 0) {
        if (!value.isObject())
          throw jsi::JSError(rt, "Blob binding did not evaluate to helpers");
        state_->blob_helpers = jsi::Value(rt, value);
        continue;
      }
      if (std::strcmp(script.name, "fetch") == 0) {
        if (!value.isObject() || !value.getObject(rt).isFunction(rt))
          throw jsi::JSError(rt, "fetch binding did not evaluate to a factory");
        state_->fetch_factory = jsi::Value(rt, value);
        continue;
      }
      if (std::strcmp(script.name, "websocket") == 0) {
        if (!value.isObject() || !value.getObject(rt).isFunction(rt))
          throw jsi::JSError(rt, "WebSocket binding did not evaluate to a factory");
        state_->websocket_factory = jsi::Value(rt, value);
        continue;
      }
      if (std::strcmp(script.name, "sqlite") == 0) {
        if (!value.isObject() || !value.getObject(rt).isFunction(rt))
          throw jsi::JSError(rt, "SQLite binding did not evaluate to a factory");
        state_->sqlite_factory = jsi::Value(rt, value);
        continue;
      }
      if (std::strcmp(script.name, "events") == 0) {
        if (!value.isObject())
          throw jsi::JSError(rt, "events binding did not return its engine hooks");
        auto hooks = value.getObject(rt);
        auto capture = [&](const char* name, jsi::Value& slot) {
          auto hook = hooks.getProperty(rt, name);
          if (!hook.isObject() || !hook.getObject(rt).isFunction(rt))
            throw jsi::JSError(rt, "events binding returned an invalid engine hook");
          slot = jsi::Value(rt, hook);
        };
        capture("reportException", state_->event_reporter);
        capture("onUnhandled", state_->rejection_unhandled);
        capture("onHandled", state_->rejection_handled);
        capture("fireTrustedEvent", state_->trusted_event_dispatch);
        capture("hasEventListener", state_->event_listener_query);
        capture("setListenerChangeHook", state_->event_listener_change_hook);
        if (has(groups, GROUP_ABORT)) {
          // The hook crosses only the next installation step. abort.js takes
          // and deletes it before any application entrance can run.
          rt.global().setProperty(
              rt, "__ibex2_fire_trusted_event",
              jsi::Value(rt, state_->trusted_event_dispatch));
          auto set_abort_hooks = hooks.getProperty(rt, "setAbortHooks");
          if (!set_abort_hooks.isObject() ||
              !set_abort_hooks.getObject(rt).isFunction(rt))
            throw jsi::JSError(
                rt, "events binding returned an invalid abort-hook bridge");
          rt.global().setProperty(
              rt, "__ibex2_set_event_abort_hooks", std::move(set_abort_hooks));
        }
        continue;
      }
#if defined(IBEX2_JSI_HAS_INTL)
      if (std::strcmp(script.name, "intl_datetime") == 0) {
        if (!value.isObject() || !value.getObject(rt).isFunction(rt))
          throw jsi::JSError(rt, "DateTimeFormat binding did not evaluate to a factory");
        auto arguments =
            ibex2::intl_datetime::factory_arguments(rt, state_->lifetime);
        value.getObject(rt).getFunction(rt).call(
            rt, static_cast<const jsi::Value*>(arguments.data()), arguments.size());
      }
#endif
    }

    if (has(groups, GROUP_EVENTS) && has(groups, GROUP_ABORT)) {
      remove_global(rt, rt.global(), "__ibex2_fire_trusted_event");
      remove_global(rt, rt.global(), "__ibex2_set_event_abort_hooks");
    }
    if (has(groups, GROUP_WEBSOCKET)) {
      remove_global(rt, rt.global(), "__ibex2_fire_trusted_event");
    }

    // @ref LLP 0057.000#l3--events-abort-and-the-second-direction — vanilla Hermes has delayed tracker callbacks, not the patched checkpoint hook
    // Stock Hermes exposes its JavaScript Promise rejection tracker through
    // HermesInternal. The upstream tracker uses setTimeout (100 ms for the
    // standard programmer-error classes, two seconds otherwise), so EVENTS
    // remains independent and tracking is enabled only when TIMERS was also
    // selected. Engines without this optional intrinsic simply omit the two
    // rejection events; ordinary EventTarget and reportError still work.
    if (has(groups, GROUP_EVENTS) && has(groups, GROUP_TIMERS)) {
      auto internal_value = rt.global().getProperty(rt, "HermesInternal");
      if (internal_value.isObject()) {
        auto internal = internal_value.getObject(rt);
        auto enable = internal.getProperty(rt, "enablePromiseRejectionTracker");
        if (enable.isObject() && enable.getObject(rt).isFunction(rt) &&
            state_->rejection_unhandled.isObject() &&
            state_->rejection_handled.isObject()) {
          jsi::Object options(rt);
          options.setProperty(rt, "allRejections", true);
          options.setProperty(rt, "onUnhandled",
                              jsi::Value(rt, state_->rejection_unhandled));
          options.setProperty(rt, "onHandled",
                              jsi::Value(rt, state_->rejection_handled));
          enable.getObject(rt).getFunction(rt).callWithThis(rt, internal, options);
          // The upstream tracker installs its callbacks in Hermes's private
          // Promise._B/_C slots. They are trusted engine mutations made before
          // hardening, so advance the integrity baseline to those exact
          // function identities; the ordinary harden walk freezes them.
          accept_trusted_intrinsic_property(
              rt.global().getPropertyAsObject(rt, "Promise"), "_B");
          accept_trusted_intrinsic_property(
              rt.global().getPropertyAsObject(rt, "Promise"), "_C");
        }
      }
    }

#if defined(IBEX2_JSI_HAS_INTL)
    if (has(groups, GROUP_INTL)) {
      auto global = rt.global();
      auto accept = [&](const char* constructor, const char* property) {
        auto prototype = global.getPropertyAsObject(rt, constructor)
                             .getPropertyAsObject(rt, "prototype");
        accept_trusted_intrinsic_property(std::move(prototype), property);
      };
      accept("Number", "toLocaleString");
      accept("BigInt", "toLocaleString");
      accept("String", "toLocaleLowerCase");
      accept("String", "toLocaleUpperCase");
    }
#endif

    state_->groups = groups;
    auto global = rt.global();
    if (has(groups, GROUP_FETCH)) {
      global.setProperty(rt, "fetch", fetch(grants));
    } else if (has(groups, GROUP_PURE)) {
      for (const char* name : {"__ibex2_headers_free", "__ibex2_text_encode",
                               "__ibex2_text_decode", "__ibex2_text_encode_into"})
        remove_global(rt, global, name);
    }
    if (has(groups, GROUP_WEBSOCKET))
      global.setProperty(rt, "WebSocket", websocket(grants));
    if (has(groups, GROUP_ABORT) && !has(groups, GROUP_FETCH))
      remove_global(rt, global, "__ibex2_abort");
    if (has(groups, GROUP_STORAGE)) {
      auto storage_value = storage(grants);
      global.setProperty(rt, "fs", storage_value.getProperty(rt, "fs"));
      global.setProperty(rt, "sqlite", storage_value.getProperty(rt, "sqlite"));
    }
    if (has(groups, GROUP_ENV))
      global.setProperty(rt, "process", make_process(rt, grants));
    auto publish_bootstrap_output = [&](const char* kind,
                                        const std::string& name,
                                        jsi::Object object,
                                        const auto& members) {
      if (global.hasProperty(rt, bootstrap_output_key(rt, name)))
        throw std::invalid_argument(
            std::string(kind) + " global collides with an installed binding");
      State::BootstrapOutput output{kind, name, {}};
      output.identities.emplace_back(
          std::string("the ") + kind + " object", jsi::Value(rt, object));
      for (const char* member : members)
        output.identities.emplace_back(
            std::string(kind) + " member " + member,
            object.getProperty(rt, member));
      global.setProperty(rt, bootstrap_output_key(rt, name),
                         jsi::Value(rt, object));
      state_->bootstrap_outputs.push_back(std::move(output));
    };
    if (!fetch_primitives.empty()) {
      auto primitives =
          make_fetch_primitives(rt, *this, state_->lifetime, grants);
      publish_bootstrap_output(
          "fetch primitives", fetch_primitives, std::move(primitives),
          kFetchPrimitiveMembers);
    }
    if (!abort_hooks.empty()) {
      if (!published_abort_hooks.isObject())
        throw jsi::JSError(rt, "abort binding hooks were not retained");
      constexpr const char* members[] = {"own", "subscribe"};
      publish_bootstrap_output(
          "abort hooks", abort_hooks,
          published_abort_hooks.getObject(rt), members);
    }
    if (!state_->bootstrap_outputs.empty())
      state_->reachability = std::move(reachability);

    if (options.defer_intrinsic_snapshot) {
      // Installation itself may make narrow trusted intrinsic replacements.
      // Discard the constructor-time baseline only after all installation
      // mutations succeed; capture_intrinsics() will record the embedder's
      // complete post-prelude baseline before hardening. The API snapshots the
      // realm unconditionally; the embedder is responsible for running only
      // its trusted prelude in the interval.
      state_->integrity.reset();
      state_->intrinsic_snapshot_deferred = true;
    }

    state_->install_status = InstallStatus::Installed;
  } catch (const std::exception& error) {
    if (mutation_started)
      throw std::runtime_error(
          std::string("Ibex2 binding installation failed after mutating the runtime; ") +
          "the runtime must be discarded: " + error.what());
    throw;
  } catch (...) {
    if (mutation_started)
      throw std::runtime_error(
          "Ibex2 binding installation failed after mutating the runtime; the "
          "runtime must be discarded");
    throw;
  }
}

void Adapter::verify_fetch_primitives_unreachable() {
  verify_trusted_bootstrap_unreachable();
}

void Adapter::verify_trusted_bootstrap_unreachable() {
  if (!runtime_) throw std::logic_error("Ibex2 bindings are detached");
  auto& rt = *runtime_;
  if (state_->bootstrap_outputs.empty()) return;
  for (const auto& output : state_->bootstrap_outputs) {
    if (rt.global().hasProperty(rt, bootstrap_output_key(rt, output.name)))
      throw std::runtime_error(
          "refusing to harden while " + output.kind + " global \"" +
          output.name + "\" is present");
  }
  for (const auto& output : state_->bootstrap_outputs)
    require_unreachable(rt, *state_->reachability, output.identities);
}

void Adapter::verify_harden_preconditions() {
  if (!runtime_) throw std::logic_error("Ibex2 bindings are detached");
  if (!state_->integrity)
    throw std::runtime_error(
        "refusing to harden before the deferred intrinsic snapshot is captured");
  verify_trusted_bootstrap_unreachable();
}

void Adapter::harden(const CompiledScript& script) {
  if (!runtime_) throw std::logic_error("Ibex2 bindings are detached");
  auto& rt = *runtime_;
  state_->require(rt);
  if (script.bytes == nullptr)
    throw std::invalid_argument("Ibex2 harden requires its compiled bytecode");
  validate_bytecode(script, state_->bytecode_version);
  verify_harden_preconditions();
  state_->hardened = true;
  rt.evaluateJavaScript(std::make_shared<CompiledBytes>(script.bytes, script.len),
                        "harden.js");
}

static jsi::Value filesystem_promise(jsi::Runtime& r, jsi::Value value, uint32_t op) {
  auto promise = value.getObject(r);
  auto convert = jsi::Function::createFromHostFunction(r,
      jsi::PropNameID::forAscii(r, "filesystemResult"), 1,
      [op](jsi::Runtime& r, const jsi::Value&, const jsi::Value* args, size_t count) {
        if (count != 1 || !args[0].isString())
          throw jsi::JSError(r, "invalid filesystem result");
        auto text = args[0].getString(r).utf8(r);
        if (op == 113) {
          std::vector<std::string> names;
          for (size_t start = 0; start < text.size();) {
            auto end = text.find('\0', start);
            if (end == std::string::npos) end = text.size();
            names.push_back(text.substr(start, end - start));
            start = end + 1;
          }
          jsi::Array result(r, names.size());
          for (size_t i = 0; i < names.size(); ++i)
            result.setValueAtIndex(r, i, jsi::String::createFromUtf8(r, names[i]));
          return jsi::Value(r, result);
        }
        std::vector<std::string> fields;
        size_t start = 0;
        for (;;) {
          auto end = text.find('\t', start);
          fields.push_back(text.substr(start, end == std::string::npos ? end : end - start));
          if (end == std::string::npos) break;
          start = end + 1;
        }
        if (fields.size() != 4) throw jsi::JSError(r, "invalid filesystem stat");
        jsi::Object result(r);
        result.setProperty(r, "size", std::stod(fields[0]));
        result.setProperty(r, "isFile", fields[1] == "1");
        result.setProperty(r, "isDirectory", fields[2] == "1");
        result.setProperty(r, "modifiedMs", std::stod(fields[3]));
        return jsi::Value(r, result);
      });
  return promise.getPropertyAsFunction(r, "then").callWithThis(r, promise, convert);
}

jsi::Function Adapter::async_binding(const char* name, uint32_t op, const void* grants) {
  if (!runtime_) throw std::logic_error("Ibex2 bindings are detached");
  auto& rt = *runtime_;
  state_->require(rt);
  auto state = state_;
  std::shared_ptr<const void> authority(ibex2_grants_retain(grants), ibex2_grants_destroy);
  return jsi::Function::createFromHostFunction(rt, jsi::PropNameID::forAscii(rt, name), 1,
      [state, authority, op](jsi::Runtime& r, const jsi::Value&, const jsi::Value* args, size_t count) {
        state->require(r);
        if (op == 150) {
          if (!state->integrity)
            throw jsi::JSError(
                r, "Ibex2 SQLite requires capture_intrinsics() before use");
          state->integrity->require(r);
        }
        std::vector<std::string> owned;
        std::vector<Ibex2AbiValue> abi;
        owned.reserve(count); abi.reserve(count);
        for (size_t i = 0; i < count; ++i) abi.push_back(to_abi(r, args[i], owned));
        uint64_t id = state->next_task_id++;
        auto executor = jsi::Function::createFromHostFunction(r,
            jsi::PropNameID::forAscii(r, "executor"), 2,
            [state, id](jsi::Runtime& r, const jsi::Value&, const jsi::Value* args, size_t count) {
              state->require(r);
              if (count < 2) throw jsi::JSError(r, "promise executor needs resolve and reject");
              state->pending.emplace(id, State::Pending{
                  args[0].getObject(r).getFunction(r), args[1].getObject(r).getFunction(r)});
              return jsi::Value::undefined();
            });
        auto ctor = r.global().getPropertyAsFunction(r, "Promise");
        auto promise = ctor.callAsConstructor(r, executor);
        if (ibex2_async_begin(state->queue, authority.get(), op, abi.data(), abi.size(), id) != 0) {
          state->pending.erase(id);
          throw jsi::JSError(r, "could not start the async operation");
        }
        if (op == 113 || op == 116) return filesystem_promise(r, std::move(promise), op);
        return promise;
      });
}

jsi::Function Adapter::fetch(const void* grants) {
  if (!runtime_) throw std::logic_error("Ibex2 bindings are detached");
  auto& rt = *runtime_;
  state_->require(rt);
  if (!has(state_->groups, GROUP_FETCH) || !state_->fetch_factory.isObject() ||
      !state_->fetch_factory.getObject(rt).isFunction(rt))
    throw std::logic_error("Ibex2 FETCH group is not installed");
  auto raw = async_binding("fetch", 101, grants);
  return state_->fetch_factory.getObject(rt).getFunction(rt)
      .call(rt, raw).getObject(rt).getFunction(rt);
}

jsi::Function Adapter::websocket(const void* grants) {
  if (!runtime_) throw std::logic_error("Ibex2 bindings are detached");
  auto& rt = *runtime_;
  state_->require(rt);
  if (!has(state_->groups, GROUP_WEBSOCKET) ||
      !state_->websocket_factory.isObject() ||
      !state_->websocket_factory.getObject(rt).isFunction(rt) ||
      !state_->event_listener_change_hook.isObject() ||
      !state_->event_listener_change_hook.getObject(rt).isFunction(rt))
    throw std::logic_error("Ibex2 WEBSOCKET group is not installed");
  return state_->websocket_factory.getObject(rt).getFunction(rt)
      .call(rt, websocket_hooks(grants), state_->event_listener_change_hook)
      .getObject(rt).getFunction(rt);
}

jsi::Object Adapter::storage(const void* grants) {
  if (!runtime_) throw std::logic_error("Ibex2 bindings are detached");
  auto& rt = *runtime_;
  state_->require(rt);
  if (!has(state_->groups, GROUP_STORAGE) || !state_->sqlite_factory.isObject() ||
      !state_->sqlite_factory.getObject(rt).isFunction(rt))
    throw std::logic_error("Ibex2 STORAGE group is not installed");
  return storage(grants, state_->sqlite_factory.getObject(rt).getFunction(rt));
}

namespace {
struct SqliteOwner final : jsi::NativeState {
  void* owner;
  explicit SqliteOwner(void* value) : owner(value) {}
  ~SqliteOwner() override { ibex2_sqlite_owner_destroy(owner); }
};
void freeze(jsi::Runtime& rt, const jsi::Object& value) {
  rt.global().getPropertyAsObject(rt, "Object").getPropertyAsFunction(rt, "freeze").call(rt, value);
}
}

jsi::Object Adapter::storage(const void* grants, const jsi::Function& factory) {
  if (!runtime_) throw std::logic_error("Ibex2 bindings are detached");
  auto& rt = *runtime_;
  state_->require(rt);
  struct Method { const char* name; uint32_t op; };
  static const Method fs_methods[] = {
      {"readFile",110}, {"writeFile",111}, {"appendFile",112}, {"readdir",113},
      {"mkdir",114}, {"rm",115}, {"stat",116}, {"rename",117}, {"copyFile",118},
      {"realpath",119}, {"atomicWriteFile",120}};
  jsi::Object fs(rt);
  for (const auto& method : fs_methods)
    fs.setProperty(rt, method.name, async_binding(method.name, method.op, grants));
  jsi::Object directories(rt);
  directories.setProperty(rt, "data", "app:/data");
  directories.setProperty(rt, "cache", "app:/cache");
  directories.setProperty(rt, "temporary", "app:/tmp");
  freeze(rt, directories);
  fs.setProperty(rt, "directories", directories);
  freeze(rt, fs);

  auto state = state_;
  auto field = jsi::Function::createFromHostFunction(rt,
      jsi::PropNameID::forAscii(rt, "sqliteResult"), 1,
      [state](jsi::Runtime& r, const jsi::Value&, const jsi::Value* args, size_t count) {
        state->require(r);
        return call_host(r, state->queue, 80, args, count);
      });
  auto retain = jsi::Function::createFromHostFunction(rt,
      jsi::PropNameID::forAscii(rt, "sqliteOwn"), 3,
      [state](jsi::Runtime& r, const jsi::Value&, const jsi::Value* args, size_t count) {
        state->require(r);
        if (count != 3 || !args[0].isNumber() || !args[1].isNumber() || !args[2].isObject())
          throw jsi::JSError(r, "SQLite owner needs a handle, kind, and object");
        args[2].getObject(r).setNativeState(r, std::make_shared<SqliteOwner>(
            ibex2_sqlite_owner_create(state->queue, args[0].asNumber(), static_cast<int>(args[1].asNumber()))));
        return jsi::Value::undefined();
      });
  auto make_sqlite = factory.call(rt, field, retain).getObject(rt).getFunction(rt);
  static const Method sqlite_methods[] = {
      {"open",150}, {"prepare",151}, {"execute",152}, {"query",153},
      {"statementExecute",154}, {"statementQuery",155}, {"transaction",156},
      {"close",157}, {"statementClose",158}};
  jsi::Object raw(rt);
  for (const auto& method : sqlite_methods)
    raw.setProperty(rt, method.name, async_binding(method.name, method.op, grants));
  jsi::Object result(rt);
  result.setProperty(rt, "fs", fs);
  result.setProperty(rt, "sqlite", make_sqlite.call(rt, raw));
  freeze(rt, result);
  return result;
}

void Adapter::settle(uint64_t id, Ibex2AbiValue& value, bool is_error) {
  struct Release { Ibex2AbiValue& value; ~Release() { ibex2_host_release(&value); } } release{value};
  auto found = state_->pending.find(id);
  if (!state_->alive || found == state_->pending.end()) return;
  auto promise = std::move(found->second);
  state_->pending.erase(found);
  auto& rt = *runtime_;
  auto payload = from_abi(rt, value);
  if (is_error) {
    auto error = rt.global().getPropertyAsFunction(rt, "Error").callAsConstructor(rt, payload);
    promise.reject.call(rt, error);
  } else promise.resolve.call(rt, payload);
}

uint64_t Adapter::subscribe(jsi::Function callback) {
  if (!runtime_) throw std::logic_error("Ibex2 bindings are detached");
  state_->require(*runtime_);
  uint64_t id = 0;
  void* rust = ibex2_subscription_create(state_->queue, &id);
  if (rust == nullptr)
    throw std::runtime_error("could not create Ibex2 event subscription");
  try {
    state_->subscriptions.emplace(
        id, State::EventSubscription{
                rust, std::move(callback), 0, nullptr,
                jsi::Value::undefined(), false, {}});
  } catch (...) {
    ibex2_subscription_destroy(rust);
    throw;
  }
  return id;
}

void Adapter::unsubscribe(uint64_t id) {
  auto found = state_->subscriptions.find(id);
  if (found == state_->subscriptions.end()) return;
  // Rust cancellation is the barrier for the host-task FIFO; only after it
  // returns is the owner-thread JSI root released.
  if (auto owner = found->second.native_owner.lock())
    owner->unsubscribe_events();
  else
    ibex2_subscription_destroy(found->second.rust);
  state_->subscriptions.erase(found);
}

void Adapter::refresh_websocket_keepalives() {
  if (!runtime_ || !state_->alive) return;
  auto& rt = *runtime_;
  for (auto found = state_->subscriptions.begin();
       found != state_->subscriptions.end();) {
    auto& subscription = found->second;
    if (subscription.websocket == 0 || subscription.weak_owner == nullptr) {
      ++found;
      continue;
    }
    auto native = subscription.native_owner.lock();
    auto owner = subscription.weak_owner->lock(rt);
    if (!native || !owner.isObject()) {
      found = state_->subscriptions.erase(found);
      continue;
    }
    auto has_listener = [&](const char* type) {
      if (!state_->event_listener_query.isObject() ||
          !state_->event_listener_query.getObject(rt).isFunction(rt))
        return false;
      auto result = state_->event_listener_query.getObject(rt)
          .getFunction(rt).call(
              rt, owner, jsi::String::createFromAscii(rt, type));
      return result.isBool() && result.getBool();
    };
    int ready = ibex2_websocket_ready_state(state_->queue,
                                            subscription.websocket);
    subscription.listener_keepalive = ready == 0
        ? has_listener("open") || has_listener("message") ||
              has_listener("error") || has_listener("close")
        : ready == 1
        ? has_listener("message") || has_listener("error") ||
              has_listener("close")
        : has_listener("error") || has_listener("close");
    bool keep = subscription.listener_keepalive ||
        (ready == 1 &&
         ibex2_websocket_buffered_amount(state_->queue,
                                         subscription.websocket) != 0);
    subscription.strong_owner = keep
        ? jsi::Value(rt, owner) : jsi::Value::undefined();
    ++found;
  }
}

void Adapter::prepare_garbage_collection() {
  refresh_websocket_keepalives();
}

size_t Adapter::websocket_keepalive_count_for_test() const {
  size_t count = 0;
  for (const auto& entry : state_->subscriptions) {
    if (entry.second.websocket != 0 && entry.second.strong_owner.isObject())
      ++count;
  }
  return count;
}

void Adapter::deliver_event(uint64_t id, Ibex2AbiValue& value) {
  struct Release {
    Ibex2AbiValue& value;
    ~Release() { ibex2_host_release(&value); }
  } release{value};
  auto found = state_->subscriptions.find(id);
  if (!state_->alive || found == state_->subscriptions.end()) return;
  bool terminal = found->second.websocket != 0 &&
      value.tag == IBEX2_TAG_BYTES && value.data != nullptr &&
      value.len > 0 && value.data[0] == 4;
  auto payload = from_abi(*runtime_, value);
  jsi::Value owner = jsi::Value::undefined();
  if (found->second.websocket != 0) {
    if (found->second.strong_owner.isObject())
      owner = jsi::Value(*runtime_, found->second.strong_owner);
    else if (found->second.weak_owner != nullptr)
      owner = found->second.weak_owner->lock(*runtime_);
    if (!owner.isObject()) {
      unsubscribe(id);
      return;
    }
  }
  try {
    if (found->second.websocket != 0)
      found->second.callback.call(*runtime_, owner, payload);
    else
      found->second.callback.call(*runtime_, payload);
  } catch (...) {
    if (terminal) unsubscribe(id);
    throw;
  }
  if (terminal) unsubscribe(id);
  else refresh_websocket_keepalives();
}

void Adapter::report_error(const jsi::Value& error) {
  if (!runtime_) throw std::logic_error("Ibex2 bindings are detached");
  auto& rt = *runtime_;
  if (state_->event_reporter.isObject() &&
      state_->event_reporter.getObject(rt).isFunction(rt)) {
    state_->event_reporter.getObject(rt).getFunction(rt).call(rt, error);
    return;
  }
  auto message = error.toString(rt).utf8(rt);
  ibex2_report_uncaught(message.c_str());
}

void Adapter::report_error(const char* message) {
  if (!runtime_) throw std::logic_error("Ibex2 bindings are detached");
  auto& rt = *runtime_;
  if (state_->event_reporter.isObject() &&
      state_->event_reporter.getObject(rt).isFunction(rt)) {
    auto error = rt.global().getPropertyAsFunction(rt, "Error")
        .callAsConstructor(rt, message == nullptr ? "uncaught error" : message);
    state_->event_reporter.getObject(rt).getFunction(rt).call(rt, error);
    return;
  }
  ibex2_report_uncaught(message);
}

bool Adapter::deliver_one() {
  if (!state_->alive) return false;
  int kind = 0, is_error = 0;
  unsigned long long id = 0;
  Ibex2AbiValue value{IBEX2_TAG_UNDEFINED, 0, nullptr, 0};
  if (!ibex2_take_task(state_->queue, &kind, &id, &value, &is_error)) return false;
  if (kind != 1 && kind != 3) {
    ibex2_host_release(&value);
    throw jsi::JSError(*runtime_, "storage adapter received a non-settlement task");
  }
  // @ref LLP 0058.000.000#8-tasks-microtasks-timers-and-callbacks — borrowed-adapter task failures are reported like owning-pump failures
  try {
    if (kind == 3)
      deliver_event(id, value);
    else
      settle(id, value, is_error != 0);
  } catch (const jsi::JSError& error) {
    try {
      report_error(error.value());
    } catch (...) {
      std::string message = error.getMessage();
      const std::string& stack = error.getStack();
      if (!stack.empty()) {
        message += "\n";
        message += stack;
      }
      ibex2_report_uncaught(message.c_str());
    }
  } catch (const std::exception& error) {
    try {
      report_error(error.what());
    } catch (...) {
      ibex2_report_uncaught(error.what());
    }
  } catch (...) {
    try {
      report_error("uncaught native callback exception");
    } catch (...) {
      ibex2_report_uncaught("uncaught native callback exception");
    }
  }
  return true;
}
} // namespace ibex2::jsi_adapter
