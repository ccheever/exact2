// Engine-independent JSI adapter; no Hermes ownership, loader or event loop.
// @ref LLP 0067#3-the-check — captured authority, one Rust boundary
#include "../../include/ibex2_jsi.h"
#if __has_include(<hermes/BCGen/HBC/BytecodeFileFormat.h>)
#include <hermes/BCGen/HBC/BytecodeFileFormat.h>
#define IBEX2_HAS_HERMES_BYTECODE_FILE_FORMAT 1
#endif
#include <cstring>
#include <unordered_map>
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

enum class InstallStatus { Fresh, Installed, Spent };

struct Adapter::State {
  struct Pending { jsi::Function resolve; jsi::Function reject; };
  const void* queue;
  uint64_t next_task_id = 1;
  std::unordered_map<uint64_t, Pending> pending;
  bool alive = true;
  InstallStatus install_status = InstallStatus::Fresh;
  Groups groups = 0;
  uint32_t bytecode_version;
  std::shared_ptr<Lifetime> lifetime;
  jsi::Value fetch_factory;
  jsi::Value sqlite_factory;
  std::unique_ptr<Integrity> integrity;
  State(jsi::Runtime& rt, const void* value, uint32_t version,
        std::shared_ptr<Lifetime> lifetime_value)
      : queue(value), bytecode_version(version),
        lifetime(std::move(lifetime_value)),
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
  if (!runtime_ || !state_->integrity)
    throw std::logic_error("Ibex2 bindings are detached");
  state_->integrity->accept_property(*runtime_, object, name);
}
void Adapter::detach() {
  if (!state_->alive) return;
  state_->alive = false;
  state_->lifetime->detach();
  state_->pending.clear();
  state_->fetch_factory = jsi::Value::undefined();
  state_->sqlite_factory = jsi::Value::undefined();
  state_->integrity.reset();
  state_->queue = nullptr;
  runtime_ = nullptr;
}

namespace {
void freeze(jsi::Runtime&, const jsi::Object&);

constexpr Groups kKnownGroups = GROUP_PURE | GROUP_CONSOLE | GROUP_TIMERS |
    GROUP_ABORT | GROUP_CRYPTO | GROUP_FETCH | GROUP_STORAGE | GROUP_ENV |
    GROUP_SECRETS | GROUP_KV | GROUP_INTL;

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
  };
  for (const auto& requirement : requirements) {
    if (has(groups, requirement.group) && !has(groups, requirement.required))
      throw std::invalid_argument("Ibex2 binding group is missing a dependency");
  }
#if !defined(IBEX2_JSI_HAS_INTL)
  if (has(groups, GROUP_INTL))
    throw std::invalid_argument("Ibex2 INTL bindings are unavailable in this build");
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
  if (has(groups, GROUP_ABORT)) result.push_back("abort");
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
  auto field = jsi::Function::createFromHostFunction(rt,
      jsi::PropNameID::forAscii(rt, "__ibex2_response_field"), 3,
      [lifetime](jsi::Runtime& r, const jsi::Value&,
                 const jsi::Value* args, size_t count) -> jsi::Value {
        const void* queue = lifetime->require(r);
        if (count < 2)
          throw jsi::JSError(r, "response field needs a handle and a field id");
        std::vector<std::string> owned;
        Ibex2AbiValue name{IBEX2_TAG_UNDEFINED, 0.0, nullptr, 0};
        if (count >= 3) name = to_abi(r, args[2], owned);
        Ibex2AbiValue out{IBEX2_TAG_UNDEFINED, 0.0, nullptr, 0};
        int status = ibex2_response_field(
            queue, args[0].asNumber(), static_cast<uint32_t>(args[1].asNumber()),
            count >= 3 ? &name : nullptr, &out);
        struct Release {
          Ibex2AbiValue& value;
          ~Release() { ibex2_host_release(&value); }
        } release{out};
        auto result = from_abi(r, out);
        if (status != 0)
          throw jsi::JSError(r, result.isString()
              ? result.getString(r).utf8(r) : std::string("response read failed"));
        return result;
      });
  global.setProperty(rt, "__ibex2_response_field", std::move(field));
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
} // namespace

void validate_groups(Groups groups) { validate_groups_impl(groups); }

std::vector<const char*> expected_scripts(Groups groups) {
  validate_groups_impl(groups);
  return expected_scripts_impl(groups);
}

void Adapter::install(Groups groups, const Ibex2Bindings* bindings,
                      const CompiledScript* scripts, size_t script_count) {
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
    auto expected = expected_scripts(groups);
    if (script_count != expected.size() || (script_count != 0 && scripts == nullptr))
      throw std::invalid_argument("Ibex2 binding bytecode count does not match groups");
    for (size_t i = 0; i < script_count; ++i) {
      if (scripts[i].name == nullptr || scripts[i].bytes == nullptr ||
          std::strcmp(scripts[i].name, expected[i]) != 0)
        throw std::invalid_argument("Ibex2 binding bytecode is not in scripts() order");
      if (scripts[i].len < kHermesBytecodeHeaderSize)
        throw std::invalid_argument(
            "Ibex2 binding payload has a truncated Hermes bytecode header");
      if (std::memcmp(scripts[i].bytes, kHermesBytecodeMagic,
                      sizeof(kHermesBytecodeMagic)) != 0)
        throw std::invalid_argument("Ibex2 binding payload is not Hermes bytecode");
      if (bytecode_declared_length(scripts[i]) != scripts[i].len)
        throw std::invalid_argument(
            "Ibex2 binding bytecode declared length does not match its buffer");
      if (state_->bytecode_version != 0 &&
          bytecode_version(scripts[i]) != state_->bytecode_version)
        throw std::invalid_argument(
            "Ibex2 binding bytecode version does not match the runtime");
    }

    // Validation above is deliberately complete before the first host function
    // or JavaScript global is installed: one bad payload refuses the whole door.
    mutation_started = true;
    if (has(groups, GROUP_CONSOLE)) install_console(rt, state_->lifetime);
    if (has(groups, GROUP_PURE)) install_pure(rt, state_->lifetime);
    if (has(groups, GROUP_TIMERS)) install_timers(rt, state_->lifetime);
    if (has(groups, GROUP_CRYPTO)) install_crypto(rt, state_->lifetime);
    if (has(groups, GROUP_FETCH)) install_fetch(rt, *this, state_->lifetime);
#if defined(IBEX2_JSI_HAS_INTL)
    if (has(groups, GROUP_INTL)) {
      ibex2::intl_number_format::install(rt, state_->lifetime);
      ibex2::intl_case::install(rt, state_->lifetime);
    }
#endif

    for (size_t i = 0; i < script_count; ++i) {
      const auto& script = scripts[i];
      auto buffer = std::make_shared<CompiledBytes>(script.bytes, script.len);
      auto value = rt.evaluateJavaScript(buffer, std::string(script.name) + ".js");
      if (std::strcmp(script.name, "fetch") == 0) {
        if (!value.isObject() || !value.getObject(rt).isFunction(rt))
          throw jsi::JSError(rt, "fetch binding did not evaluate to a factory");
        state_->fetch_factory = jsi::Value(rt, value);
        continue;
      }
      if (std::strcmp(script.name, "sqlite") == 0) {
        if (!value.isObject() || !value.getObject(rt).isFunction(rt))
          throw jsi::JSError(rt, "SQLite binding did not evaluate to a factory");
        state_->sqlite_factory = jsi::Value(rt, value);
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
    if (has(groups, GROUP_ABORT) && !has(groups, GROUP_FETCH))
      remove_global(rt, global, "__ibex2_abort");
    if (has(groups, GROUP_STORAGE)) {
      auto storage_value = storage(grants);
      global.setProperty(rt, "fs", storage_value.getProperty(rt, "fs"));
      global.setProperty(rt, "sqlite", storage_value.getProperty(rt, "sqlite"));
    }
    if (has(groups, GROUP_ENV))
      global.setProperty(rt, "process", make_process(rt, grants));

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
        if (op == 150) state->integrity->require(r);
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

bool Adapter::deliver_one() {
  if (!state_->alive) return false;
  int kind = 0, is_error = 0;
  unsigned long long id = 0;
  Ibex2AbiValue value{IBEX2_TAG_UNDEFINED, 0, nullptr, 0};
  if (!ibex2_take_task(state_->queue, &kind, &id, &value, &is_error)) return false;
  if (kind != 1) {
    ibex2_host_release(&value);
    throw jsi::JSError(*runtime_, "storage adapter received a non-settlement task");
  }
  settle(id, value, is_error != 0);
  return true;
}
} // namespace ibex2::jsi_adapter
