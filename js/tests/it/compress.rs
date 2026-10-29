//! `storage.fs.compressImage` from TypeScript (LLP 1069.002 Amendment A1),
//! over the storage fixture module (`tests/fixtures/storage.ts`).
#![cfg(exact_js_engine)]
use super::storage::{call, Root, GRANTS};
use exact_runner::{DataSource, Store};

#[cfg(target_vendor = "apple")]
use {
    super::storage::args,
    exact_runner::{Answer, Dispatch, Work},
};

/// `storage.fs.compressImage` from TypeScript (LLP 1069.002 A1): Hermes,
/// the prelude, ibex2's opcode 121 and the platform codec. On Apple the
/// fixture comes back upright as a JPEG; elsewhere the call is refused as
/// `unsupported`. Either way each refusal carries its code.
#[test]
fn compress_image_writes_an_upright_jpeg_or_refuses_by_name() {
    let root = Root::new();
    let mut m = root.module();
    m.activate().unwrap();
    let mut s = Store::new(GRANTS, Vec::<(String, Vec<u8>)>::new());
    assert_eq!(call(&mut m, &mut s, "file", "x"), "x"); // app:/data/note, a text file
    std::fs::write(
        root.0.join("data/in.jpg"),
        include_bytes!("../../../scripts/fixtures/picker/oriented-gps.jpg"),
    )
    .unwrap();
    let text = call(&mut m, &mut s, "compress", "4000");
    let lines: Vec<&str> = text.lines().collect();
    let code = |l: &str| l.split(' ').take(2).collect::<Vec<_>>().join(" ");
    let refusals: Vec<String> = lines[1..].iter().map(|l| code(l)).collect();
    if cfg!(target_vendor = "apple") {
        let size = std::fs::metadata(root.0.join("data/out.jpg"))
            .unwrap()
            .len();
        assert_eq!(
            lines[0],
            format!("app:/data/out.jpg image/jpeg {size} 48 64 {size}")
        );
        assert_eq!(
            refusals,
            [
                "Error denied",
                "Error ENOENT",
                "Error undecodable",
                "Error unfit",
                "Error failed",
                "TypeError undefined",
                "TypeError undefined"
            ],
            "{text}"
        );
        assert!(lines[4].contains("compressImage: unfit: "), "{text}");
        assert!(
            lines[5].contains("compressImage: requires bounded app:/ paths"),
            "{text}"
        );
    } else {
        assert!(
            lines[0].starts_with("Error unsupported compressImage: unsupported: "),
            "{text}"
        );
        // Admission still denies an unauthorized destination first. With no
        // installed codec, every admitted call refuses before reading its
        // source or asking the provider to interpret a path.
        assert_eq!(
            refusals,
            [
                "Error denied",
                "Error unsupported",
                "Error unsupported",
                "Error unsupported",
                "Error unsupported",
                "TypeError undefined",
                "TypeError undefined"
            ],
            "{text}"
        );
        assert!(
            lines[2..=5]
                .iter()
                .all(|line| line.contains("no image codec")),
            "{text}"
        );
    }
    assert!(
        lines[6].contains("maxDimension must be an integer from 1 to 8192"),
        "{text}"
    );
    assert!(
        lines[7].contains("options must be {maxDimension, maxBytes}"),
        "{text}"
    );
}

/// `storage.fs.transformImage` and `compressImage`'s edit (LLP 1133.019 D14,
/// D15) from TypeScript, over the JSON wire (D17): on Apple the record says
/// what was written, a `to` without an extension gains the type's, and the
/// search runs on the edited picture; elsewhere `unsupported`. Each refusal
/// carries its code, and a bad option is a `TypeError` before anything runs.
#[test]
fn transform_image_writes_the_edit_or_refuses_by_name() {
    let root = Root::new();
    let mut m = root.module();
    m.activate().unwrap();
    let mut s = Store::new(GRANTS, Vec::<(String, Vec<u8>)>::new());
    std::fs::write(
        root.0.join("data/in.jpg"),
        include_bytes!("../../../scripts/fixtures/picker/oriented-gps.jpg"),
    )
    .unwrap();
    let text = call(&mut m, &mut s, "transform", "");
    let lines: Vec<&str> = text.lines().collect();
    let size = |name: &str| {
        std::fs::metadata(root.0.join("data").join(name))
            .map(|m| m.len())
            .map_err(|e| format!("{name}: {e}: {text}"))
    };
    if cfg!(target_vendor = "apple") {
        let turned = size("turned.jpg").unwrap();
        assert_eq!(
            lines[0],
            format!("app:/data/turned.jpg image/jpeg {turned} 64 48 srgb false keep {turned}")
        );
        let private = size("private.png").unwrap();
        assert_eq!(
            lines[1],
            format!(
                "app:/data/private.png image/png {private} 24 32 srgb false no-location {private}"
            )
        );
        let crop = size("crop.jpg").unwrap();
        assert_eq!(
            lines[2],
            format!("app:/data/crop.jpg image/jpeg {crop} 64 24    {crop}")
        );
        assert!(
            lines[3].starts_with(
                "Error outside transformImage: outside: the crop misses the 48×64 image"
            ),
            "{text}"
        );
        assert!(
            lines[4].starts_with("Error unsupported transformImage: unsupported: "),
            "{text}"
        );
    } else {
        for line in &lines[..5] {
            assert!(line.starts_with("Error unsupported"), "{text}");
        }
        assert!(size("turned.jpg").is_err());
    }
    assert!(lines[5].starts_with("Error denied"), "{text}");
    assert!(
        lines[6].starts_with("TypeError undefined")
            && lines[6].contains("flip must be \"horizontal\" or \"vertical\""),
        "{text}"
    );
    assert!(
        lines[7].starts_with("TypeError undefined") && lines[7].contains("crop must be"),
        "{text}"
    );
}

/// `type: "image/png"` (LLP 1069.002 A1.11) through the binding's fifth
/// argument: on Apple a PNG with an alpha channel (IHDR colour type 6) at the
/// fitted size; elsewhere `unsupported`. Any other type is a `TypeError`.
#[test]
fn compress_image_writes_a_png_with_alpha_or_refuses_by_name() {
    let root = Root::new();
    let mut m = root.module();
    m.activate().unwrap();
    let mut s = Store::new(GRANTS, Vec::<(String, Vec<u8>)>::new());
    std::fs::write(root.0.join("data/in.png"), half_clear_png(16, 16)).unwrap();
    let text = call(&mut m, &mut s, "compress-png", "8");
    let lines: Vec<&str> = text.lines().collect();
    if cfg!(target_vendor = "apple") {
        let out = std::fs::read(root.0.join("data/out.png")).unwrap();
        let head: Vec<String> = out[..26].iter().map(u8::to_string).collect();
        assert_eq!(
            lines[0],
            format!(
                "app:/data/out.png image/png {} 8 8 {}",
                out.len(),
                head.join(",")
            )
        );
        assert!(out.starts_with(b"\x89PNG\r\n\x1a\n"), "{text}");
        assert_eq!(out[25], 6, "an RGBA PNG: {text}");
    } else {
        assert!(
            lines[0].starts_with("Error unsupported compressImage: unsupported: "),
            "{text}"
        );
    }
    assert!(
        lines[1].starts_with("TypeError undefined")
            && lines[1].contains("type must be \"image/jpeg\" or \"image/png\""),
        "{text}"
    );
}

/// A `w × h` RGBA PNG, clear on its left half and opaque red on its right,
/// in stored (uncompressed) deflate blocks.
fn half_clear_png(w: u32, h: u32) -> Vec<u8> {
    fn crc(bytes: &[u8]) -> u32 {
        let mut c = !0u32;
        for &b in bytes {
            c ^= u32::from(b);
            for _ in 0..8 {
                c = if c & 1 == 1 {
                    0xEDB8_8320 ^ (c >> 1)
                } else {
                    c >> 1
                };
            }
        }
        !c
    }
    let chunk = |png: &mut Vec<u8>, kind: &[u8], data: &[u8]| {
        png.extend_from_slice(&(data.len() as u32).to_be_bytes());
        let body = [kind, data].concat();
        png.extend_from_slice(&body);
        png.extend_from_slice(&crc(&body).to_be_bytes());
    };
    let mut raw = Vec::new();
    for _ in 0..h {
        raw.extend([0]); // filter: none (one byte a row, then its pixels)
        for x in 0..w {
            raw.extend_from_slice(if x < w / 2 {
                &[0, 0, 0, 0]
            } else {
                &[255, 0, 0, 255]
            });
        }
    }
    let mut zlib = vec![0x78, 0x01];
    for (i, block) in raw.chunks(65535).enumerate() {
        let last = (i + 1) * 65535 >= raw.len();
        zlib.push(u8::from(last));
        let n = block.len() as u16;
        zlib.extend_from_slice(&n.to_le_bytes());
        zlib.extend_from_slice(&(!n).to_le_bytes());
        zlib.extend_from_slice(block);
    }
    let (mut a, mut b) = (1u32, 0u32);
    for &byte in &raw {
        a = (a + u32::from(byte)) % 65521;
        b = (b + a) % 65521;
    }
    zlib.extend_from_slice(&((b << 16) | a).to_be_bytes());
    let mut png = b"\x89PNG\r\n\x1a\n".to_vec();
    let ihdr = [&w.to_be_bytes()[..], &h.to_be_bytes(), &[8, 6, 0, 0, 0]].concat();
    chunk(&mut png, b"IHDR", &ihdr);
    chunk(&mut png, b"IDAT", &zlib);
    chunk(&mut png, b"IEND", &[]);
    png
}

/// A wait that gives up on an `fs.compressImage` takes its right to write
/// away (LLP 1069.002 A1.5): the answer fails `timeout`, the queue moves on
/// to a write of the same path, and the compression, still running, never
/// writes over it. Each of these tests holds its compressions, so the wait
/// on one runs out while it runs.
#[cfg(target_vendor = "apple")]
#[test]
fn a_compression_the_wait_gave_up_on_never_writes() {
    let root = Root::new();
    let mut m = root.module();
    let hold = m.hold_compressions();
    m.activate().unwrap();
    let mut s = Store::new(GRANTS, Vec::<(String, Vec<u8>)>::new());
    assert_eq!(call(&mut m, &mut s, "file", "x"), "x");
    std::fs::write(root.0.join("data/noise.bmp"), noise_bmp()).unwrap();
    // The wait gives up: the answer fails, and its failure names `timeout`.
    let a = args("compress-abandoned", "");
    let Answer::Later(request) = m.answer(&mut s, "work", &a).unwrap() else {
        panic!("the compression waits on storage")
    };
    let Dispatch::Run(Work::Now(work)) =
        crate::runnable(m.dispatch(request.continuation.unwrap(), &s))
    else {
        panic!("native storage work runs")
    };
    let outcome = std::thread::spawn(work).join().unwrap();
    let err = m.parse(&mut s, "work", &a, outcome).unwrap_err();
    assert!(
        format!("{err:?}")
            .contains("compressImage: timeout: the storage wait ran out; nothing was written"),
        "{err:?}"
    );
    // The queue has moved on: a write to the same path, while the
    // compression is still held.
    assert_eq!(call(&mut m, &mut s, "write-slow", "next"), "wrote next");
    // Released, the abandoned compression ends and writes nothing.
    hold.release();
    returned(&m);
    assert_eq!(
        std::fs::read(root.0.join("data/slow.jpg")).unwrap(),
        b"next"
    );
}

/// Until the compressions a wait gave up on have returned: whatever they
/// were going to write is on disk by then. The bound only stops work that
/// never ends.
#[cfg(target_vendor = "apple")]
fn returned(m: &exact_js::Module) {
    let hang = std::time::Instant::now() + std::time::Duration::from_secs(60);
    while m.storage_in_flight() > 0 {
        assert!(
            std::time::Instant::now() < hang,
            "the compression never returned"
        );
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
}

/// 120 × 80 of noise as a 24-bit BMP.
#[cfg(target_vendor = "apple")]
fn noise_bmp() -> Vec<u8> {
    let (w, h) = (120u32, 80u32);
    let row = (w * 3).div_ceil(4) * 4;
    let mut bmp = Vec::with_capacity(54 + (row * h) as usize);
    bmp.extend_from_slice(b"BM");
    bmp.extend_from_slice(&(54 + row * h).to_le_bytes());
    bmp.extend_from_slice(&[0, 0, 0, 0]);
    bmp.extend_from_slice(&54u32.to_le_bytes());
    bmp.extend_from_slice(&40u32.to_le_bytes());
    bmp.extend_from_slice(&w.to_le_bytes());
    bmp.extend_from_slice(&h.to_le_bytes());
    bmp.extend_from_slice(&1u16.to_le_bytes());
    bmp.extend_from_slice(&24u16.to_le_bytes());
    bmp.extend_from_slice(&[0; 24]);
    let mut seed: u32 = 0x1234_5678;
    for _ in 0..row * h {
        seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        bmp.push((seed >> 24) as u8);
    }
    bmp
}

/// The same for background work: the wait gives up on a compression an
/// answer started and did not await; its rejection runs (`timeout`), the
/// write queued behind it lands, and the compression never writes.
#[cfg(target_vendor = "apple")]
#[test]
fn a_background_compression_the_wait_gave_up_on_rejects_and_the_queue_moves() {
    let root = Root::new();
    let mut m = root.module();
    let hold = m.hold_compressions();
    m.activate().unwrap();
    let mut s = Store::new(GRANTS, Vec::<(String, Vec<u8>)>::new());
    assert_eq!(call(&mut m, &mut s, "file", "x"), "x");
    std::fs::write(root.0.join("data/noise.bmp"), noise_bmp()).unwrap();
    assert_eq!(call(&mut m, &mut s, "compress-background", ""), "started");
    assert_eq!(call(&mut m, &mut s, "last-error", ""), "timeout");
    assert_eq!(
        std::fs::read(root.0.join("data/slow.jpg")).unwrap(),
        b"after"
    );
    hold.release();
    returned(&m);
    assert_eq!(
        std::fs::read(root.0.join("data/slow.jpg")).unwrap(),
        b"after"
    );
}

/// And for a call the runner let go of (superseded here, by the same call
/// again): its compression's wait gives up, the operation is failed, and
/// what was queued behind it runs; the JPEG never lands.
#[cfg(target_vendor = "apple")]
#[test]
fn a_let_go_compression_the_wait_gave_up_on_does_not_strand_the_queue() {
    let root = Root::new();
    let mut m = root.module();
    let hold = m.hold_compressions();
    m.activate().unwrap();
    let mut s = Store::new(GRANTS, Vec::<(String, Vec<u8>)>::new());
    assert_eq!(call(&mut m, &mut s, "file", "x"), "x");
    std::fs::write(root.0.join("data/noise.bmp"), noise_bmp()).unwrap();
    let a = args("compress-abandoned", "");
    let Answer::Later(_) = m.answer(&mut s, "work", &a).unwrap() else {
        panic!("the compression waits on storage")
    };
    // The same call again replaces the first, which is let go; its
    // compression is the one in flight.
    let mut next = m.answer(&mut s, "work", &a);
    for _ in 0..20 {
        let Ok(Answer::Later(request)) = next else {
            break;
        };
        let Dispatch::Run(Work::Now(work)) =
            crate::runnable(m.dispatch(request.continuation.unwrap(), &s))
        else {
            panic!("native storage work runs")
        };
        let outcome = std::thread::spawn(work).join().unwrap();
        next = m.parse(&mut s, "work", &a, outcome);
    }
    // The second waits on the let-go first, then on its own compression,
    // whose wait gives up in turn.
    let err = match next {
        Err(err) => err,
        Ok(Answer::Now(v)) => panic!("the held compression wrote: {v:?}"),
        Ok(Answer::Later(_)) => panic!("the second call did not end"),
    };
    assert!(
        format!("{err:?}").contains("compressImage: timeout: "),
        "{err:?}"
    );
    assert_eq!(call(&mut m, &mut s, "write-slow", "next"), "wrote next");
    hold.release();
    returned(&m);
    assert_eq!(
        std::fs::read(root.0.join("data/slow.jpg")).unwrap(),
        b"next"
    );
}

/// A let-go call keeps the rest of its storage when its compression is so
/// failed, and a waiter that gave up first, then was discarded, does not
/// hide the loss from the waiter that delivers the chain.
#[cfg(target_vendor = "apple")]
#[test]
fn a_let_go_chain_keeps_its_other_storage_after_a_discarded_waiter_gave_up() {
    let root = Root::new();
    let mut m = root.module();
    let hold = m.hold_compressions();
    m.activate().unwrap();
    let mut s = Store::new(GRANTS, Vec::<(String, Vec<u8>)>::new());
    assert_eq!(call(&mut m, &mut s, "file", "x"), "x");
    std::fs::write(root.0.join("data/noise.bmp"), noise_bmp()).unwrap();
    let a = args("compress-and-write", "");
    let Answer::Later(request) = m.answer(&mut s, "work", &a).unwrap() else {
        panic!("the compression waits on storage")
    };
    // The first answer's own waiter runs, gives up and takes the right away;
    // the runner then forgets that answer and its waiter's outcome.
    let Dispatch::Run(Work::Now(work)) =
        crate::runnable(m.dispatch(request.continuation.unwrap(), &s))
    else {
        panic!("native storage work runs")
    };
    let outcome = std::thread::spawn(work).join().unwrap();
    assert!(format!("{outcome:?}").contains("compressImage: timeout: "));
    // The same call again lets the first go: the let-go chain's compression
    // is failed, its write delivered on, and the second's listing follows.
    let mut next = m.answer(&mut s, "work", &a).unwrap();
    for _ in 0..20 {
        let Answer::Later(request) = next else {
            break;
        };
        let Dispatch::Run(Work::Now(work)) =
            crate::runnable(m.dispatch(request.continuation.unwrap(), &s))
        else {
            panic!("native storage work runs")
        };
        let outcome = std::thread::spawn(work).join().unwrap();
        next = m.parse(&mut s, "work", &a, outcome).unwrap();
    }
    let Answer::Now(again) = next else {
        panic!("the second call did not end")
    };
    assert_eq!(super::storage::text(again), "again");
    // Behind the chain's write in the queue: it has landed by now.
    assert_eq!(call(&mut m, &mut s, "write-slow", "next"), "wrote next");
    assert_eq!(
        std::fs::read(root.0.join("data/second")).unwrap(),
        b"second"
    );
    hold.release();
    returned(&m);
    assert_eq!(
        std::fs::read(root.0.join("data/slow.jpg")).unwrap(),
        b"next"
    );
}
