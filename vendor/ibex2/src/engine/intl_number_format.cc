// Public-JSI object plumbing for the Rust-owned Linux NumberFormat.
//
// No formatter handle is placed on a JavaScript object. A JSI NativeState
// holds the Rust owner, and every semantic operation still crosses the one
// primitive/handle host-call boundary.

// @ref LLP 0057#3-the-boundary — JavaScript owns object shape; Rust owns semantics
// @ref LLP 0067#3-the-check — private bootstrap bindings disappear before hardening

#include "../../include/ibex2_jsi.h"

#include <cstddef>
#include <cstdint>
#include <memory>
#include <limits>
#include <string>
#include <vector>

using namespace facebook;
using namespace ibex2::jsi_adapter;

extern "C" void *ibex2_intl_owner_create(const void *, double);
extern "C" void ibex2_intl_owner_destroy(void *);
extern "C" double ibex2_intl_owner_handle(const void *);
extern "C" int ibex2_host_call(const void *, uint32_t,
                                const Ibex2AbiValue *, size_t,
                                Ibex2AbiValue *);
extern "C" void ibex2_host_release(Ibex2AbiValue *);

namespace ibex2::intl_number_format {
namespace {

constexpr uint32_t kCreate = 90;
constexpr uint32_t kFormat = 91;
constexpr uint32_t kFormatParts = 92;
constexpr uint32_t kPartType = 93;
constexpr uint32_t kPartValue = 94;
constexpr uint32_t kResolved = 95;
constexpr uint32_t kSupportedLocales = 96;
constexpr uint32_t kCurrencyDigits = 98;

struct Owner final : jsi::NativeState {
  void *value;
  explicit Owner(void *owner) : value(owner) {}
  ~Owner() override { ibex2_intl_owner_destroy(value); }
};

[[noreturn]] void throw_host_error(jsi::Runtime &rt, const jsi::Value &value) {
  std::string message = value.isString()
      ? value.getString(rt).utf8(rt)
      : std::string("Intl native operation failed");
  const char *constructor = "Error";
  for (const char *kind : {"RangeError: ", "TypeError: "}) {
    const std::string prefix(kind);
    if (message.rfind(prefix, 0) == 0) {
      constructor = prefix.substr(0, prefix.size() - 2) == "RangeError"
          ? "RangeError"
          : "TypeError";
      message.erase(0, prefix.size());
      break;
    }
  }
  auto error = rt.global()
                   .getPropertyAsFunction(rt, constructor)
                   .callAsConstructor(rt, message);
  throw jsi::JSError(rt, jsi::Value(rt, error));
}

[[noreturn]] void throw_type_error(jsi::Runtime &rt, const char *message) {
  auto error = rt.global()
                   .getPropertyAsFunction(rt, "TypeError")
                   .callAsConstructor(rt, message);
  throw jsi::JSError(rt, jsi::Value(rt, error));
}

jsi::Value call_checked(jsi::Runtime &rt, const void *state, uint32_t op,
                        const jsi::Value *args, size_t count) {
  auto result = call_host_result(rt, state, op, args, count);
  if (result.status != 0) throw_host_error(rt, result.value);
  return std::move(result.value);
}

std::shared_ptr<Owner> owner_from(jsi::Runtime &rt,
                                  const jsi::Value &value) {
  if (!value.isObject())
    throw_type_error(rt, "incompatible NumberFormat receiver");
  auto object = value.getObject(rt);
  if (!object.hasNativeState<Owner>(rt))
    throw_type_error(rt, "incompatible NumberFormat receiver");
  return object.getNativeState<Owner>(rt);
}

jsi::Value handle_value(const std::shared_ptr<Owner> &owner) {
  return jsi::Value(ibex2_intl_owner_handle(owner->value));
}

jsi::Function host_function(
    jsi::Runtime &rt, const char *name, unsigned int length,
    jsi::HostFunctionType body) {
  return jsi::Function::createFromHostFunction(
      rt, jsi::PropNameID::forAscii(rt, name), length, std::move(body));
}

} // namespace

void install(jsi::Runtime &rt, const void *state) {
  jsi::Object raw(rt);

  raw.setProperty(
      rt, "initialize",
      host_function(
          rt, "initialize", 1,
          [state](jsi::Runtime &r, const jsi::Value &,
                  const jsi::Value *args, size_t count) -> jsi::Value {
            if (count != 19 || !args[0].isObject())
              throw jsi::JSError(r, "Intl.NumberFormat initialization failed");
            auto handle = call_checked(r, state, kCreate, args + 1, count - 1);
            if (!handle.isNumber())
              throw jsi::JSError(r, "Intl.NumberFormat returned no native handle");
            auto owner = std::make_shared<Owner>(
                ibex2_intl_owner_create(state, handle.asNumber()));
            if (owner->value == nullptr)
              throw jsi::JSError(r, "Intl.NumberFormat could not own its native handle");
            args[0].getObject(r).setNativeState(r, std::move(owner));
            return jsi::Value::undefined();
          }));

  raw.setProperty(
      rt, "assertReceiver",
      host_function(
          rt, "assertReceiver", 1,
          [](jsi::Runtime &r, const jsi::Value &,
             const jsi::Value *args, size_t count) -> jsi::Value {
            if (count != 1) throw jsi::JSError(r, "NumberFormat receiver expected");
            owner_from(r, args[0]);
            return jsi::Value::undefined();
          }));

  raw.setProperty(
      rt, "currencyDigits",
      host_function(
          rt, "currencyDigits", 1,
          [state](jsi::Runtime &r, const jsi::Value &,
                  const jsi::Value *args, size_t count) -> jsi::Value {
            if (count != 1) throw jsi::JSError(r, "currency code expected");
            return call_checked(r, state, kCurrencyDigits, args, count);
          }));

  raw.setProperty(
      rt, "format",
      host_function(
          rt, "format", 3,
          [state](jsi::Runtime &r, const jsi::Value &,
                  const jsi::Value *args, size_t count) -> jsi::Value {
            if (count != 3)
              throw jsi::JSError(r, "Intl.NumberFormat format needs a receiver and value");
            auto owner = owner_from(r, args[0]);
            std::vector<jsi::Value> input;
            input.emplace_back(handle_value(owner));
            input.emplace_back(r, args[1]);
            input.emplace_back(r, args[2]);
            return call_checked(r, state, kFormat, input.data(), input.size());
          }));

  raw.setProperty(
      rt, "formatToParts",
      host_function(
          rt, "formatToParts", 3,
          [state](jsi::Runtime &r, const jsi::Value &,
                  const jsi::Value *args, size_t count) -> jsi::Value {
            if (count != 3)
              throw jsi::JSError(r, "formatToParts needs a receiver and value");
            auto owner = owner_from(r, args[0]);
            std::vector<jsi::Value> input;
            input.emplace_back(handle_value(owner));
            input.emplace_back(r, args[1]);
            input.emplace_back(r, args[2]);
            auto length = call_checked(r, state, kFormatParts, input.data(), input.size());
            if (!length.isNumber())
              throw jsi::JSError(r, "formatToParts returned no part count");
            const size_t count_parts = static_cast<size_t>(length.asNumber());
            jsi::Array result(r, count_parts);
            for (size_t i = 0; i < count_parts; ++i) {
              jsi::Value field_args[] = {
                  handle_value(owner), jsi::Value(static_cast<double>(i))};
              auto type = call_checked(r, state, kPartType, field_args, 2);
              auto value = call_checked(r, state, kPartValue, field_args, 2);
              jsi::Object part(r);
              part.setProperty(r, "type", std::move(type));
              part.setProperty(r, "value", std::move(value));
              result.setValueAtIndex(r, i, std::move(part));
            }
            return result;
          }));

  raw.setProperty(
      rt, "resolvedOptions",
      host_function(
          rt, "resolvedOptions", 1,
          [state](jsi::Runtime &r, const jsi::Value &,
                  const jsi::Value *args, size_t count) -> jsi::Value {
            if (count != 1)
              throw jsi::JSError(r, "resolvedOptions needs a receiver");
            auto owner = owner_from(r, args[0]);
            static const char *names[] = {
                "locale", "numberingSystem", "style", "currency",
                "currencyDisplay", "currencySign", "unit", "unitDisplay",
                "minimumIntegerDigits", "minimumFractionDigits",
                "maximumFractionDigits", "minimumSignificantDigits",
                "maximumSignificantDigits", "useGrouping", "notation",
                "compactDisplay", "signDisplay"};
            jsi::Object result(r);
            for (size_t i = 0; i < sizeof(names) / sizeof(names[0]); ++i) {
              jsi::Value field_args[] = {
                  handle_value(owner), jsi::Value(static_cast<double>(i))};
              auto value = call_checked(r, state, kResolved, field_args, 2);
              if (!value.isUndefined())
                result.setProperty(r, names[i], std::move(value));
            }
            return result;
          }));

  raw.setProperty(
      rt, "supportedLocalesOf",
      host_function(
          rt, "supportedLocalesOf", 2,
          [state](jsi::Runtime &r, const jsi::Value &,
                  const jsi::Value *args, size_t count) -> jsi::Value {
            if (count != 2)
              throw jsi::JSError(r, "supportedLocalesOf needs locales and matcher");
            return call_checked(r, state, kSupportedLocales, args, count);
          }));

  rt.global().setProperty(rt, "__ibex2_intl_number_format", std::move(raw));
}

} // namespace ibex2::intl_number_format

namespace ibex2::intl_datetime {
namespace {

constexpr uint32_t kCreate = 130;
constexpr uint32_t kFormat = 131;
constexpr uint32_t kFormatParts = 132;
constexpr uint32_t kPartType = 133;
constexpr uint32_t kPartValue = 134;
constexpr uint32_t kResolved = 135;
constexpr uint32_t kSupportedLocales = 136;
constexpr uint32_t kCanonicalTimeZone = 137;

extern "C" void *ibex2_intl_datetime_owner_create(const void *, double);
extern "C" void ibex2_intl_datetime_owner_destroy(void *);
extern "C" double ibex2_intl_datetime_owner_handle(const void *);

struct Owner final : jsi::NativeState {
  void *value;
  explicit Owner(void *owner) : value(owner) {}
  ~Owner() override { ibex2_intl_datetime_owner_destroy(value); }
};

[[noreturn]] void throw_host_error(jsi::Runtime &rt,
                                   const jsi::Value &value) {
  std::string message = value.isString()
      ? value.getString(rt).utf8(rt)
      : std::string("Intl.DateTimeFormat native operation failed");
  const char *constructor = "Error";
  for (const char *kind : {"RangeError: ", "TypeError: "}) {
    const std::string prefix(kind);
    if (message.rfind(prefix, 0) == 0) {
      constructor = prefix[0] == 'R' ? "RangeError" : "TypeError";
      message.erase(0, prefix.size());
      break;
    }
  }
  auto error = rt.global()
                   .getPropertyAsFunction(rt, constructor)
                   .callAsConstructor(rt, message);
  throw jsi::JSError(rt, jsi::Value(rt, error));
}

[[noreturn]] void throw_type_error(jsi::Runtime &rt, const char *message) {
  auto error = rt.global()
                   .getPropertyAsFunction(rt, "TypeError")
                   .callAsConstructor(rt, message);
  throw jsi::JSError(rt, jsi::Value(rt, error));
}

jsi::Value call_checked(jsi::Runtime &rt, const void *state, uint32_t op,
                        const jsi::Value *args, size_t count) {
  auto result = call_host_result(rt, state, op, args, count);
  if (result.status != 0) throw_host_error(rt, result.value);
  return std::move(result.value);
}

std::shared_ptr<Owner> owner_from(jsi::Runtime &rt,
                                  const jsi::Value &value) {
  if (!value.isObject())
    throw_type_error(rt, "incompatible DateTimeFormat receiver");
  auto object = value.getObject(rt);
  if (!object.hasNativeState<Owner>(rt))
    throw_type_error(rt, "incompatible DateTimeFormat receiver");
  return object.getNativeState<Owner>(rt);
}

jsi::Value handle_value(const std::shared_ptr<Owner> &owner) {
  return jsi::Value(ibex2_intl_datetime_owner_handle(owner->value));
}

jsi::Function host_function(jsi::Runtime &rt, const char *name,
                            unsigned int length, jsi::HostFunctionType body) {
  return jsi::Function::createFromHostFunction(
      rt, jsi::PropNameID::forAscii(rt, name), length, std::move(body));
}

} // namespace

std::vector<jsi::Value> factory_arguments(jsi::Runtime &rt,
                                          const void *state) {
  std::vector<jsi::Value> result;
  result.reserve(8);

  result.emplace_back(host_function(
      rt, "createDateTimeFormat", 19,
      [state](jsi::Runtime &r, const jsi::Value &, const jsi::Value *args,
              size_t count) -> jsi::Value {
        if (count != 19)
          throw jsi::JSError(r, "DateTimeFormat initialization failed");
        auto handle = call_checked(r, state, kCreate, args, count);
        if (!handle.isNumber())
          throw jsi::JSError(r, "DateTimeFormat returned no native handle");
        auto owner = std::make_shared<Owner>(
            ibex2_intl_datetime_owner_create(state, handle.asNumber()));
        if (owner->value == nullptr)
          throw jsi::JSError(r, "DateTimeFormat could not own its native handle");
        jsi::Object object(r);
        object.setNativeState(r, std::move(owner));
        return object;
      }));

  result.emplace_back(host_function(
      rt, "formatDateTime", 2,
      [state](jsi::Runtime &r, const jsi::Value &, const jsi::Value *args,
              size_t count) -> jsi::Value {
        if (count != 2)
          throw jsi::JSError(r, "DateTimeFormat format needs an owner and value");
        auto owner = owner_from(r, args[0]);
        jsi::Value input[] = {handle_value(owner), jsi::Value(r, args[1])};
        return call_checked(r, state, kFormat, input, 2);
      }));

  result.emplace_back(host_function(
      rt, "dateTimeParts", 2,
      [state](jsi::Runtime &r, const jsi::Value &, const jsi::Value *args,
              size_t count) -> jsi::Value {
        if (count != 2)
          throw jsi::JSError(r, "DateTimeFormat parts needs an owner and value");
        auto owner = owner_from(r, args[0]);
        jsi::Value input[] = {handle_value(owner), jsi::Value(r, args[1])};
        return call_checked(r, state, kFormatParts, input, 2);
      }));

  auto part = [state](const char *name, uint32_t op) {
    return [state, name, op](jsi::Runtime &r, const jsi::Value &,
                             const jsi::Value *args,
                             size_t count) -> jsi::Value {
      if (count != 2)
        throw jsi::JSError(r, std::string(name) + " needs an owner and index");
      auto owner = owner_from(r, args[0]);
      jsi::Value input[] = {handle_value(owner), jsi::Value(r, args[1])};
      return call_checked(r, state, op, input, 2);
    };
  };
  result.emplace_back(
      host_function(rt, "dateTimePartType", 2, part("partType", kPartType)));
  result.emplace_back(
      host_function(rt, "dateTimePartValue", 2, part("partValue", kPartValue)));

  result.emplace_back(host_function(
      rt, "resolvedDateTimeOption", 2,
      [state](jsi::Runtime &r, const jsi::Value &, const jsi::Value *args,
              size_t count) -> jsi::Value {
        if (count != 2)
          throw jsi::JSError(r, "resolved DateTimeFormat option needs owner and field");
        auto owner = owner_from(r, args[0]);
        jsi::Value input[] = {handle_value(owner), jsi::Value(r, args[1])};
        return call_checked(r, state, kResolved, input, 2);
      }));

  result.emplace_back(host_function(
      rt, "supportedDateTimeLocales", 2,
      [state](jsi::Runtime &r, const jsi::Value &, const jsi::Value *args,
              size_t count) -> jsi::Value {
        if (count != 2)
          throw jsi::JSError(r, "supported DateTimeFormat locales need list and matcher");
        return call_checked(r, state, kSupportedLocales, args, count);
      }));

  result.emplace_back(host_function(
      rt, "canonicalTimeZone", 1,
      [state](jsi::Runtime &r, const jsi::Value &, const jsi::Value *args,
              size_t count) -> jsi::Value {
        if (count > 1)
          throw jsi::JSError(r, "canonical time zone accepts at most one value");
        return call_checked(r, state, kCanonicalTimeZone, args, count);
      }));

  return result;
}

} // namespace ibex2::intl_datetime

namespace ibex2::intl_case {

void install(jsi::Runtime &rt, const void *state) {
  auto function = jsi::Function::createFromHostFunction(
      rt, jsi::PropNameID::forAscii(rt, "__ibex2_intl_case"), 3,
      [state](jsi::Runtime &r, const jsi::Value &,
              const jsi::Value *args, size_t count) -> jsi::Value {
        if (count != 3 || !args[0].isString() || !args[1].isString() ||
            !args[2].isString())
          throw jsi::JSError(r, "Intl case mapping needs mode, locale, and string");
        std::string mode = args[0].getString(r).utf8(r);
        std::string locale = args[1].getString(r).utf8(r);
        std::u16string input = args[2].getString(r).utf16(r);
        if (input.size() > std::numeric_limits<size_t>::max() / 2)
          throw jsi::JSError(r, "Intl case input is too large");
        std::vector<unsigned char> bytes;
        bytes.reserve(input.size() * 2);
        for (char16_t unit : input) {
          bytes.push_back(static_cast<unsigned char>(unit & 0xff));
          bytes.push_back(static_cast<unsigned char>((unit >> 8) & 0xff));
        }
        Ibex2AbiValue values[] = {
            {IBEX2_TAG_STRING, 0, reinterpret_cast<const unsigned char *>(mode.data()), mode.size()},
            {IBEX2_TAG_STRING, 0, reinterpret_cast<const unsigned char *>(locale.data()), locale.size()},
            {IBEX2_TAG_BYTES, 0, bytes.data(), bytes.size()}};
        Ibex2AbiValue out{IBEX2_TAG_UNDEFINED, 0, nullptr, 0};
        const int status = ibex2_host_call(state, 97, values, 3, &out);
        struct Release {
          Ibex2AbiValue &value;
          ~Release() { ibex2_host_release(&value); }
        } release{out};
        if (status != 0) {
          jsi::Value error = from_abi(r, out);
          throw jsi::JSError(
              r, error.isString() ? error.getString(r).utf8(r)
                                  : std::string("Intl case mapping failed"));
        }
        if (out.tag != IBEX2_TAG_BYTES || out.len % 2 != 0)
          throw jsi::JSError(r, "Intl case mapping returned invalid UTF-16LE");
        std::vector<char16_t> output;
        output.reserve(out.len / 2);
        for (size_t i = 0; i < out.len; i += 2) {
          output.push_back(static_cast<char16_t>(
              static_cast<uint16_t>(out.data[i]) |
              (static_cast<uint16_t>(out.data[i + 1]) << 8)));
        }
        return jsi::String::createFromUtf16(r, output.data(), output.size());
      });
  rt.global().setProperty(rt, "__ibex2_intl_case", std::move(function));
}

} // namespace ibex2::intl_case
