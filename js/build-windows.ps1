#requires -Version 7.0
<#
Build Exact's pinned bytecode-only Hermes in an x64 Visual Studio developer shell.
  pwsh -File js/build-windows.ps1 -Jobs 2
EXACT_HERMES_DIR selects an absent final install; otherwise use the resolver's
LOCALAPPDATA cache. Existing installs, sources and build directories are never
replaced. Failed work is retained at the printed short work directory.
@ref LLP 1027.006
#>
[CmdletBinding()]
param(
  [ValidateRange(1, 2)][int]$Jobs = 2,
  [string]$WorkRoot = '',
  [string]$SourceRepository = ''
)

$ErrorActionPreference = 'Stop'
$PSNativeCommandUseErrorActionPreference = $false
Set-StrictMode -Version Latest
if (-not $IsWindows -or -not [Environment]::Is64BitProcess) {
  throw 'This builder requires 64-bit Windows PowerShell 7 and x64 MSVC.'
}
if ($env:VSCMD_ARG_TGT_ARCH -ne 'x64') {
  throw 'Run in an x64 Visual Studio developer shell (VsDevCmd.bat -arch=x64).'
}
foreach ($name in @('git', 'tar', 'cmake', 'ninja', 'cl', 'link', 'lib', 'rc', 'dumpbin')) {
  if (-not (Get-Command $name -CommandType Application -ErrorAction SilentlyContinue)) {
    throw "Required tool is missing: $name"
  }
}
foreach ($name in @('CL', '_CL_', 'CFLAGS', 'CXXFLAGS', 'CPPFLAGS', 'LDFLAGS', 'CMAKE_TOOLCHAIN_FILE',
    'ICU_ROOT', 'ICU_DATA', 'CMAKE_PREFIX_PATH', 'CONFIG_SITE', 'BASH_ENV', 'ENV')) {
  if ([Environment]::GetEnvironmentVariable($name)) {
    throw "Unset $name for this explicit Release /MD build; inherited compiler flags are not accepted."
  }
}
$pinText = Get-Content -LiteralPath (Join-Path $PSScriptRoot 'hermes.rs') -Raw
$pins = [regex]::Matches($pinText, 'pub const HERMES_PIN: &str = "([0-9a-f]{40})";')
if ($pins.Count -ne 1) { throw 'Cannot read the single HERMES_PIN in js/hermes.rs.' }
$pin = $pins[0].Groups[1].Value
$target = 'x86_64-pc-windows-msvc'
$requestedInstall = $env:EXACT_HERMES_DIR
if (-not $requestedInstall) {
  if (-not $env:LOCALAPPDATA) { throw 'Set LOCALAPPDATA or EXACT_HERMES_DIR.' }
  $requestedInstall = Join-Path $env:LOCALAPPDATA "Exact/hermes/$($pin.Substring(0, 12))-lean-windows-x64-icu76-intl1"
}
$requestedInstall = [IO.Path]::GetFullPath($requestedInstall)
if (Test-Path -LiteralPath $requestedInstall) {
  throw "Installation already exists; preserve it and choose a new EXACT_HERMES_DIR: $requestedInstall"
}
if (-not $WorkRoot) { $WorkRoot = Join-Path "$($env:SystemDrive)\" 'Temp/exact-hermes' }
$WorkRoot = [IO.Path]::GetFullPath($WorkRoot)

# Resolve the actual directory handle, including packaged-app path redirection.
# This is provenance/local setup, not a hostile-filesystem containment boundary.
Add-Type -TypeDefinition @'
using System;
using System.ComponentModel;
using System.Runtime.InteropServices;
using System.Text;
using Microsoft.Win32.SafeHandles;
public static class ExactHermesBuildPath {
  [DllImport("kernel32.dll", CharSet=CharSet.Unicode, SetLastError=true)]
  static extern SafeFileHandle CreateFileW(string path, uint access, uint share,
      IntPtr security, uint disposition, uint flags, IntPtr template);
  [DllImport("kernel32.dll", CharSet=CharSet.Unicode, SetLastError=true)]
  static extern uint GetFinalPathNameByHandleW(SafeFileHandle file,
      StringBuilder path, uint capacity, uint flags);
  public static string Resolve(string path) {
    using (var file = CreateFileW(path, 0, 7, IntPtr.Zero, 3, 0x02000000, IntPtr.Zero)) {
      if (file.IsInvalid) throw new Win32Exception(Marshal.GetLastWin32Error());
      var result = new StringBuilder(32768);
      uint length = GetFinalPathNameByHandleW(file, result, (uint)result.Capacity, 0);
      if (length == 0) throw new Win32Exception(Marshal.GetLastWin32Error());
      if (length >= result.Capacity) throw new InvalidOperationException("Resolved path is too long");
      string value = result.ToString();
      if (value.StartsWith(@"\\?\UNC\", StringComparison.Ordinal)) return @"\\" + value.Substring(8);
      return value.StartsWith(@"\\?\", StringComparison.Ordinal) ? value.Substring(4) : value;
    }
  }
}
'@
[IO.Directory]::CreateDirectory($WorkRoot) | Out-Null
$workParent = [ExactHermesBuildPath]::Resolve($WorkRoot)
$work = Join-Path $workParent ([Guid]::NewGuid().ToString('N').Substring(0, 12))
if ($work.Length -gt 64) {
  throw "Use a shorter -WorkRoot (resolved build root must be at most 64 characters): $work"
}
if (Test-Path -LiteralPath $work) { throw "Unique work directory already exists: $work" }
[IO.Directory]::CreateDirectory($work) | Out-Null
$work = [ExactHermesBuildPath]::Resolve($work)
Write-Host "Owned work directory (retained on success or failure): $work"
$installParent = Split-Path -Parent $requestedInstall
[IO.Directory]::CreateDirectory($installParent) | Out-Null
$installParent = [ExactHermesBuildPath]::Resolve($installParent)
$install = Join-Path $installParent (Split-Path -Leaf $requestedInstall)
$stage = Join-Path $installParent ".hermes-stage-$([Guid]::NewGuid().ToString('N'))"
$logNumber = 0

function Invoke-Logged([string]$Tool, [string[]]$Arguments, [string]$Label, [bool]$Quiet = $false) {
  $script:logNumber++
  $log = Join-Path $work ('{0:d2}-{1}.log' -f $script:logNumber, $Label)
  Write-Host "$Label -> $log"
  & $Tool @Arguments 2>&1 | Tee-Object -FilePath $log | ForEach-Object {
    if (-not $Quiet) { Write-Host "$_" }
  }
  if ($LASTEXITCODE -ne 0) { throw "$Label failed with exit $LASTEXITCODE; retained log: $log" }
  return $log
}
function Identity([string]$Path) {
  $item = Get-Item -LiteralPath $Path
  if ($item.PSIsContainer -or ($item.Attributes -band [IO.FileAttributes]::ReparsePoint)) {
    throw "Expected a regular file: $Path"
  }
  return [ordered]@{ bytes = $item.Length; sha256 = (Get-FileHash -LiteralPath $Path -Algorithm SHA256).Hash.ToLowerInvariant() }
}
function Copy-Headers([string]$From, [string]$To) {
  foreach ($entry in Get-ChildItem -LiteralPath $From -Recurse) {
    if ($entry.Attributes -band [IO.FileAttributes]::ReparsePoint) {
      throw "Reparse entry in exported header tree: $($entry.FullName)"
    }
    if ($entry.PSIsContainer -or $entry.Extension -notin @('.h', '.hpp', '.inc')) { continue }
    $destination = Join-Path $To ([IO.Path]::GetRelativePath($From, $entry.FullName))
    [IO.Directory]::CreateDirectory((Split-Path -Parent $destination)) | Out-Null
    Copy-Item -LiteralPath $entry.FullName -Destination $destination
  }
}

function Write-Utf8([string]$Path, [string]$Text) {
  [IO.File]::WriteAllText($Path, $Text.Replace("`r`n", "`n"), [Text.UTF8Encoding]::new($false))
}
function Get-PinnedArchive([string]$Url, [string]$Name, [long]$Bytes,
    [string]$Algorithm, [string]$Digest) {
  $path = Join-Path $work $Name
  if (Test-Path -LiteralPath $path) { throw "Fresh download already exists: $path" }
  Write-Host "Fetching pinned input: $Url"
  Invoke-WebRequest -Uri $Url -OutFile $path
  if ((Get-Item -LiteralPath $path).Length -ne $Bytes -or
      (Get-FileHash -LiteralPath $path -Algorithm $Algorithm).Hash.ToLowerInvariant() -cne $Digest) {
    throw "Pinned archive length/$Algorithm mismatch; retained: $path"
  }
  return $path
}
function Assert-NoIcuDll([string]$Executable, [string]$Label) {
  $log = Invoke-Logged dumpbin @('/nologo', '/dependents', $Executable) $Label $true
  $text = Get-Content -LiteralPath $log -Raw
  if ($text -match '(?im)^\s*(?:s?icu[^\s]*|msys[^\s]*|cygwin[^\s]*)\.dll\s*$') {
    throw "Unexpected ICU/MSYS runtime dependency: $log"
  }
  return [ordered]@{ log = $log; identity = (Identity $log) }
}
function Build-StaticIcu {
  # Build-only dependencies are private, pinned, and never installed by pacman.
  $icuHash = 'b702ab62fb37a1574d5f4a768326d0f8fa30d9db5b015605b5f8215b5d8547f83d84880c586d3dcc7b6c76f8d47ef34e04b0f51baa55908f737024dd79a42a6c'
  $msysHash = 'ea2f31a0b6ade63914ce441ffb022f0f6aa96982bfefa2326460a26d5fb01322'
  $makeHash = 'af0bdba17f06fe037f0194069adaa31a8fe45f1a11381501896aea1fae37bd5d'
  $icuArchive = Get-PinnedArchive 'https://github.com/unicode-org/icu/releases/download/release-76-1/icu4c-76_1-src.tgz' 'icu.tgz' 27437767 'SHA512' $icuHash
  $msysArchive = Get-PinnedArchive 'https://github.com/msys2/msys2-installer/releases/download/2026-09-27/msys2-base-x86_64-20260927.tar.xz' 'msys.tar.xz' 42860696 'SHA256' $msysHash
  $makeArchive = Get-PinnedArchive 'https://repo.msys2.org/msys/x86_64/make-4.4.1-3-x86_64.pkg.tar.zst' 'make.tar.zst' 514683 'SHA256' $makeHash
  $null = Invoke-Logged tar @('-xf', $icuArchive, '-C', $work) 'icu-extract'
  $null = Invoke-Logged tar @('-xf', $msysArchive, '-C', $work) 'msys-extract'
  $msys = Join-Path $work 'msys64'
  $makeMeta = Join-Path $work 'make-meta'
  [IO.Directory]::CreateDirectory($makeMeta) | Out-Null
  $null = Invoke-Logged tar @('-xf', $makeArchive, '-C', $makeMeta, '.PKGINFO') 'make-metadata'
  $metadata = Get-Content -LiteralPath (Join-Path $makeMeta '.PKGINFO') -Raw
  $dependencies = @([regex]::Matches($metadata, '(?m)^depend = ([^\r\n]+)') | ForEach-Object { $_.Groups[1].Value })
  if ($dependencies.Count -eq 0) { throw 'Pinned make package has no dependency metadata.' }
  foreach ($dependency in $dependencies) {
    if ($dependency -notin @('libintl', 'sh')) { throw "Unqualified make dependency: $dependency" }
    $package = if ($dependency -eq 'sh') { 'bash' } else { 'libintl' }
    $providers = @(Get-ChildItem -LiteralPath (Join-Path $msys 'var/lib/pacman/local') -Directory | Where-Object {
      $description = Join-Path $_.FullName 'desc'
      (Test-Path -LiteralPath $description -PathType Leaf) -and
        (Get-Content -LiteralPath $description -Raw) -match "(?m)^%NAME%\r?\n$package\r?$"
    })
    if ($providers.Count -ne 1) {
      throw "Pinned MSYS base does not supply declared make dependency: $dependency"
    }
    if ($dependency -eq 'sh' -and (Get-Content -LiteralPath (Join-Path $providers[0].FullName 'desc') -Raw) -notmatch '(?m)^%PROVIDES%\r?\nsh\r?$') {
      throw 'Pinned Bash package does not declare the required sh provider.'
    }
  }
  foreach ($required in @('usr/bin/bash.exe', 'usr/bin/sh.exe', 'usr/bin/msys-intl-8.dll', 'usr/bin/cygpath.exe')) {
    if (-not (Test-Path -LiteralPath (Join-Path $msys $required) -PathType Leaf)) {
      throw "Pinned MSYS base missing required build tool/dependency: $required"
    }
  }
  $null = Invoke-Logged tar @('-xf', $makeArchive, '-C', $msys, '--exclude=.PKGINFO', '--exclude=.BUILDINFO', '--exclude=.MTREE') 'make-extract'
  $icuSource = Join-Path $work 'icu/source'
  $icuPrefix = Join-Path $work 'i'
  $icuBuild = Join-Path $work 'u'
  $data = Join-Path $icuSource 'data/in/icudt76l.dat'
  $dataOrigin = 'source-release'
  if (-not (Test-Path -LiteralPath $data -PathType Leaf)) {
    $dataHash = '1359ff28bad54f73fe29cc5c4fffb4c11c64399ddcc39bea2ee60b5d3672e7f79546a2255d604474cbd861791c11e62eb50bcecc0cf2cf9a7ece59180e8520fc'
    $dataArchive = Get-PinnedArchive 'https://github.com/unicode-org/icu/releases/download/release-76-1/icu4c-76_1-data-bin-l.zip' 'icu-data.zip' 12433427 'SHA512' $dataHash
    $dataExtract = Join-Path $work 'data-extract'
    [IO.Directory]::CreateDirectory($dataExtract) | Out-Null
    $null = Invoke-Logged tar @('-xf', $dataArchive, '-C', $dataExtract) 'icu-data-extract'
    $foundData = @(Get-ChildItem -LiteralPath $dataExtract -Recurse -File -Filter 'icudt76l.dat')
    if ($foundData.Count -ne 1) { throw 'Expected one matching little-endian ICU76 data file.' }
    [IO.Directory]::CreateDirectory((Split-Path -Parent $data)) | Out-Null
    Copy-Item -LiteralPath $foundData[0].FullName -Destination $data
    $dataOrigin = 'data-release'
  }
  $dataIdentity = Identity $data
  if ($dataIdentity.bytes -lt 1000000) { throw 'Refusing stub/empty ICU data input.' }
  $msvc = Split-Path -Parent (Get-Command cl -CommandType Application).Source
  foreach ($name in @('link', 'lib')) {
    if ((Split-Path -Parent (Get-Command $name -CommandType Application).Source) -cne $msvc) {
      throw "$name must resolve beside the active MSVC cl.exe."
    }
  }
  $sdkBin = Split-Path -Parent (Get-Command rc -CommandType Application).Source
  $scriptFile = Join-Path $work 'build-icu.sh'
  Write-Utf8 $scriptFile @'
#!/usr/bin/env bash
set -euo pipefail
# --noprofile/--norc excludes an existing MSYS setup. Convert only known roots.
work=$(/usr/bin/cygpath -u "$1")
msvc=$(/usr/bin/cygpath -u "$2")
sdk=$(/usr/bin/cygpath -u "$3")
windows=$(/usr/bin/cygpath -u "$4")
jobs=$5
export PATH="$msvc:$sdk:/usr/bin:$windows/System32:$windows"
unset ICU_DATA ICU_ROOT CMAKE_PREFIX_PATH CONFIG_SITE BASH_ENV ENV
for tool in cl.exe LINK.EXE LIB.EXE; do
  resolved=$(command -v "$tool")
  test "$(cygpath -aw "$resolved")" = "$(cygpath -aw "$msvc/$tool")" || {
    echo "Incorrect Microsoft tool resolution: $tool => $resolved" >&2; exit 1;
  }
  printf '%s=%s\n' "$tool" "$resolved"
done
command -v make
make --version
mkdir "$work/u"
cd "$work/u"
export CPPFLAGS='-D_ITERATOR_DEBUG_LEVEL=0'
export CFLAGS='-O2'
export CXXFLAGS='-O2 -std:c++17'
export LDFLAGS=''
bash "$work/icu/source/runConfigureICU" MSYS/MSVC --prefix="$work/i" \
  --enable-static --disable-shared --with-data-packaging=static \
  --disable-tests --disable-samples --disable-extras --enable-tools
grep -E '^(CC|CXX|CFLAGS|CXXFLAGS|CPPFLAGS|ENABLE_STATIC|ENABLE_SHARED|PKGDATA_MODE)[[:space:]]*=' icudefs.mk
grep -Fx 'include $(top_srcdir)/config/mh-msys-msvc' icudefs.mk
grep -E '^EXEEXT[[:space:]]*=[[:space:]]*\.exe[[:space:]]*$' icudefs.mk
grep -Fx '#EXTRA = extra' Makefile
grep -Fx 'TOOLS = tools' Makefile
grep -Fx 'DATASUBDIR = data' Makefile
make -j"$jobs"
make install
'@
  $priorMsystem = $env:MSYSTEM
  try {
    # ICU76's configure recognizes the MSYS/MSVC fragment from the mingw host
    # tuple. The narrow PATH and identity checks still select MSVC, never GCC.
    $env:MSYSTEM = 'MINGW64'
    $icuLog = Invoke-Logged (Join-Path $msys 'usr/bin/bash.exe') @('--noprofile', '--norc', $scriptFile.Replace('\', '/'),
      $work, $msvc, $sdkBin, $env:SystemRoot, "$Jobs") 'icu-build'
  } finally {
    $env:MSYSTEM = $priorMsystem
  }
  $defsPath = Join-Path $icuBuild 'icudefs.mk'
  $defs = Get-Content -LiteralPath $defsPath -Raw
  foreach ($entry in @(@('ENABLE_STATIC', 'YES'), @('ENABLE_SHARED', ''), @('PKGDATA_MODE', 'static'))) {
    if ($defs -notmatch "(?m)^$($entry[0])[ \t]*=[ \t]*$($entry[1])[ \t]*\r?$") { throw "ICU effective setting mismatch: $($entry[0])" }
  }
  foreach ($setting in @('CFLAGS', 'CXXFLAGS')) {
    $line = [regex]::Match($defs, "(?m)^$setting\s*=([^\r\n]+)").Groups[1].Value
    if ($line -notmatch '(?:^|\s)[-/]MD(?:\s|$)' -or $line -match '(?:^|\s)[-/](MTd?|MDd)(?:\s|$)') {
      throw "ICU effective $setting must use Release /MD."
    }
  }
  if ($defs -notmatch '_ITERATOR_DEBUG_LEVEL=0' -or $defs -notmatch 'std:c\+\+17') { throw 'Missing ICU C++ ABI settings.' }
  $unicode = Join-Path $stage 'icu-headers/unicode'
  Copy-Headers (Join-Path $icuPrefix 'include/unicode') $unicode
  $version = Get-Content -LiteralPath (Join-Path $unicode 'uvernum.h') -Raw
  if ($version -notmatch '(?m)^#define U_ICU_VERSION "76\.1"\s*$') { throw 'Installed ICU headers are not 76.1.' }
  Copy-Item -LiteralPath (Join-Path $work 'icu/LICENSE') -Destination (Join-Path $stage 'icu-headers/ICU-LICENSE')
  [IO.Directory]::CreateDirectory((Join-Path $stage 'icu-data')) | Out-Null
  Copy-Item -LiteralPath $data -Destination (Join-Path $stage 'icu-data/icudt76l.dat')
  $icuArchives = @(
    @('sicuuc.lib', 'icuuc.lib'), @('sicuin.lib', 'icui18n.lib'), @('sicudt.lib', 'icudata.lib')
  )
  foreach ($pair in $icuArchives) {
    $from = Join-Path $icuPrefix "lib/$($pair[0])"
    if (-not (Test-Path -LiteralPath $from -PathType Leaf)) { throw "Missing installed static ICU archive: $from" }
    Copy-Item -LiteralPath $from -Destination (Join-Path $stage "windows-static/$($pair[1])")
  }
  if ((Identity (Join-Path $stage 'windows-static/icudata.lib')).bytes -lt $dataIdentity.bytes) {
    throw 'Static data archive is smaller than its packaged input; possible stubdata library.'
  }
  $probeSource = Join-Path $work 'icu-probe.cpp'
  Write-Utf8 $probeSource @'
#include <unicode/dtptngen.h>
#include <unicode/dtfmtsym.h>
#include <unicode/timezone.h>
#include <unicode/udata.h>
#include <unicode/uclean.h>
#include <memory>
#include <iostream>
int main() {
  UErrorCode status = U_ZERO_ERROR;
  udata_setFileAccess(UDATA_NO_FILES, &status);
  u_init(&status);
  if (U_FAILURE(status)) return 1;
  std::unique_ptr<icu::DateTimePatternGenerator> gen(
      icu::DateTimePatternGenerator::createInstance(icu::Locale("fr_FR"), status));
  if (U_FAILURE(status) || !gen || gen->getBestPattern(u"yMMMMd", status).isEmpty()) return 2;
  icu::DateFormatSymbols symbols(icu::Locale("fr_FR"), status);
  int32_t count = 0;
  const auto* months = symbols.getMonths(count);
  if (U_FAILURE(status) || count < 12 || months[0] != icu::UnicodeString(u"janvier")) return 3;
  std::unique_ptr<icu::TimeZone> zone(icu::TimeZone::createTimeZone(u"Europe/Paris"));
  icu::UnicodeString id;
  zone->getID(id);
  if (id != icu::UnicodeString(u"Europe/Paris") || zone->getRawOffset() != 3600000) return 4;
  std::cout << "ICU76.1 static C++/French/Paris data passed with UDATA_NO_FILES\n";
  return 0;
}
'@
  $probe = Join-Path $work 'icu-probe.exe'
  $probeLog = Invoke-Logged (Join-Path $msvc 'cl.exe') @('/nologo', '/MD', '/EHsc', '/std:c++17', '/D_ITERATOR_DEBUG_LEVEL=0',
    '/DU_STATIC_IMPLEMENTATION', "/I$(Join-Path $stage 'icu-headers')", $probeSource, "/Fe:$probe", "/Fo:$(Join-Path $work 'icu-probe.obj')",
    (Join-Path $stage 'windows-static/icui18n.lib'), (Join-Path $stage 'windows-static/icuuc.lib'),
    (Join-Path $stage 'windows-static/icudata.lib'), 'advapi32.lib') 'icu-probe-link'
  $probeDependency = Assert-NoIcuDll $probe 'icu-probe-dependencies'
  $priorIcuData = $env:ICU_DATA
  try {
    $env:ICU_DATA = $null
    $probeRun = Invoke-Logged $probe @() 'icu-probe-run'
  } finally {
    $env:ICU_DATA = $priorIcuData
  }
  return [ordered]@{
    version = '76.1'; sourceCommit = '8eca245c7484ac6cc179e3e5f7c1ea7680810f39'
    sourceSha512 = $icuHash; linkage = 'static'; dataPath = 'icu-data/icudt76l.dat'
    extras = $false; toolsEnabled = $true; dataEnabled = $true
    dataIdentity = $dataIdentity; dataOrigin = $dataOrigin
    tools = [ordered]@{ msys2Sha256 = $msysHash; makeSha256 = $makeHash
      bash = (Identity (Join-Path $msys 'usr/bin/bash.exe')); make = (Identity (Join-Path $msys 'usr/bin/make.exe'))
      makeDependencies = $dependencies }
    build = [ordered]@{ prefix = $icuPrefix; sourceArchive = (Identity $icuArchive)
      script = (Identity $scriptFile); log = (Identity $icuLog); config = (Identity $defsPath)
      makefile = (Identity (Join-Path $icuBuild 'Makefile'))
      configLog = (Identity (Join-Path $icuBuild 'config.log'))
      probeSource = (Identity $probeSource); probe = (Identity $probe); probeLink = (Identity $probeLog)
      probeRun = (Identity $probeRun); probeDependencies = $probeDependency }
  }
}

# A kernel-owned lock prevents two provisioners publishing the same destination.
$lock = [IO.File]::Open("$install.build.lock", 'OpenOrCreate', 'ReadWrite', 'None')
try {
  if (Test-Path -LiteralPath $install) { throw "Refusing to replace existing install: $install" }
  $source = Join-Path $work 's'
  $build = Join-Path $work 'b'
  $archive = Join-Path $work 'source.tar'
  [IO.Directory]::CreateDirectory($source) | Out-Null
  $sourceMode = 'existing-git-object'
  if (-not $SourceRepository -and $env:LOCALAPPDATA) {
    $candidate = Join-Path $env:LOCALAPPDATA 'Exact/hermes-windows/hermes-src'
    if (Test-Path -LiteralPath (Join-Path $candidate '.git')) {
      $candidatePin = & git -C $candidate rev-parse --verify "$pin^{commit}" 2>$null
      if ($LASTEXITCODE -eq 0 -and "$candidatePin".Trim() -ceq $pin) { $SourceRepository = $candidate }
    }
  }
  if (-not $SourceRepository) {
    # Fetch into a new owned object database; never fetch/reset the old checkout.
    $sourceMode = 'fresh-git-fetch'
    $SourceRepository = Join-Path $work 'objects.git'
    $null = Invoke-Logged git @('init', '--bare', $SourceRepository) 'git-init'
    $null = Invoke-Logged git @('-C', $SourceRepository, 'fetch', '--depth=1', 'https://github.com/facebook/hermes.git', $pin) 'git-fetch'
  }
  $SourceRepository = [IO.Path]::GetFullPath($SourceRepository)
  $actualPin = & git -C $SourceRepository rev-parse --verify "$pin^{commit}"
  if ($LASTEXITCODE -ne 0 -or "$actualPin".Trim() -cne $pin) { throw 'Pinned Git commit verification failed.' }
  $null = Invoke-Logged git @('-C', $SourceRepository, 'archive', '--format=tar', "--output=$archive", $pin) 'git-archive'
  $archiveIdentity = Identity $archive
  $null = Invoke-Logged tar @('-xf', $archive, '-C', $source) 'source-extract'
  $versionHeader = Join-Path $source 'include/hermes/BCGen/HBC/BytecodeVersion.h'
  $versionMatch = [regex]::Match((Get-Content -LiteralPath $versionHeader -Raw), 'BYTECODE_VERSION\s*=\s*(\d+)\s*;')
  if (-not $versionMatch.Success -or $versionMatch.Groups[1].Value -ne '99') {
    throw 'The exported pin must declare bytecode version 99.'
  }

  # This verified base plus one declared portability patch is not pristine source.
  $cmakeSource = Join-Path $source 'CMakeLists.txt'
  $beforeHash = 'ae124a8b50f14fece21cccc059b82cfb5d493248bf3e817c2f1321a7b0f20beb'
  $afterHash = 'c8777d23ad355f34b2ed0a74f93a185641693ca4fc4da9c74085bc4e9a91903c'
  $oldClause = 'if ((NOT EMSCRIPTEN) AND target_type MATCHES "EXECUTABLE|STATIC_LIBRARY")'
  $newClause = 'if ((NOT EMSCRIPTEN) AND (NOT WIN32) AND target_type MATCHES "EXECUTABLE|STATIC_LIBRARY")'
  $cmakeText = [IO.File]::ReadAllText($cmakeSource)
  if ((Identity $cmakeSource).sha256 -cne $beforeHash -or
      [regex]::Matches($cmakeText, [regex]::Escape($oldClause)).Count -ne 1) {
    throw 'Pinned static ICU source patch does not match the verified base exactly once.'
  }
  Write-Utf8 $cmakeSource $cmakeText.Replace($oldClause, $newClause)
  if ((Identity $cmakeSource).sha256 -cne $afterHash) { throw 'Patched CMakeLists.txt identity mismatch.' }
  $patchText = $oldClause + "`n" + $newClause + "`n"
  $patchHash = [Convert]::ToHexString([Security.Cryptography.SHA256]::HashData([Text.Encoding]::UTF8.GetBytes($patchText))).ToLowerInvariant()
  $sourcePatch = [ordered]@{ path = 'CMakeLists.txt'; beforeSha256 = $beforeHash; afterSha256 = $afterHash
    old = $oldClause; new = $newClause; sha256 = $patchHash }

  # A fresh pin plus the reviewed internal Intl adapter/caller patch. All
  # preimages are exact, and git apply must accept the whole patch before writes.
  $intlPatch = Get-Content -LiteralPath (Join-Path $PSScriptRoot 'windows-intl.json') -Raw | ConvertFrom-Json
  $intlPatchFile = Join-Path $PSScriptRoot 'windows-intl.patch'
  if ((Identity $intlPatchFile).sha256 -cne $intlPatch.patchSha256) { throw 'Intl patch file identity mismatch.' }
  foreach ($entry in $intlPatch.sources) {
    if ((Identity (Join-Path $source $entry.path)).sha256 -cne $entry.beforeSha256) {
      throw "Intl pinned source preimage mismatch: $($entry.path)"
    }
  }
  $null = Invoke-Logged git @('-C', $source, 'apply', '--check', '--', $intlPatchFile) 'intl-patch-check'
  $null = Invoke-Logged git @('-C', $source, 'apply', '--', $intlPatchFile) 'intl-patch-apply'
  foreach ($entry in $intlPatch.sources) {
    if ((Identity (Join-Path $source $entry.path)).sha256 -cne $entry.afterSha256) {
      throw "Intl patched source identity mismatch: $($entry.path)"
    }
  }
  foreach ($entry in $intlPatch.fragments) {
    $fragment = Join-Path $PSScriptRoot ([IO.Path]::GetFileName($entry.path))
    if ((Identity $fragment).sha256 -cne $entry.sha256) { throw "Intl fragment identity mismatch: $fragment" }
    $destination = Join-Path $source $entry.path
    if (Test-Path -LiteralPath $destination) { throw "Intl fragment unexpectedly exists: $destination" }
    Copy-Item -LiteralPath $fragment -Destination $destination
  }
  if (Test-Path -LiteralPath $stage) { throw "Unique install stage already exists: $stage" }
  [IO.Directory]::CreateDirectory((Join-Path $stage 'windows-static')) | Out-Null
  $icu = Build-StaticIcu
  $icuRoot = $stage.Replace('\', '/')

  $configure = @(
    '-S', $source, '-B', $build, '-G', 'Ninja',
    '-DCMAKE_BUILD_TYPE=Release', '-DCMAKE_MSVC_RUNTIME_LIBRARY=MultiThreadedDLL',
    '-DCMAKE_C_COMPILER=cl', '-DCMAKE_CXX_COMPILER=cl',
    '-DCMAKE_C_FLAGS=/D_ITERATOR_DEBUG_LEVEL=0', '-DCMAKE_CXX_FLAGS=/D_ITERATOR_DEBUG_LEVEL=0',
    '-DCMAKE_EXPORT_COMPILE_COMMANDS=ON', '-DHERMES_ENABLE_DEBUGGER=OFF',
    '-DHERMESVM_ALLOW_JIT=0', '-DHERMES_ENABLE_INTL=ON',
    '-DHERMES_USE_STATIC_ICU=ON', '-DHERMES_ENABLE_WIN10_ICU_FALLBACK=OFF',
    "-DICU_ROOT=$icuRoot", "-DICU_INCLUDE_DIR=$icuRoot/icu-headers",
    "-DICU_UC_LIBRARY_RELEASE=$icuRoot/windows-static/icuuc.lib",
    "-DICU_I18N_LIBRARY_RELEASE=$icuRoot/windows-static/icui18n.lib",
    "-DICU_DATA_LIBRARY_RELEASE=$icuRoot/windows-static/icudata.lib",
    '-DHERMES_BUILD_APPLE_FRAMEWORK=OFF', '-DHERMES_BUILD_SHARED_JSI=OFF',
    '-DHERMES_ENABLE_TEST_SUITE=OFF', '-DHERMES_ENABLE_TOOLS=ON', '-DHERMES_MSVC_MP=OFF'
  )
  $cmakeVersionLog = Invoke-Logged cmake @('--version') 'cmake-version'
  $ninjaVersionLog = Invoke-Logged ninja @('--version') 'ninja-version'
  $configureLog = Invoke-Logged cmake $configure 'configure'
  $cacheFile = Join-Path $build 'CMakeCache.txt'
  $cache = Get-Content -LiteralPath $cacheFile -Raw
  foreach ($setting in @(
      'CMAKE_BUILD_TYPE:STRING=Release', 'CMAKE_MSVC_RUNTIME_LIBRARY:UNINITIALIZED=MultiThreadedDLL',
      'HERMES_ENABLE_DEBUGGER:BOOL=OFF', 'HERMESVM_ALLOW_JIT:STRING=0',
      'HERMES_ENABLE_INTL:BOOL=ON', 'HERMES_BUILD_SHARED_JSI:BOOL=OFF', 'HERMES_MSVC_MP:STRING=OFF',
      'HERMES_USE_STATIC_ICU:BOOL=ON', 'HERMES_ENABLE_WIN10_ICU_FALLBACK:BOOL=OFF',
      "ICU_INCLUDE_DIR:PATH=$icuRoot/icu-headers", "ICU_UC_LIBRARY_RELEASE:FILEPATH=$icuRoot/windows-static/icuuc.lib",
      "ICU_I18N_LIBRARY_RELEASE:FILEPATH=$icuRoot/windows-static/icui18n.lib", "ICU_DATA_LIBRARY_RELEASE:FILEPATH=$icuRoot/windows-static/icudata.lib")) {
    # CMake may assign a cache type to a command-line setting; its value is binding.
    $parts = $setting.Split('=', 2)
    $key = $parts[0].Split(':', 2)[0]
    if ($cache -notmatch "(?m)^$([regex]::Escape($key)):[^=]+=$([regex]::Escape($parts[1]))\r?$" ) {
      throw "Effective CMake setting differs: $setting"
    }
  }
  if ((Get-Content -LiteralPath $configureLog -Raw) -notmatch 'Found ICU:.*76\.1') {
    throw 'Hermes configure did not report the required found ICU76.1 dependency.'
  }
  $commandsPath = Join-Path $build 'compile_commands.json'
  $commands = @(Get-Content -LiteralPath $commandsPath -Raw | ConvertFrom-Json)
  if ($commands.Count -eq 0) { throw 'CMake emitted no compile commands.' }
  foreach ($entry in $commands) {
    if ([IO.Path]::GetExtension($entry.file) -notin @('.c', '.cc', '.cpp', '.cxx')) { continue }
    if ($entry.command -notmatch '(?i)(?:^|\s)[-/]MD(?:\s|$)' -or
        $entry.command -match '(?i)(?:^|\s)[-/](MTd?|MDd)(?:\s|$)' -or
        $entry.command -notmatch '_ITERATOR_DEBUG_LEVEL=0(?:\s|$)') {
      throw "Effective compiler flags violate Release /MD iterator=0: $($entry.file)"
    }
  }
  $null = Invoke-Logged cmake @('--build', $build, '--target', 'hermesvmlean_a', 'jsi', 'boost_context', 'hermesc', '--parallel', "$Jobs") 'build'

  $headers = Join-Path $stage 'hermes-headers'
  Copy-Headers (Join-Path $source 'API/jsi/jsi') (Join-Path $headers 'jsi')
  Copy-Headers (Join-Path $source 'API/hermes') (Join-Path $headers 'hermes')
  Copy-Headers (Join-Path $source 'public/hermes/Public') (Join-Path $headers 'hermes/Public')
  Copy-Item -LiteralPath (Join-Path $source 'LICENSE') -Destination (Join-Path $headers 'HERMES-LICENSE')
  $archives = @('hermesvmlean_a.lib', 'jsi.lib', 'boost_context.lib', 'icuuc.lib', 'icui18n.lib', 'icudata.lib')
  $directiveEvidence = @()
  foreach ($name in $archives) {
    $destination = Join-Path $stage "windows-static/$name"
    if ($name -notlike 'icu*') {
      $found = @(Get-ChildItem -LiteralPath $build -Recurse -File -Filter $name)
      if ($found.Count -ne 1) { throw "Expected exactly one $name; found $($found.Count)." }
      Copy-Item -LiteralPath $found[0].FullName -Destination $destination
    }
    $directives = Invoke-Logged dumpbin @('/nologo', '/directives', $destination) "directives-$name" $true
    $text = Get-Content -LiteralPath $directives -Raw
    # The generated data-only object has no CRT/iterator references of its own.
    if (($name -ne 'icudata.lib' -and $text -notmatch '(?i)DEFAULTLIB:\s*"?MSVCRT(?:"|\s|$)') -or
        $text -match '(?i)DEFAULTLIB:\s*"?(LIBCMTD?|MSVCRTD)(?:"|\s|$)' -or
        $text -match '_ITERATOR_DEBUG_LEVEL=[1-9]') {
      throw "Archive directives violate Release /MD iterator=0: $name"
    }
    $directiveEvidence += [ordered]@{ archive = $name; log = $directives; identity = (Identity $directives) }
  }
  $compiler = Join-Path $stage 'hermesc.exe'
  Copy-Item -LiteralPath (Join-Path $build 'bin/hermesc.exe') -Destination $compiler
  $pe = [IO.File]::ReadAllBytes($compiler)
  $peOffset = [BitConverter]::ToInt32($pe, 0x3c)
  if ([BitConverter]::ToUInt32($pe, $peOffset) -ne 0x4550 -or
      [BitConverter]::ToUInt16($pe, $peOffset + 4) -ne 0x8664) { throw 'hermesc must be an x64 PE.' }
  $versionLog = Invoke-Logged $compiler @('-version') 'hermesc-version'
  $compilerDependencies = Assert-NoIcuDll $compiler 'hermesc-dependencies'
  $compilerVersion = Get-Content -LiteralPath $versionLog -Raw
  if ($compilerVersion -notmatch '(?im)HBC bytecode version:\s*99\s*$') {
    throw 'Built hermesc does not report bytecode version 99.'
  }
  foreach ($required in @('hermes/hermes.h', 'jsi/jsi.h')) {
    if (-not (Test-Path -LiteralPath (Join-Path $headers $required) -PathType Leaf)) {
      throw "Missing public header: $required"
    }
  }

  # This probe executes actual HBC through the newly built lean VM. It cannot
  # borrow ICU files/DLLs or evaluate source. Probe artifacts remain in the
  # retained build directory, with their exact identities in the install receipt.
  $intlJs = Join-Path $PSScriptRoot 'windows-intl-probe.js'
  $intlRunner = Join-Path $PSScriptRoot 'windows-intl-probe.cc'
  if ((Identity $intlJs).sha256 -cne $intlPatch.probe.sourceSha256 -or
      (Identity $intlRunner).sha256 -cne $intlPatch.probe.runnerSha256) { throw 'Intl probe source identity mismatch.' }
  $intlHbc = Join-Path $work 'intl-probe.hbc'
  $intlExe = Join-Path $work 'intl-probe.exe'
  $null = Invoke-Logged $compiler @('-O', '-emit-binary', '-out', $intlHbc, $intlJs) 'intl-probe-bytecode'
  $intlLibs = @('hermesvmlean_a', 'jsi', 'boost_context', 'icuuc', 'icui18n', 'icudata') | ForEach-Object {
    Join-Path $stage "windows-static/$_.lib"
  }
  $intlLink = Invoke-Logged cl (@('/nologo', '/std:c++20', '/EHsc', '/MD', '/O2', '/DNDEBUG',
    '/D_ITERATOR_DEBUG_LEVEL=0', '/DSTATIC_HERMES', '/DU_STATIC_IMPLEMENTATION',
    "/I$headers", "/I$(Join-Path $stage 'icu-headers')", $intlRunner,
    "/Fo:$(Join-Path $work 'intl-probe.obj')", "/Fe:$intlExe") + $intlLibs +
    @('dbghelp.lib', 'version.lib', 'psapi.lib', 'winmm.lib', 'advapi32.lib', '/link', "/MAP:$(Join-Path $work 'intl-probe.map')")) 'intl-probe-link'
  $intlDependencies = Assert-NoIcuDll $intlExe 'intl-probe-dependencies'
  $priorIcuData = $env:ICU_DATA
  Push-Location $work
  try {
    $env:ICU_DATA = $null
    $intlOutput = Invoke-Logged $intlExe @($intlHbc) 'intl-probe-run'
  } finally {
    $env:ICU_DATA = $priorIcuData
    Pop-Location
  }
  $intlResult = Get-Content -LiteralPath $intlOutput -Raw | ConvertFrom-Json
  if ($intlResult.assertions -ne $intlPatch.probe.assertions) { throw 'Intl probe assertion count differs from reviewed qualification.' }
  $intlProbe = [ordered]@{ assertions = $intlResult.assertions
    source = (Identity $intlJs); runner = (Identity $intlRunner); bytecode = (Identity $intlHbc)
    compiler = (Identity $compiler); executable = (Identity $intlExe); output = (Identity $intlOutput)
    result = $intlResult; link = (Identity $intlLink); map = (Identity (Join-Path $work 'intl-probe.map'))
    dependencies = $intlDependencies }

  [string[]]$names = @(Get-ChildItem -LiteralPath $stage -Recurse -File | ForEach-Object {
    [IO.Path]::GetRelativePath($stage, $_.FullName).Replace('\', '/')
  })
  [Array]::Sort($names, [StringComparer]::Ordinal)
  $files = @($names | ForEach-Object {
    $identity = Identity (Join-Path $stage $_)
    [ordered]@{ path = $_; sha256 = $identity.sha256; bytes = $identity.bytes }
  })
  $receipt = [ordered]@{
    schema = 'exact/hermes-windows-lean/3'; sourceCommit = $pin; target = $target
    role = 'lean'; bytecodeVersion = 99; crt = 'MD'; iteratorDebugLevel = 0
    debugger = $false; jit = $false; intl = $true
    systemLibraries = @('dbghelp', 'version', 'psapi', 'winmm', 'advapi32')
    files = $files; icu = $icu
    source = [ordered]@{ mode = $sourceMode; repository = $SourceRepository; archive = $archive; identity = $archiveIdentity }
    build = [ordered]@{
      completedUtc = [DateTime]::UtcNow.ToString('o'); work = $work; jobs = $Jobs
      sourcePatch = $sourcePatch; intlPatch = $intlPatch; intlProbe = $intlProbe
      compilerDependencies = $compilerDependencies
      configureArguments = $configure; cache = (Identity $cacheFile)
      compileCommands = (Identity $commandsPath); compileCommandCount = $commands.Count
      compilerVersion = $compilerVersion; archiveDirectives = $directiveEvidence
      msvc = (Get-Command cl -CommandType Application).Source
      msvcVersion = (Get-Item -LiteralPath (Get-Command cl -CommandType Application).Source).VersionInfo.FileVersion
      cmake = (Get-Command cmake -CommandType Application).Source
      cmakeVersion = (Get-Content -LiteralPath $cmakeVersionLog -Raw).Trim()
      ninja = (Get-Command ninja -CommandType Application).Source
      ninjaVersion = (Get-Content -LiteralPath $ninjaVersionLog -Raw).Trim()
      windowsSdkVersion = $env:WindowsSDKVersion
    }
    installation = [ordered]@{ requestedPath = $requestedInstall; resolvedPath = $install }
  }
  $receiptFile = Join-Path $stage 'hermes-input-receipt.json'
  [IO.File]::WriteAllText($receiptFile, ($receipt | ConvertTo-Json -Depth 12) + "`n", [Text.UTF8Encoding]::new($false))
  # Re-read every payload byte before publication. The Cargo resolver independently
  # checks the schema, exact inventory and identities before any linking or baking.
  foreach ($entry in $files) {
    $actual = Identity (Join-Path $stage $entry.path)
    if ($actual.bytes -ne $entry.bytes -or $actual.sha256 -cne $entry.sha256) {
      throw "Install payload changed while staging: $($entry.path)"
    }
  }
  if (Test-Path -LiteralPath $install) { throw "Install appeared during build; preserved: $install" }
  # Same-parent rename, no replacement. Neither failed stages nor old installs are deleted.
  [IO.Directory]::Move($stage, $install)
  $actualInstall = [ExactHermesBuildPath]::Resolve($install)
  if ($actualInstall -cne $install) { throw "Published path changed: $actualInstall (expected $install)" }
  Write-Host "Installed pinned lean Hermes: $actualInstall"
  Write-Host "Receipt SHA-256: $((Identity (Join-Path $actualInstall 'hermes-input-receipt.json')).sha256)"
  Write-Host "For an explicit build selection: `$env:EXACT_HERMES_DIR = '$($actualInstall.Replace("'", "''"))'"
} catch {
  Write-Host "Build failed; owned work is retained at $work; any install stage is retained at $stage"
  throw
} finally {
  $lock.Dispose()
}
