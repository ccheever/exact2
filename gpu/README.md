# GPU module ABI

The host and module ship together. Before 1.0, missing required symbols refuse
loading; there is no compatibility shim. See LLP 1009 and LLP 1014.000.

| Version / revision | Child delivery export | Contract |
|---|---|---|
| GPU ABI 1 / exact-gpu 0.1.0, before `126daba5` | `gpu_child` | Unnamed child pixels and geometry. |
| GPU ABI 1 / exact-gpu 0.1.0, from `126daba5` | `gpu_child_view` | **Breaking host ABI change:** adds UTF-8 `testId` and its byte length before geometry and pixel arguments, on native and Wasm. All embedders must rebuild against this revision. |

GPU tests use `exact_gpu::fixture::device_or_skip`: only `no adapter:` skips;
request-device, configuration, validation, and device-loss errors fail.
