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
inline constexpr Groups GROUP_EVENTS = 1u << 11;
inline constexpr Groups GROUP_BLOB = 1u << 12;
inline constexpr Groups GROUP_WEBSOCKET = 1u << 13;

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

/// Explicit trusted-bootstrap controls for one binding installation.
///
/// `fetch_primitives`, when set, names the global under which installation
/// publishes one frozen object. The name must be an ASCII JavaScript identifier,
/// `[A-Za-z_$][A-Za-z0-9_$]*`, that is not already a property of the global
/// object or its prototype chain and that no installed binding takes; anything
/// else is refused before the runtime is touched (an installed-binding collision
/// is found after installation and spends the runtime). The option requires the
/// GROUP_FETCH group.
///
/// `abort_hooks`, when set, names a global with the same name rules and
/// collisions. It requires GROUP_ABORT and publishes the frozen hook object
/// created by the abort binding, with exactly `{ own, subscribe }`. `own(signal)`
/// returns a frozen, null-prototype live view with only read-only `aborted` and
/// `reason` getters for an Ibex AbortSignal and throws a TypeError for any other
/// value. The view retains private state and is capability-equivalent to `own`
/// for reading that signal. The harden reachability walk does not recognize or
/// track views returned from `own`; trusted embedder code may read one but must
/// never return, publish, or attach it to an application-reachable object.
/// `subscribe(signal, callback[, alive])`
/// registers an abort algorithm, returns an idempotent unsubscribe function,
/// and invokes `callback` synchronously before the public `abort` event is
/// dispatched. If the signal is already aborted it invokes `callback`
/// synchronously and returns a no-op unsubscribe function. The optional `alive`
/// predicate is checked before every delivery, including that already-aborted
/// path, and lets an internal consumer discard a stale hook. An exception from
/// either `alive` or `callback` is reported and suppresses only that hook; later
/// hooks and public abort dispatch still run. Because abort algorithms run
/// before event dispatch, an application listener's
/// `stopImmediatePropagation()` cannot suppress them.
///
/// `defer_intrinsic_snapshot`, when true, makes installation discard the
/// constructor-time SQLite integrity baseline after all selected binding
/// scripts have installed. The trusted embedder must run its prelude and then
/// call `Adapter::capture_intrinsics()` exactly once before hardening. Hardening
/// and SQLite operations refuse while that baseline is absent. The default is
/// false and preserves construction-time capture. Capture is an unconditional
/// snapshot of the realm as it stands: everything the embedder runs first is
/// trusted as part of the baseline. The embedder must run only its trusted
/// prelude in this interval, without evaluating application code or pumping a
/// microtask/timer source that can run it. The API cannot detect that provenance.
///
/// CONTRACT: trusted embedder bootstrap only. Application code must never reach
/// either object or any member. Between installation and hardening, and before
/// any application code, the bootstrap captures each requested object, deletes
/// its global, and wraps the members in closures it publishes instead. It then
/// hardens through `Adapter::harden()`, never by evaluating HARDEN_SOURCE
/// directly. That refuses (and freezes nothing) while a chosen global is still
/// present or while a published object or member is reachable from
/// what the freeze walks: the global object's own string- and symbol-keyed
/// properties, its prototype chain, and transitively each reached object's own
/// data values, accessor functions (never invoked), and prototype. Values held
/// only in closures, native state, collection entries, or behind a concealing
/// Proxy are outside that walk, so a wrapper must never hand `p` or a member to
/// application code.
///
/// Members (synchronous unless a Promise is named):
///
///   fetch(url, method, body, redirect, headersHandle, controlToken)
///       -> Promise<responseHandle>
///   responseField(responseHandle, fieldId[, headerName]) -> value
///   responseRead(responseHandle) -> Promise<ArrayBuffer | null>
///   fetchControl(action[, token]) -> token | undefined
///   textEncode(string) -> ArrayBuffer
///   textDecode(bytes[, fatal[, ignoreBOM]]) -> string
///   textEncodeInto(string, Uint8Array) -> "read,written"
///   headersFree(headersHandle) -> undefined
///
/// `fetch` is async op 101 bound to the installation's endowment, so it has the
/// same `net.fetch` grants as the ordinary installed fetch. `url` is a string;
/// `method` a string or undefined (undefined or "" means GET; it is uppercased);
/// `body` undefined or null for none, or an ArrayBuffer or typed array whose
/// bytes are sent (a string must be encoded first). `redirect` is a string or
/// undefined: "manual" resolves with the 3xx response itself; "error" rejects on
/// a 3xx; anything else, undefined included, follows up to 20 redirects inside
/// the transport, admitting every hop against the same grants (an ungranted hop
/// rejects with "denied: net.fetch"), turning 303 and a non-GET/HEAD 301/302
/// into a bodiless GET, and dropping authorization, cookie, and
/// proxy-authorization on a cross-origin hop. `headersHandle` is undefined or a
/// live Headers registry handle (a standard `Headers` object's `_handle`). The
/// complete list is validated and copied before `fetch` returns; its worker uses
/// only that owned snapshot, so collecting the wrapper while the job is queued
/// cannot invalidate the request. The accepted handle belongs to this object's
/// handle domain: release it with `headersFree` exactly once after the returned
/// promise settles, even when a later argument check throws (release remains
/// valid when wrapper collection already removed the registry row).
/// `controlToken` is undefined or a token from this object's `fetchControl(0)`.
/// The promise resolves to a response handle owned by this object, or rejects
/// with the host error text as message.
///
/// `responseField` field ids: 0 status (number), 1 ok (boolean), 2 final URL
/// (string), 3 one header value by `headerName`, a string (string, or null when
/// absent), 5 redirected (boolean), 7 every header as a JSON string of
/// `[name, value]` pairs sorted by name, 8 cancel: abort the body, drop the row,
/// release its control, and stop accepting the handle (returns undefined). Any
/// other id is a RangeError.
///
/// `responseRead` reads one chunk of 1 to 16384 bytes as an ArrayBuffer and
/// resolves null at end of body; await each read before issuing the next. Null
/// or a rejection (transport error or abort) is terminal: the row is dropped and
/// the handle is no longer accepted.
///
/// `fetchControl` actions: 0 allocates and returns a new token; 1 aborts the
/// request or body read using `token`; 2 releases `token`, which is then no
/// longer accepted. Any other action is a RangeError. Release every token once,
/// even after its response has finished.
///
/// The text members are UTF-8 operations 20-22 with the semantics of the
/// standard binding scripts; `headersFree` is op 51.
///
/// Ownership. Each published object is its own handle domain: it accepts only
/// response handles its `fetch` resolved, tokens its `fetchControl(0)`
/// allocated, and headers handles its `fetch` accepted, and only until they are
/// released. A handle from ordinary `fetch` or `Headers`, from another object, or
/// already released is a TypeError even when the id is live. Numeric arguments
/// are validated before conversion: a non-number is a TypeError; NaN, an
/// infinity, a fraction, or an out-of-range value is a RangeError (handles and
/// tokens 1 to 2^53 - 1, field ids 0 to 2^32 - 1, actions 0 to 2). Primitive
/// response rows have no garbage-collection owner: finish every response handle
/// with a null read, a rejected read, or field 8, or its row and connection stay
/// open until the runtime is destroyed.
///
/// Lifecycle (bootstrap source, evaluated after installation):
///
///   globalThis.embedderFetch = (function (p) {
///     delete globalThis.__embedder_fetch_primitives;   // capture, then delete
///     return function embedderFetch(url) {             // wrap: only closures hold p
///       var token = p.fetchControl(0);
///       return p.fetch(String(url), "GET", undefined, "follow", undefined, token)
///         .then(function (handle) {
///           var status = p.responseField(handle, 0);
///           p.responseField(handle, 8);                // done: cancel, drop the row
///           p.fetchControl(2, token);
///           return status;
///         }, function (error) { p.fetchControl(2, token); throw error; });
///     };
///   })(globalThis.__embedder_fetch_primitives);
///
/// then `Adapter::harden()` before any application code.
// @ref LLP 0068#opt-in-fetch-primitives-protocol — the normative L1e handoff protocol
struct InstallOptions {
  const char* fetch_primitives = nullptr;
  // @ref LLP 0068#opt-in-abort-hooks-protocol — abort algorithms are a trusted-bootstrap handoff, never an application global
  const char* abort_hooks = nullptr;
  // @ref LLP 0068#deferred-intrinsic-integrity-baseline — trusted preludes may establish the whole SQLite integrity baseline once
  bool defer_intrinsic_snapshot = false;
};

Ibex2AbiValue to_abi(jsi::Runtime&, const jsi::Value&, std::vector<std::string>&);
jsi::Value from_abi(jsi::Runtime&, Ibex2AbiValue&);
struct HostCallResult {
  int status;
  jsi::Value value;
};
HostCallResult call_host_result(jsi::Runtime&, const void*, uint32_t,
                                const jsi::Value*, size_t);

// One JavaScript wrapper may own at most one native resource. JSI's raw
// setNativeState overwrites and finalizes the old owner, so every attachment
// goes through this refusing form instead.
void set_native_state_once(jsi::Runtime&, const jsi::Object&,
                           std::shared_ptr<jsi::NativeState>, const char*);

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
// Construct before trusted bootstrap or application code. By default the
// constructor captures SQLite's intrinsic-integrity baseline. An install that
// opts into deferred capture must be followed by the trusted prelude,
// capture_intrinsics(), and Adapter::harden(), in that order. In every mode,
// harden before application code uses storage. SQLite refuses mutable or
// replaced intrinsics, including methods changed before a later freeze.
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
  // after Ibex's trusted bootstrap replaces that property. This suffices for a
  // named replacement but cannot admit a newly added property because the
  // construction-time key set is closed. Every other captured identity remains
  // anchored to runtime construction. A deferred complete baseline uses
  // capture_intrinsics() instead.
  void accept_trusted_intrinsic_property(jsi::Object, const char* name);
  // Complete a deferred intrinsic snapshot exactly once, after install_with
  // and the trusted embedder prelude but before hardening. This unconditionally
  // trusts the current realm; the API cannot distinguish prelude from app code.
  // Throws if deferral was not requested, installation is incomplete, capture
  // already ran, or Array.prototype is already frozen (including after direct
  // evaluation of HARDEN_SOURCE).
  // @ref LLP 0068#deferred-intrinsic-integrity-baseline — capture the trusted prelude's complete property sets and identities
  void capture_intrinsics();
  // Install exactly `groups`. `scripts` must be the compiled results of
  // bindings::scripts(groups), in that order, from the compiler belonging to
  // this runtime's engine. `bindings` is the opaque endowment made from
  // Host::endow and must carry the same runtime state given to this adapter.
  // Missing dependencies, wrong order, and a second install are refused. The
  // call neither drives nor waits on the runtime.
  // @ref LLP 0057.000#50-three-doors-one-implementation — door 2 installs into a caller-owned runtime and returns
  void install(Groups groups, const Ibex2Bindings* bindings,
               const CompiledScript* scripts, size_t script_count);
  // The same one-shot, atomic installation with explicitly requested trusted-
  // bootstrap outputs. Existing install() is exactly the empty-options case.
  // A caller that requests a trusted-bootstrap output must harden through
  // Adapter::harden() so its reachability guard runs before the freeze.
  // @ref LLP 0057.000#l1--the-bindings-door — L1e keeps fetch ownership with embedders without creating a second authority path
  void install_with(Groups groups, const Ibex2Bindings* bindings,
                    const CompiledScript* scripts, size_t script_count,
                    const InstallOptions& options);
  // Compatibility name for the complete trusted-bootstrap harden guard. A
  // no-op unless install_with published fetch primitives or abort hooks;
  // otherwise throws (std::runtime_error) if a chosen global is still present,
  // or if a published object or member is reachable from what HARDEN_SOURCE
  // freezes: the global
  // object's own string- and symbol-keyed properties, its prototype chain,
  // and transitively each reached object's own data values, accessor
  // functions (never invoked), and prototype. Values held only in closures,
  // native state, collection entries, or behind concealing Proxy traps are
  // not visible to this walk, exactly as they are not visible to the freeze.
  void verify_fetch_primitives_unreachable();
  // The complete trusted-bootstrap guard used by harden(): fetch primitives
  // and abort hooks, when requested. Prefer this over the compatibility-named
  // entry point above when checking explicitly; both run the complete guard.
  void verify_trusted_bootstrap_unreachable();
  // Every fail-closed check that must precede the freeze: a deferred intrinsic
  // snapshot has been captured, and trusted-bootstrap outputs are unreachable.
  void verify_harden_preconditions();
  // The post-install hardening step through the adapter: validates `script`
  // as this runtime's Hermes bytecode (the header checks install() applies),
  // runs verify_harden_preconditions(), and only then evaluates it.
  // `script` is HARDEN_SOURCE compiled by this engine's hermesc -- in Rust,
  // ibex2::bindings::HARDEN_BYTECODE (path: HARDEN_BYTECODE_PATH). A caller
  // that requested a trusted-bootstrap output through install_with MUST harden with
  // this method rather than evaluating HARDEN_SOURCE itself; evaluating it
  // directly skips the guard. For a plain install() it is equivalent to
  // evaluating the bytecode. Throws, and freezes nothing, on refusal.
  // @ref LLP 0068#opt-in-fetch-primitives-protocol — the bindings door gets the same harden guard as the owning runtime
  void harden(const CompiledScript& script);
  jsi::Function async_binding(const char* name, uint32_t op, const void* grants);
  // Endowed values built from the factories retained by install().
  jsi::Function fetch(const void* grants);
  jsi::Function websocket(const void* grants);
  jsi::Object storage(const void* grants);
  // sqlite_factory is the completion value of precompiled bindings/sqlite.js.
  // This returns frozen {fs, sqlite}; it never modifies the global object.
  jsi::Object storage(const void* grants, const jsi::Function& sqlite_factory);
  // Takes/releases the ABI payload even if no promise is awaiting this id.
  void settle(uint64_t task_id, Ibex2AbiValue&, bool is_error);
  // Register a JS callback for a future host-event source. The returned
  // identity is the one Rust carries in HostTask::Event.
  uint64_t subscribe(jsi::Function callback);
  // Removes the callback root and cancels admitted, unreserved events.
  void unsubscribe(uint64_t subscription);
  // Deliver one already-reserved event, taking/releasing its payload.
  void deliver_event(uint64_t subscription, Ibex2AbiValue&);
  // Route an uncaught callback/timer failure through the EVENTS error event;
  // without EVENTS these fall back to the host console reporter.
  void report_error(const jsi::Value& error);
  void report_error(const char* message);
  // Takes at most one storage settlement or subscribed event. No timers or
  // microtask checkpoints. Callback failures are reported through the EVENTS
  // error path and do not escape. Returns true if a task was delivered; throws
  // for a timer task, which belongs to an owning runtime's driver.
  bool deliver_one();
  // Release WebSocket keepalive roots whose listener/queued-data condition
  // ended before an embedder explicitly requests collection.
  void prepare_garbage_collection();
  // Test seam for proving listener mutations synchronously release roots,
  // before the pre-collection reconciliation above has a chance to help.
  size_t websocket_keepalive_count_for_test() const;
private:
  jsi::Object websocket_hooks(const void* grants);
  void refresh_websocket_keepalives();
  struct State;
  jsi::Runtime* runtime_;
  std::shared_ptr<State> state_;
};
} // namespace ibex2::jsi_adapter
