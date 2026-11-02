/-
libm 0.2.16's sin, cos, k_sin, k_cos, rem_pio2 and rem_pio2_large
(the binary64, prec = 1 path), over the exact F64 operations. Constants
are their binary64 bits; each authored operation rounds separately.
The floating-point exception flags raised by force_eval are not values.

Origin: FreeBSD /usr/src/lib/msun/src/{s_sin,s_cos,k_sin,k_cos,e_rem_pio2,k_rem_pio2}.c
Copyright (C) 1993 by Sun Microsystems, Inc. All rights reserved.
Developed at SunPro / SunSoft, a Sun Microsystems, Inc. business.
Permission to use, copy, modify, and distribute this software is freely
granted, provided that this notice is preserved.
-/
import Contract.Binary64

namespace Contract.F64
namespace Trig

private def half : F64 := ofBits 0x3fe0000000000000
private def two24 : F64 := ofBits 0x4170000000000000
private def twoNeg24 : F64 := ofBits 0x3e70000000000000
private def pio2_1 : F64 := ofBits 0x3ff921fb54400000
private def pio2_1t : F64 := ofBits 0x3dd0b4611a626331

private def kernelSin (x y : F64) (tail : Bool) : F64 :=
  let s1 := ofBits 0xbfc5555555555549
  let s2 := ofBits 0x3f8111111110f8a6
  let s3 := ofBits 0xbf2a01a019c161d5
  let s4 := ofBits 0x3ec71de357b1fe7d
  let s5 := ofBits 0xbe5ae5e68a2b9ceb
  let s6 := ofBits 0x3de5d93a5acfd57c
  let z := x * x
  let w := z * z
  let r := s2 + z * (s3 + z * s4) + z * w * (s5 + z * s6)
  let v := z * x
  if !tail then x + v * (s1 + z * r)
  else x - ((z * (half * y - v * r) - y) - v * s1)

private def kernelCos (x y : F64) : F64 :=
  let c1 := ofBits 0x3fa555555555554c
  let c2 := ofBits 0xbf56c16c16c15177
  let c3 := ofBits 0x3efa01a019cb1590
  let c4 := ofBits 0xbe927e4f809c52ad
  let c5 := ofBits 0x3e21ee9ebdb4b1c4
  let c6 := ofBits 0xbda8fae9be8838d4
  let z := x * x
  let w := z * z
  let r := z * (c1 + z * (c2 + z * c3)) + w * w * (c4 + z * (c5 + z * c6))
  let hz := half * z
  let w := 1 - hz
  w + (((1 - w) - hz) + (z * r - x * y))

private def toInt (x : F64) : Int := x.scaled.tdiv (2 ^ 1074)

/-- scalbn on the finite values used during reduction, rounded once. -/
private def scale (x : F64) (n : Int) : F64 :=
  if n ≥ 0 then round x.sign (x.mag * 2 ^ n.toNat) 1
  else round x.sign x.mag (2 ^ (-n).toNat)

private def medium (x : F64) : Int × F64 × F64 := Id.run do
  let invpio2 := ofBits 0x3fe45f306dc9c883
  let toint := ofBits 0x4338000000000000
  let fn := (x * invpio2 + toint) - toint
  let mut r := x - fn * pio2_1
  let mut w := fn * pio2_1t
  let mut y0 := r - w
  if x.bexp > y0.bexp + 16 then
    let t := r
    w := fn * ofBits 0x3dd0b4611a600000
    r := t - w
    w := fn * ofBits 0x3ba3198a2e037073 - ((t - r) - w)
    y0 := r - w
    if x.bexp > y0.bexp + 49 then
      let t := r
      w := fn * ofBits 0x3ba3198a2e000000
      r := t - w
      w := fn * ofBits 0x397b839a252049c1 - ((t - r) - w)
      y0 := r - w
  return (toInt fn, y0, (r - y0) - w)

/-- The first 66 base-2^24 digits of 2/pi, sufficient for binary64
(e0 ≤ 1000). libm's longer table also serves wider formats. -/
private def ipio2 : Array Nat := #[
  0xA2F983, 0x6E4E44, 0x1529FC, 0x2757D1, 0xF534DD, 0xC0DB62, 0x95993C, 0x439041, 0xFE5163,
  0xABDEBB, 0xC561B7, 0x246E3A, 0x424DD2, 0xE00649, 0x2EEA09, 0xD1921C, 0xFE1DEB, 0x1CB129,
  0xA73EE8, 0x8235F5, 0x2EBB44, 0x84E99C, 0x7026B4, 0x5F7E41, 0x3991D6, 0x398353, 0x39F49C,
  0x845F8B, 0xBDF928, 0x3B1FF8, 0x97FFDE, 0x05980F, 0xEF2F11, 0x8B5A0A, 0x6D1F6D, 0x367ECF,
  0x27CB09, 0xB74F46, 0x3F669E, 0x5FEA2D, 0x7527BA, 0xC7EBE5, 0xF17B3D, 0x0739F7, 0x8A5292,
  0xEA6BFB, 0x5FB11F, 0x8D5D08, 0x560330, 0x46FC7B, 0x6BABF0, 0xCFBC20, 0x9AF436, 0x1DA9E3,
  0x91615E, 0xE61B08, 0x659985, 0x5F14A0, 0x68408D, 0xFFD880, 0x4D7327, 0x310606, 0x1556CA,
  0x73A8C9, 0x60E27B, 0xC08C6B]

private def pio2 : Array F64 := #[
  ofBits 0x3ff921fb40000000, ofBits 0x3e74442d00000000,
  ofBits 0x3cf8469880000000, ofBits 0x3b78cc5160000000,
  ofBits 0x39f01b8380000000, ofBits 0x387a252040000000,
  ofBits 0x36e3822280000000, ofBits 0x3569f31d00000000]

private def large (x : Array F64) (e0 : Int) : Int × F64 × F64 := Id.run do
  let jk := 4
  let jx := x.size - 1
  let jv := ((e0 - 3).tdiv 24).toNat
  let mut q0 := e0 - 24 * (jv + 1 : Nat)
  let mut f : Array F64 := Array.replicate 20 0
  let mut q : Array F64 := Array.replicate 20 0
  let mut iq : Array Nat := Array.replicate 20 0
  for i in [:jx + jk + 1] do
    let j := (jv : Int) - jx + i
    f := f.set! i (if j < 0 then 0 else F64.ofNat ipio2[j.toNat]!)
  for i in [:jk + 1] do
    let mut fw : F64 := 0
    for j in [:jx + 1] do
      fw := fw + x[j]! * f[jx + i - j]!
    q := q.set! i fw
  let mut jz := jk
  let mut n : Nat := 0
  let mut ih : Nat := 0
  let mut z : F64 := 0
  -- Each recomputation adds at least one term to libm's 20-entry buffers.
  -- Exhausting them is outside that implementation's defined array accesses.
  for _ in [:20] do
    z := q[jz]!
    for i in [:jz] do
      let fw := F64.ofNat (twoNeg24 * z).toNat
      iq := iq.set! i (z - two24 * fw).toNat
      z := q[jz - i - 1]! + fw
    z := scale z q0
    z := z - 8 * (z * ofBits 0x3fc0000000000000).floor
    n := z.toNat
    z := z - F64.ofNat n
    ih := 0
    if q0 > 0 then
      let i := iq[jz - 1]! >>> (24 - q0).toNat
      n := n + i
      iq := iq.set! (jz - 1) (iq[jz - 1]! - (i <<< (24 - q0).toNat))
      ih := iq[jz - 1]! >>> (23 - q0).toNat
    else if q0 == 0 then
      ih := iq[jz - 1]! >>> 23
    else if z ≥ half then
      ih := 2
    if ih > 0 then
      n := n + 1
      let mut carry := false
      for i in [:jz] do
        let j := iq[i]!
        if !carry then
          if j != 0 then
            carry := true
            iq := iq.set! i (0x1000000 - j)
        else
          iq := iq.set! i (0xffffff - j)
      if q0 == 1 then iq := iq.set! (jz - 1) (iq[jz - 1]! &&& 0x7fffff)
      if q0 == 2 then iq := iq.set! (jz - 1) (iq[jz - 1]! &&& 0x3fffff)
      if ih == 2 then
        z := 1 - z
        if carry then z := z - scale 1 q0
    if z == 0 && (List.range (jz - jk)).all (fun i => iq[jk + i]! == 0) then
      let k := 1 + ((List.range jk).takeWhile (fun i => iq[jk - 1 - i]! == 0)).length
      if jx + jz + k ≥ 20 then return (0, nan, nan)
      for i in [jz + 1:jz + k + 1] do
        f := f.set! (jx + i) (F64.ofNat ipio2[jv + i]!)
        let mut fw : F64 := 0
        for j in [:jx + 1] do
          fw := fw + x[j]! * f[jx + i - j]!
        q := q.set! i fw
      jz := jz + k
    else
      break
  if z == 0 then
    jz := jz - 1
    q0 := q0 - 24
    for _ in [:jz] do
      if iq[jz]! != 0 then break
      jz := jz - 1
      q0 := q0 - 24
  else
    z := scale z (-q0)
    if z ≥ two24 then
      let fw := F64.ofNat (twoNeg24 * z).toNat
      iq := iq.set! jz (z - two24 * fw).toNat
      jz := jz + 1
      q0 := q0 + 24
      iq := iq.set! jz fw.toNat
    else
      iq := iq.set! jz z.toNat
  let mut fw := scale 1 q0
  for k in [:jz + 1] do
    let i := jz - k
    q := q.set! i (fw * F64.ofNat iq[i]!)
    fw := fw * twoNeg24
  let mut fq : Array F64 := Array.replicate 20 0
  for j in [:jz + 1] do
    let i := jz - j
    fw := 0
    for k in [:min jk (jz - i) + 1] do
      fw := fw + pio2[k]! * q[i + k]!
    fq := fq.set! (jz - i) fw
  fw := 0
  for k in [:jz + 1] do fw := fw + fq[jz - k]!
  let y0 := if ih == 0 then fw else -fw
  fw := fq[0]! - fw
  for i in [1:jz + 1] do fw := fw + fq[i]!
  return (Int.ofNat (n % 8), y0, if ih == 0 then fw else -fw)

private def small (x : F64) (n : Nat) : Int × F64 × F64 :=
  let fn := F64.ofNat n
  if x.sign then
    let z := x + fn * pio2_1
    let y0 := z + fn * pio2_1t
    (-(n : Int), y0, (z - y0) + fn * pio2_1t)
  else
    let z := x - fn * pio2_1
    let y0 := z - fn * pio2_1t
    (n, y0, (z - y0) - fn * pio2_1t)

private def remPio2 (x : F64) : Int × F64 × F64 := Id.run do
  let ix := (x.word >>> 32) &&& 0x7fffffff
  if ix ≤ 0x400f6a7a then
    if ix &&& 0xfffff == 0x921fb then return medium x
    return small x (if ix ≤ 0x4002d97c then 1 else 2)
  if ix ≤ 0x401c463b then
    if ix ≤ 0x4015fdbc then
      if ix == 0x4012d97c then return medium x
      return small x 3
    if ix == 0x401921fb then return medium x
    return small x 4
  if ix < 0x413921fb then return medium x
  if ix ≥ 0x7ff00000 then return (0, nan, nan)
  let mut z := ofBits (UInt64.ofNat (x.frac + (0x3ff + 23) * 2 ^ 52))
  let mut tx : Array F64 := #[]
  for _ in [:2] do
    let t := F64.ofNat z.toNat
    tx := tx.push t
    z := (z - t) * two24
  tx := tx.push z
  for _ in [:2] do
    if tx.back! == 0 then tx := tx.pop else break
  let (n, y0, y1) := large tx ((x.bexp : Int) - (0x3ff + 23))
  return if x.sign then (-n, -y0, -y1) else (n, y0, y1)

end Trig

/-- Sine in radians, with the bits of the runner's pinned libm 0.2.16. -/
def sin (x : F64) : F64 :=
  let ix := (x.word >>> 32) &&& 0x7fffffff
  if ix ≤ 0x3fe921fb then
    if ix < 0x3e500000 then x else Trig.kernelSin x 0 false
  else if ix ≥ 0x7ff00000 then nan
  else
    let (n, y0, y1) := Trig.remPio2 x
    match (n % 4).toNat with
    | 0 => Trig.kernelSin y0 y1 true
    | 1 => Trig.kernelCos y0 y1
    | 2 => -(Trig.kernelSin y0 y1 true)
    | _ => -(Trig.kernelCos y0 y1)

/-- Cosine in radians, with the bits of the runner's pinned libm 0.2.16. -/
def cos (x : F64) : F64 :=
  let ix := (x.word >>> 32) &&& 0x7fffffff
  if ix ≤ 0x3fe921fb then
    if ix < 0x3e46a09e then 1 else Trig.kernelCos x 0
  else if ix ≥ 0x7ff00000 then nan
  else
    let (n, y0, y1) := Trig.remPio2 x
    match (n % 4).toNat with
    | 0 => Trig.kernelCos y0 y1
    | 1 => -(Trig.kernelSin y0 y1 true)
    | 2 => -(Trig.kernelCos y0 y1)
    | _ => Trig.kernelSin y0 y1 true

end Contract.F64
