// ICU computation backend for Ibex's Linux Intl.DateTimeFormat binding.
//
// Rust chooses ECMA-402 policy and owns formatter state. This file is the
// narrow native computation seam: CLDR pattern selection, calendar/time-zone
// metadata, and formatting with field positions from the same UDateFormat.
//
// @ref LLP 0057#3-the-boundary — Rust owns semantics; ICU is the computation backend

#include <unicode/ucal.h>
#include <unicode/udat.h>
#include <unicode/udatpg.h>
#include <unicode/uenum.h>
#include <unicode/ufieldpositer.h>
#include <unicode/uloc.h>
#include <unicode/stringoptions.h>
#include <unicode/ustring.h>

#include <cstddef>
#include <cstdint>
#include <cstdlib>
#include <cstring>
#include <memory>
#include <string>
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
  UDateFormat *value = nullptr;
  std::string pattern;
  ~Formatter() { udat_close(value); }
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

std::string tag_to_locale(const char *tag) {
  if (tag == nullptr) return {};
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

std::string unicode_calendar_type(const char *legacy) {
  if (legacy == nullptr) return {};
  const char *type = uloc_toUnicodeLocaleType("calendar", legacy);
  return type == nullptr ? std::string(legacy) : std::string(type);
}

std::string calendar_for_locale(const char *locale) {
  const std::string id = tag_to_locale(locale);
  if (id.empty()) return {};
  UErrorCode status = U_ZERO_ERROR;
  std::unique_ptr<UCalendar, decltype(&ucal_close)> calendar(
      ucal_open(nullptr, 0, id.c_str(), UCAL_DEFAULT, &status), &ucal_close);
  if (U_FAILURE(status) || calendar == nullptr) return {};
  return unicode_calendar_type(ucal_getType(calendar.get(), &status));
}

bool best_pattern(const std::string &locale, const char *skeleton,
                  std::vector<UChar> &out) {
  std::vector<UChar> input;
  if (!utf8_to_uchar(skeleton, input)) return false;
  UErrorCode status = U_ZERO_ERROR;
  std::unique_ptr<UDateTimePatternGenerator, decltype(&udatpg_close)> generator(
      udatpg_open(locale.c_str(), &status), &udatpg_close);
  if (U_FAILURE(status) || generator == nullptr) return false;
  int32_t length = udatpg_getBestPattern(generator.get(), input.data(), -1,
                                         nullptr, 0, &status);
  if (status != U_BUFFER_OVERFLOW_ERROR && U_FAILURE(status)) return false;
  status = U_ZERO_ERROR;
  out.resize(static_cast<size_t>(length) + 1);
  udatpg_getBestPattern(generator.get(), input.data(), -1, out.data(),
                        length + 1, &status);
  return U_SUCCESS(status);
}

std::string formatter_pattern(const UDateFormat *formatter) {
  UErrorCode status = U_ZERO_ERROR;
  int32_t length = udat_toPattern(formatter, false, nullptr, 0, &status);
  if (status != U_BUFFER_OVERFLOW_ERROR && U_FAILURE(status)) return {};
  status = U_ZERO_ERROR;
  std::vector<UChar> pattern(static_cast<size_t>(length) + 1);
  udat_toPattern(formatter, false, pattern.data(), length + 1, &status);
  std::string result;
  return U_SUCCESS(status) && uchar_to_utf8(pattern.data(), length, result)
      ? result
      : std::string();
}

UDateFormatStyle style_from_int(int32_t style) {
  switch (style) {
    case 0: return UDAT_FULL;
    case 1: return UDAT_LONG;
    case 2: return UDAT_MEDIUM;
    case 3: return UDAT_SHORT;
    default: return UDAT_NONE;
  }
}

int32_t stable_field(int32_t field) {
  switch (field) {
    case UDAT_ERA_FIELD: return 0;
    case UDAT_YEAR_FIELD:
    case UDAT_YEAR_WOY_FIELD:
    case UDAT_EXTENDED_YEAR_FIELD: return 1;
    case UDAT_MONTH_FIELD:
    case UDAT_STANDALONE_MONTH_FIELD:
    case UDAT_QUARTER_FIELD:
    case UDAT_STANDALONE_QUARTER_FIELD: return 2;
    case UDAT_DATE_FIELD: return 3;
    case UDAT_HOUR_OF_DAY1_FIELD:
    case UDAT_HOUR_OF_DAY0_FIELD:
    case UDAT_HOUR1_FIELD:
    case UDAT_HOUR0_FIELD: return 4;
    case UDAT_MINUTE_FIELD: return 5;
    case UDAT_SECOND_FIELD: return 6;
    case UDAT_DAY_OF_WEEK_FIELD:
    case UDAT_DOW_LOCAL_FIELD:
    case UDAT_STANDALONE_DAY_FIELD: return 7;
    case UDAT_AM_PM_FIELD:
    case UDAT_AM_PM_MIDNIGHT_NOON_FIELD:
    case UDAT_FLEXIBLE_DAY_PERIOD_FIELD: return 8;
    case UDAT_TIMEZONE_FIELD:
    case UDAT_TIMEZONE_RFC_FIELD:
    case UDAT_TIMEZONE_GENERIC_FIELD:
    case UDAT_TIMEZONE_SPECIAL_FIELD:
    case UDAT_TIMEZONE_LOCALIZED_GMT_OFFSET_FIELD:
    case UDAT_TIMEZONE_ISO_FIELD:
    case UDAT_TIMEZONE_ISO_LOCAL_FIELD: return 9;
    case UDAT_FRACTIONAL_SECOND_FIELD: return 10;
    case UDAT_RELATED_YEAR_FIELD: return 11;
    case UDAT_YEAR_NAME_FIELD: return 12;
    default: return -1;
  }
}

Result *make_result(const Formatter *formatter, double millis) {
  if (formatter == nullptr || formatter->value == nullptr) return nullptr;
  UErrorCode status = U_ZERO_ERROR;
  int32_t length = udat_format(formatter->value, millis, nullptr, 0, nullptr,
                               &status);
  if (status != U_BUFFER_OVERFLOW_ERROR && U_FAILURE(status)) return nullptr;
  status = U_ZERO_ERROR;
  std::vector<UChar> text(static_cast<size_t>(length) + 1);
  std::unique_ptr<UFieldPositionIterator, decltype(&ufieldpositer_close)>
      positions(ufieldpositer_open(&status), &ufieldpositer_close);
  if (U_FAILURE(status) || positions == nullptr) return nullptr;
  udat_formatForFields(formatter->value, millis, text.data(), length + 1,
                       positions.get(), &status);
  auto result = std::make_unique<Result>();
  if (U_FAILURE(status) || !uchar_to_utf8(text.data(), length, result->text))
    return nullptr;

  for (;;) {
    int32_t begin = 0, end = 0;
    const int32_t raw_field =
        ufieldpositer_next(positions.get(), &begin, &end);
    if (raw_field < 0) break;
    const int32_t field = stable_field(raw_field);
    if (field < 0) continue;
    if (begin >= 0 && begin <= end && end <= length) {
      result->spans.push_back(
          Span{field, utf8_offset(text.data(), begin),
               utf8_offset(text.data(), end)});
    }
  }
  return result.release();
}

std::string canonical_time_zone(const char *zone) {
  std::vector<UChar> input;
  if (zone == nullptr) {
    UErrorCode status = U_ZERO_ERROR;
    input.resize(64);
    int32_t length = ucal_getDefaultTimeZone(
        input.data(), static_cast<int32_t>(input.size()), &status);
    if (status == U_BUFFER_OVERFLOW_ERROR) {
      status = U_ZERO_ERROR;
      input.resize(static_cast<size_t>(length) + 1);
      length = ucal_getDefaultTimeZone(
          input.data(), static_cast<int32_t>(input.size()), &status);
    }
    if (U_FAILURE(status)) return {};
    input.resize(static_cast<size_t>(length));
  } else if (!utf8_to_uchar(zone, input)) {
    return {};
  } else if (!input.empty()) {
    // IANA time-zone identifiers are ASCII. ICU's Unicode case folding would
    // otherwise accept lookalikes such as the Kelvin sign in Asia/Kolkata.
    for (const UChar code_unit : input) {
      if (code_unit > 0x7f) return {};
    }
    input.pop_back();
    // ECMA-402 time-zone matching is ASCII case-insensitive. ICU's canonical
    // query is strict about spelling, so recover the spelling from ICU's own
    // zone inventory before asking it to canonicalize aliases.
    UErrorCode enumeration_status = U_ZERO_ERROR;
    std::unique_ptr<UEnumeration, decltype(&uenum_close)> zones(
        ucal_openTimeZones(&enumeration_status), &uenum_close);
    bool found = false;
    while (U_SUCCESS(enumeration_status) && zones != nullptr) {
      int32_t candidate_length = 0;
      const UChar *candidate =
          uenum_unext(zones.get(), &candidate_length, &enumeration_status);
      if (candidate == nullptr) break;
      UErrorCode compare_status = U_ZERO_ERROR;
      if (u_strCaseCompare(input.data(), static_cast<int32_t>(input.size()),
                           candidate, candidate_length, U_FOLD_CASE_DEFAULT,
                           &compare_status) == 0 && U_SUCCESS(compare_status)) {
        input.assign(candidate, candidate + candidate_length);
        found = true;
        break;
      }
    }
    if (!found) return {};
  }

  UErrorCode status = U_ZERO_ERROR;
  UBool system = false;
  std::vector<UChar> canonical(64);
  int32_t length = ucal_getCanonicalTimeZoneID(
      input.data(), static_cast<int32_t>(input.size()), canonical.data(),
      static_cast<int32_t>(canonical.size()), &system, &status);
  if (status == U_BUFFER_OVERFLOW_ERROR) {
    status = U_ZERO_ERROR;
    canonical.resize(static_cast<size_t>(length) + 1);
    length = ucal_getCanonicalTimeZoneID(
        input.data(), static_cast<int32_t>(input.size()), canonical.data(),
        static_cast<int32_t>(canonical.size()), &system, &status);
  }
  std::string result;
  if (U_FAILURE(status) || !system ||
      !uchar_to_utf8(canonical.data(), length, result)) return {};
  if (result == "Etc/UTC" || result == "Etc/GMT" || result == "GMT")
    return "UTC";
  return result;
}

}  // namespace

extern "C" {

char *ibex2_icu_datetime_default_calendar(const char *locale) {
  return protect<char *>(nullptr, [locale] {
    const std::string calendar = calendar_for_locale(locale);
    return calendar.empty() ? nullptr : copy_string(calendar);
  });
}

int ibex2_icu_datetime_calendar_supported(const char *locale,
                                          const char *calendar) {
  return protect<int>(0, [locale, calendar] {
    if (locale == nullptr || calendar == nullptr) return 0;
    const std::string id = tag_to_locale(locale);
    if (id.empty()) return 0;
    UErrorCode status = U_ZERO_ERROR;
    std::unique_ptr<UEnumeration, decltype(&uenum_close)> values(
        ucal_getKeywordValuesForLocale("calendar", id.c_str(), false, &status),
        &uenum_close);
    if (U_FAILURE(status) || values == nullptr) return 0;
    for (;;) {
      int32_t length = 0;
      const char *value = uenum_next(values.get(), &length, &status);
      if (U_FAILURE(status) || value == nullptr) break;
      if (unicode_calendar_type(value) == calendar) return 1;
    }
    return 0;
  });
}

int ibex2_icu_datetime_default_hour_cycle(const char *locale) {
  return protect<int>(0, [locale] {
    const std::string id = tag_to_locale(locale);
    if (id.empty()) return 0;
    std::vector<UChar> pattern;
    if (!best_pattern(id, "j", pattern)) return 0;
    bool quoted = false;
    for (size_t i = 0; i + 1 < pattern.size(); ++i) {
      const UChar ch = pattern[i];
      if (ch == 0x27) {
        if (i + 1 < pattern.size() && pattern[i + 1] == 0x27) {
          ++i;
        } else {
          quoted = !quoted;
        }
      } else if (!quoted) {
        if (ch == 0x4b) return 11;  // K
        if (ch == 0x68) return 12;  // h
        if (ch == 0x48) return 23;  // H
        if (ch == 0x6b) return 24;  // k
      }
    }
    return 0;
  });
}

char *ibex2_icu_datetime_canonical_time_zone(const char *zone) {
  return protect<char *>(nullptr, [zone] {
    const std::string result = canonical_time_zone(zone);
    return result.empty() ? nullptr : copy_string(result);
  });
}

void *ibex2_icu_datetime_formatter_create(const char *locale,
                                          const char *time_zone,
                                          const char *skeleton,
                                          int32_t date_style,
                                          int32_t time_style) {
  return protect<void *>(nullptr, [=] {
    const std::string id = tag_to_locale(locale);
    std::vector<UChar> zone;
    if (id.empty() || !utf8_to_uchar(time_zone, zone))
      return static_cast<void *>(nullptr);
    UErrorCode status = U_ZERO_ERROR;
    auto formatter = std::make_unique<Formatter>();
    if (date_style >= 0 || time_style >= 0) {
      formatter->value = udat_open(
          style_from_int(time_style), style_from_int(date_style), id.c_str(),
          zone.data(), -1, nullptr, 0, &status);
    } else {
      std::vector<UChar> pattern;
      if (skeleton == nullptr || !best_pattern(id, skeleton, pattern))
        return static_cast<void *>(nullptr);
      formatter->value = udat_open(UDAT_PATTERN, UDAT_PATTERN, id.c_str(),
                                   zone.data(), -1, pattern.data(), -1,
                                   &status);
    }
    if (U_FAILURE(status) || formatter->value == nullptr)
      return static_cast<void *>(nullptr);
    formatter->pattern = formatter_pattern(formatter->value);
    return formatter->pattern.empty()
        ? static_cast<void *>(nullptr)
        : static_cast<void *>(formatter.release());
  });
}

void ibex2_icu_datetime_formatter_destroy(void *value) noexcept {
  delete static_cast<Formatter *>(value);
}

const char *ibex2_icu_datetime_formatter_pattern(const void *value,
                                                  size_t *length) noexcept {
  const auto *formatter = static_cast<const Formatter *>(value);
  if (formatter == nullptr) return nullptr;
  if (length != nullptr) *length = formatter->pattern.size();
  return formatter->pattern.data();
}

void *ibex2_icu_datetime_format(const void *formatter, double millis) {
  return protect<void *>(nullptr, [formatter, millis] {
    return make_result(static_cast<const Formatter *>(formatter), millis);
  });
}

const char *ibex2_icu_datetime_result_text(const void *value,
                                           size_t *length) noexcept {
  const auto *result = static_cast<const Result *>(value);
  if (result == nullptr) return nullptr;
  if (length != nullptr) *length = result->text.size();
  return result->text.data();
}

size_t ibex2_icu_datetime_result_field_count(const void *value) noexcept {
  const auto *result = static_cast<const Result *>(value);
  return result == nullptr ? 0 : result->spans.size();
}

int ibex2_icu_datetime_result_field(const void *value, size_t index,
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

void ibex2_icu_datetime_result_destroy(void *value) noexcept {
  delete static_cast<Result *>(value);
}

}  // extern "C"
