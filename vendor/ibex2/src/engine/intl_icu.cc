// ICU computation backend for Ibex's Linux Intl bindings.
//
// The ECMAScript policy lives in Rust. This file deliberately exposes only
// locale/metadata queries and an immutable skeleton formatter: ICU computes,
// but it does not decide which ECMA-402 option wins or what is observable.

// @ref LLP 0057#3-the-boundary — Rust owns semantics; native libraries are computation backends

#include <unicode/ucurr.h>
#include <unicode/uformattednumber.h>
#include <unicode/uformattedvalue.h>
#include <unicode/uloc.h>
#include <unicode/unumberformatter.h>
#include <unicode/unumsys.h>
#include <unicode/ustring.h>

#include <algorithm>
#include <cstddef>
#include <cstdint>
#include <cstdlib>
#include <cstring>
#include <memory>
#include <string>
#include <unordered_set>
#include <vector>

namespace {

template <typename T, typename F>
T protect(T failure, F &&body) noexcept {
  try {
    return body();
  } catch (...) {
    return failure;
  }
}

struct Formatter {
  UNumberFormatter *value = nullptr;
  ~Formatter() { unumf_close(value); }
};

struct Span {
  int32_t field;
  size_t begin;
  size_t end;
};

struct Result {
  std::string text;
  std::vector<Span> spans;
};

char *copy_string(const std::string &value) {
  auto *out = static_cast<char *>(std::malloc(value.size() + 1));
  if (out == nullptr) return nullptr;
  std::memcpy(out, value.data(), value.size());
  out[value.size()] = '\0';
  return out;
}

bool utf8_to_uchar(const char *value, std::vector<UChar> &out) {
  if (value == nullptr) return false;
  UErrorCode status = U_ZERO_ERROR;
  int32_t length = 0;
  u_strFromUTF8(nullptr, 0, &length, value, -1, &status);
  if (status != U_BUFFER_OVERFLOW_ERROR && U_FAILURE(status)) return false;
  status = U_ZERO_ERROR;
  out.resize(static_cast<size_t>(length) + 1);
  u_strFromUTF8(out.data(), length + 1, nullptr, value, -1, &status);
  return U_SUCCESS(status);
}

bool uchar_to_utf8(const UChar *value, int32_t length, std::string &out) {
  UErrorCode status = U_ZERO_ERROR;
  int32_t bytes = 0;
  u_strToUTF8(nullptr, 0, &bytes, value, length, &status);
  if (status != U_BUFFER_OVERFLOW_ERROR && U_FAILURE(status)) return false;
  status = U_ZERO_ERROR;
  out.resize(static_cast<size_t>(bytes));
  if (bytes != 0)
    u_strToUTF8(out.data(), bytes, nullptr, value, length, &status);
  return U_SUCCESS(status);
}

size_t utf8_offset(const UChar *value, int32_t offset) {
  UErrorCode status = U_ZERO_ERROR;
  int32_t bytes = 0;
  u_strToUTF8(nullptr, 0, &bytes, value, offset, &status);
  return status == U_BUFFER_OVERFLOW_ERROR || U_SUCCESS(status)
      ? static_cast<size_t>(bytes)
      : 0;
}

std::string locale_to_tag(const char *locale) {
  UErrorCode status = U_ZERO_ERROR;
  int32_t length = uloc_toLanguageTag(locale, nullptr, 0, true, &status);
  if (status != U_BUFFER_OVERFLOW_ERROR && U_FAILURE(status)) return {};
  status = U_ZERO_ERROR;
  std::string result(static_cast<size_t>(length) + 1, '\0');
  uloc_toLanguageTag(locale, result.data(), length + 1, true, &status);
  if (U_FAILURE(status)) return {};
  result.resize(static_cast<size_t>(length));
  return result;
}

std::string tag_to_locale(const char *tag) {
  UErrorCode status = U_ZERO_ERROR;
  int32_t parsed = 0;
  int32_t length = uloc_forLanguageTag(tag, nullptr, 0, &parsed, &status);
  if ((status != U_BUFFER_OVERFLOW_ERROR && U_FAILURE(status)) ||
      parsed != static_cast<int32_t>(std::strlen(tag))) return {};
  status = U_ZERO_ERROR;
  std::string result(static_cast<size_t>(length) + 1, '\0');
  uloc_forLanguageTag(tag, result.data(), length + 1, &parsed, &status);
  if (U_FAILURE(status)) return {};
  result.resize(static_cast<size_t>(length));
  return result;
}

const std::unordered_set<std::string> &available_locales() {
  static const std::unordered_set<std::string> locales = [] {
    std::unordered_set<std::string> result;
    const int32_t count = uloc_countAvailable();
    for (int32_t i = 0; i < count; ++i) result.emplace(uloc_getAvailable(i));
    return result;
  }();
  return locales;
}

Result *make_result(const Formatter *formatter, double number,
                    const char *decimal) {
  if (formatter == nullptr || formatter->value == nullptr) return nullptr;
  UErrorCode status = U_ZERO_ERROR;
  std::unique_ptr<UFormattedNumber, decltype(&unumf_closeResult)> raw(
      unumf_openResult(&status), &unumf_closeResult);
  if (U_FAILURE(status)) return nullptr;
  if (decimal == nullptr)
    unumf_formatDouble(formatter->value, number, raw.get(), &status);
  else
    unumf_formatDecimal(formatter->value, decimal, -1, raw.get(), &status);
  if (U_FAILURE(status)) return nullptr;

  const UFormattedValue *formatted = unumf_resultAsValue(raw.get(), &status);
  int32_t length = 0;
  const UChar *text = ufmtval_getString(formatted, &length, &status);
  auto result = std::make_unique<Result>();
  if (U_FAILURE(status) || !uchar_to_utf8(text, length, result->text)) {
    return nullptr;
  }

  std::unique_ptr<UConstrainedFieldPosition, decltype(&ucfpos_close)> position(
      ucfpos_open(&status), &ucfpos_close);
  ucfpos_constrainCategory(position.get(), UFIELD_CATEGORY_NUMBER, &status);
  while (U_SUCCESS(status) &&
         ufmtval_nextPosition(formatted, position.get(), &status)) {
    int32_t begin = 0, end = 0;
    ucfpos_getIndexes(position.get(), &begin, &end, &status);
    const int32_t field = ucfpos_getField(position.get(), &status);
    if (U_SUCCESS(status) && begin >= 0 && begin <= end && end <= length) {
      result->spans.push_back(
          Span{field, utf8_offset(text, begin), utf8_offset(text, end)});
    }
  }
  return U_FAILURE(status) ? nullptr : result.release();
}

} // namespace

extern "C" {

char *ibex2_icu_default_locale() {
  return protect<char *>(nullptr, [] {
    const std::string tag = locale_to_tag(uloc_getDefault());
    return tag.empty() ? nullptr : copy_string(tag);
  });
}

char *ibex2_icu_best_available_locale(const char *tag) {
  return protect<char *>(nullptr, [tag] {
    std::string locale = tag_to_locale(tag);
    if (locale.empty()) return static_cast<char *>(nullptr);
    const auto &available = available_locales();
    while (!locale.empty()) {
      if (available.find(locale) != available.end()) {
        const std::string result = locale_to_tag(locale.c_str());
        return result.empty() ? nullptr : copy_string(result);
      }
      UErrorCode status = U_ZERO_ERROR;
      const int32_t length = uloc_getParent(locale.c_str(), nullptr, 0, &status);
      if (status != U_BUFFER_OVERFLOW_ERROR || length == 0) break;
      status = U_ZERO_ERROR;
      std::string parent(static_cast<size_t>(length) + 1, '\0');
      uloc_getParent(locale.c_str(), parent.data(), length + 1, &status);
      if (U_FAILURE(status)) break;
      parent.resize(static_cast<size_t>(length));
      if (parent == locale) break;
      locale = std::move(parent);
    }
    return static_cast<char *>(nullptr);
  });
}

char *ibex2_icu_default_numbering_system(const char *locale) {
  return protect<char *>(nullptr, [locale] {
    UErrorCode status = U_ZERO_ERROR;
    std::unique_ptr<UNumberingSystem, decltype(&unumsys_close)> system(
        unumsys_open(locale, &status), &unumsys_close);
    if (U_FAILURE(status) || system == nullptr) return static_cast<char *>(nullptr);
    const char *name = unumsys_getName(system.get());
    return name == nullptr ? nullptr : copy_string(name);
  });
}

int ibex2_icu_numbering_system_supported(const char *name) {
  return protect<int>(0, [name] {
    UErrorCode status = U_ZERO_ERROR;
    std::unique_ptr<UNumberingSystem, decltype(&unumsys_close)> system(
        unumsys_openByName(name, &status), &unumsys_close);
    return U_SUCCESS(status) && system != nullptr &&
            !unumsys_isAlgorithmic(system.get())
        ? 1
        : 0;
  });
}

int32_t ibex2_icu_currency_digits(const char *currency) {
  return protect<int32_t>(2, [currency] {
    std::vector<UChar> code;
    if (!utf8_to_uchar(currency, code)) return 2;
    UErrorCode status = U_ZERO_ERROR;
    const int32_t digits = ucurr_getDefaultFractionDigits(code.data(), &status);
    return U_SUCCESS(status) && digits >= 0 && digits <= 20 ? digits : 2;
  });
}

void *ibex2_icu_number_formatter_create(const char *locale,
                                        const char *skeleton) {
  return protect<void *>(nullptr, [locale, skeleton] {
    std::vector<UChar> spec;
    if (!utf8_to_uchar(skeleton, spec)) return static_cast<void *>(nullptr);
    UErrorCode status = U_ZERO_ERROR;
    auto formatter = std::make_unique<Formatter>();
    formatter->value = unumf_openForSkeletonAndLocale(
        spec.data(), -1, locale, &status);
    return U_FAILURE(status) || formatter->value == nullptr
        ? static_cast<void *>(nullptr)
        : static_cast<void *>(formatter.release());
  });
}

void ibex2_icu_number_formatter_destroy(void *value) noexcept {
  delete static_cast<Formatter *>(value);
}

void *ibex2_icu_number_format_double(const void *formatter, double value) {
  return protect<void *>(nullptr, [formatter, value] {
    return make_result(static_cast<const Formatter *>(formatter), value, nullptr);
  });
}

void *ibex2_icu_number_format_decimal(const void *formatter,
                                      const char *value) {
  return protect<void *>(nullptr, [formatter, value] {
    return make_result(static_cast<const Formatter *>(formatter), 0, value);
  });
}

const char *ibex2_icu_number_result_text(const void *value,
                                         size_t *length) noexcept {
  const auto *result = static_cast<const Result *>(value);
  if (result == nullptr) return nullptr;
  if (length != nullptr) *length = result->text.size();
  return result->text.data();
}

size_t ibex2_icu_number_result_field_count(const void *value) noexcept {
  const auto *result = static_cast<const Result *>(value);
  return result == nullptr ? 0 : result->spans.size();
}

int ibex2_icu_number_result_field(const void *value, size_t index,
                                  int32_t *field, size_t *begin,
                                  size_t *end) noexcept {
  const auto *result = static_cast<const Result *>(value);
  if (result == nullptr || index >= result->spans.size()) return 0;
  const Span &span = result->spans[index];
  if (field != nullptr) *field = span.field;
  if (begin != nullptr) *begin = span.begin;
  if (end != nullptr) *end = span.end;
  return 1;
}

void ibex2_icu_number_result_destroy(void *value) noexcept {
  delete static_cast<Result *>(value);
}

void ibex2_icu_string_destroy(char *value) noexcept { std::free(value); }

} // extern "C"
