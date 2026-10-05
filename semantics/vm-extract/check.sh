#!/bin/sh
# Extract runner/src/machine.rs to Lean (Charon, then Aeneas) and check the
# proofs that it steps as semantics/Contract/Vm.lean does (README.md).
#
# Tools, outside the repo: CHARON (default ~/tools/charon/bin/charon), its
# pinned nightly (CHARON_TOOLCHAIN, default the one ~/tools/charon pins),
# AENEAS_DIR (default ~/tools/aeneas, with bin/aeneas built and its Lean
# library built), and elan's lake. A missing tool skips the check, naming it.
set -u
here=$(cd "$(dirname "$0")" && pwd)
aeneas_dir=${AENEAS_DIR:-$HOME/tools/aeneas}
charon=${CHARON:-$HOME/tools/charon/bin/charon}
toolchain=${CHARON_TOOLCHAIN:-$(sed -n 's/^channel = "\(.*\)"/\1/p' "$(dirname "$(dirname "$charon")")/rust-toolchain" 2>/dev/null)}
PATH="$HOME/.elan/bin:$PATH"
skip() { echo "vm-extract: skipped: $1"; exit 0; }
[ -x "$charon" ] || skip "Charon not found at $charon (CHARON)"
[ -x "$aeneas_dir/bin/aeneas" ] || skip "Aeneas not found at $aeneas_dir/bin/aeneas (AENEAS_DIR)"
[ -d "$aeneas_dir/backends/lean/.lake/build" ] || skip "Aeneas's Lean library is not built ($aeneas_dir/backends/lean: lake build)"
[ -n "$toolchain" ] || skip "Charon's nightly is not known (CHARON_TOOLCHAIN)"
rustup run "$toolchain" rustc --version >/dev/null 2>&1 || skip "Charon's nightly $toolchain is not installed"
command -v lake >/dev/null || skip "lake not found (elan)"

work=${TMPDIR:-/tmp}/vm-extract.$$
mkdir -p "$work"
trap 'rm -rf "$work"' EXIT
out="$here/lean/VmExtract"

echo "vm-extract: charon"
(cd "$here" && RUSTUP_TOOLCHAIN=$toolchain CARGO_TARGET_DIR="$work/target" \
  "$charon" cargo --preset=aeneas \
  --start-from vm_extract::machine::run --start-from vm_extract::machine::equal \
  --dest-file "$work/vm.llbc" >"$work/charon.log" 2>&1) \
  || { cat "$work/charon.log"; echo "error: vm-extract: charon failed"; exit 1; }

echo "vm-extract: aeneas"
"$aeneas_dir/bin/aeneas" -backend lean -use-lean-modules false -split-files -sequential -loops-to-rec \
  -no-progress-bar -subdir VmExtract -dest "$work/lean" "$work/vm.llbc" >"$work/aeneas.log" 2>&1 \
  || { cat "$work/aeneas.log"; echo "error: vm-extract: aeneas failed"; exit 1; }
# Every external Aeneas asks for must be one FunsExternal.lean models.
for f in $(sed -n 's/^axiom \([^ ]*\).*/\1/p' "$work/lean/VmExtract/FunsExternal_Template.lean"); do
  grep -q "^def $f " "$out/FunsExternal.lean" \
    || { echo "error: vm-extract: $f is not modelled in FunsExternal.lean"; exit 1; }
done
# `f64` is `F64` in the generated types: VmExtract/Prelude.lean defines it.
awk '{ print } /^import Aeneas$/ { print "import VmExtract.Prelude" }' \
  "$work/lean/VmExtract/Types.lean" >"$out/Types.lean"
cp "$work/lean/VmExtract/Funs.lean" "$out/Funs.lean"

echo "vm-extract: lake build"
log=$(cd "$here/lean" && lake build 2>&1); status=$?
echo "$log"
[ $status -eq 0 ] || { echo "error: vm-extract: the proofs do not check"; exit 1; }
if echo "$log" | grep -q "declaration uses .sorry."; then
  echo "error: vm-extract: a proof uses sorry"; exit 1
fi
# The theorems rest on Lean's axioms alone (VmExtract.lean prints them).
if echo "$log" | grep "depends on axioms" | grep -qv "\[propext, Classical.choice, Quot.sound\]"; then
  echo "error: vm-extract: a theorem depends on an axiom beyond Lean's own"; exit 1
fi
[ "$(echo "$log" | grep -c "depends on axioms")" -eq 3 ] \
  || { echo "error: vm-extract: the theorems' axioms were not printed"; exit 1; }
echo "vm-extract: ok"
