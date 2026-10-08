//! `storage.fs.compressImage` from TypeScript (LLP 1069.002 Amendment A1),
//! over the storage fixture module (`tests/fixtures/storage.ts`).
#![cfg(exact_js_engine)]
use super::storage::{args, call, Root, GRANTS};
use exact_runner::{Answer, DataSource, Dispatch, Store, Work};

/// `storage.fs.compressImage` from TypeScript (LLP 1069.002 A1): Hermes,
/// the prelude, ibex2's opcode 121 and the platform codec. On Apple the
/// fixture comes back upright as a JPEG; elsewhere the call is refused as
/// `unsupported`. Either way each refusal carries its code.
#[test]
fn compress_image_writes_an_upright_jpeg_or_refuses_by_name() {
    let root = Root::new();
    let mut m = root.module();
    m.activate().unwrap();
    let mut s = Store::new(GRANTS, Vec::<(String, String)>::new());
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
    } else {
        assert!(
            lines[0].starts_with("Error unsupported compressImage: unsupported: "),
            "{text}"
        );
        assert_eq!(refusals[..2], ["Error denied", "Error ENOENT"], "{text}");
    }
    assert!(
        lines[5].contains("compressImage: needs app:/ paths"),
        "{text}"
    );
    assert!(
        lines[6].contains("maxDimension must be an integer from 1 to 8192"),
        "{text}"
    );
    assert!(
        lines[7].contains("options must be {maxDimension, maxBytes}"),
        "{text}"
    );
}

/// A wait that gives up on an `fs.compressImage` takes its right to write
/// away (LLP 1069.002 A1.5): the answer fails `timeout`, the queue moves on
/// to a write of the same path, and the compression, still running, never
/// writes over it.
#[cfg(target_vendor = "apple")]
#[test]
fn a_compression_the_wait_gave_up_on_never_writes() {
    let root = Root::new();
    let mut m = root.module();
    m.set_storage_wait(std::time::Duration::from_millis(1));
    m.activate().unwrap();
    let mut s = Store::new(GRANTS, Vec::<(String, String)>::new());
    assert_eq!(call(&mut m, &mut s, "file", "x"), "x");
    // 2000 × 1500 of noise as a 24-bit BMP: seconds of trials, then a write.
    let (w, h) = (2000u32, 1500u32);
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
    std::fs::write(root.0.join("data/noise.bmp"), &bmp).unwrap();
    // The wait gives up: the answer fails, and its failure names `timeout`.
    let a = args("compress-abandoned", "");
    let Answer::Later(request) = m.answer(&mut s, "work", &a).unwrap() else {
        panic!("the compression waits on storage")
    };
    let Dispatch::Run(Work::Now(work)) = m.dispatch(request.continuation.unwrap(), &s) else {
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
    // compression still runs.
    let mut m2 = m;
    assert_eq!(call(&mut m2, &mut s, "write-slow", "next"), "wrote next");
    // The abandoned compression finishes in the background and writes nothing.
    std::thread::sleep(std::time::Duration::from_secs(4));
    assert_eq!(
        std::fs::read(root.0.join("data/slow.jpg")).unwrap(),
        b"next"
    );
    drop(m2);
}
