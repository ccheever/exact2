// ICU computation backend for locale-sensitive String case mapping.
//
// ECMA-402 coercion and locale selection happen before this seam. This file
// accepts explicit UTF-16 code units so embedded NUL and unpaired surrogates
// never pass through a lossy UTF-8 conversion.

// @ref LLP 0057#3-the-boundary — native libraries compute; Rust owns semantics

#include <unicode/ustring.h>

#include <cstddef>
#include <cstdint>
#include <limits>
#include <string>
#include <vector>

namespace {

constexpr int kLower = 0;
constexpr int kUpper = 1;

int validate(uint8_t mode, const char *locale_data, size_t locale_length,
             const uint16_t *input, size_t input_length,
             size_t output_capacity) {
  if ((mode != kLower && mode != kUpper) || locale_data == nullptr ||
      (input == nullptr && input_length != 0) ||
      locale_length > static_cast<size_t>(std::numeric_limits<int32_t>::max()) ||
      input_length > static_cast<size_t>(std::numeric_limits<int32_t>::max()) ||
      output_capacity > static_cast<size_t>(std::numeric_limits<int32_t>::max())) {
    return 1;
  }
  return 0;
}

} // namespace

extern "C" int ibex2_icu_case_map(
    uint8_t mode, const char *locale_data, size_t locale_length,
    const uint16_t *input, size_t input_length, uint16_t *output,
    size_t output_capacity, size_t *output_length) noexcept {
  try {
    if (output_length == nullptr || (output == nullptr && output_capacity != 0))
      return 1;

    const int validity = validate(mode, locale_data, locale_length, input,
                                  input_length, output_capacity);
    if (validity != 0) return validity;

    // Canonical language tags cannot contain NUL, but the ABI carries an
    // explicit length. Reject one instead of silently truncating at c_str().
    const std::string locale(locale_data, locale_length);
    if (locale.find('\0') != std::string::npos) return 1;

    std::vector<UChar> source(input_length);
    for (size_t i = 0; i < input_length; ++i)
      source[i] = static_cast<UChar>(input[i]);
    // ICU requires a valid source pointer even when the explicit length is
    // zero. A vector is permitted to return null for data() in that case.
    const UChar empty_source = 0;
    const UChar *source_data = source.empty() ? &empty_source : source.data();

    std::vector<UChar> destination(output_capacity);
    UChar *destination_data = destination.empty() ? nullptr : destination.data();
    UErrorCode icu_status = U_ZERO_ERROR;
    const auto convert = mode == kLower ? u_strToLower : u_strToUpper;
    const int32_t written = convert(
        destination_data, static_cast<int32_t>(output_capacity), source_data,
        static_cast<int32_t>(source.size()), locale.c_str(), &icu_status);
    if (written < 0) return 2;
    *output_length = static_cast<size_t>(written);

    if (icu_status == U_BUFFER_OVERFLOW_ERROR) return output == nullptr ? 0 : 3;
    if (U_FAILURE(icu_status)) return 2;
    if (output == nullptr) return 0;
    if (static_cast<size_t>(written) > output_capacity) return 3;
    for (int32_t i = 0; i < written; ++i)
      output[i] = static_cast<uint16_t>(destination[static_cast<size_t>(i)]);
    return 0;
  } catch (...) {
    // No C++ exception may cross the Rust ABI.
    return 4;
  }
}
