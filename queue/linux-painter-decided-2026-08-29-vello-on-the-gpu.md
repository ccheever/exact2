**Linux painter: decided 2026-08-29 — vello on the GPU** (Charlie), with tiny-skia
kept as the CPU fallback and the pixel oracle (LLP 1015 §7). What it owes: the
DEFERRED wording "compiles no shaders at runtime" now needs the clause "the GPU
painter compiles its shaders on the first launch on a machine and reads them from
the pipeline cache after" beside LLP 1009 §5's — Charlie's to write.

*Filed under “Open decisions (Charlie's)”.*
