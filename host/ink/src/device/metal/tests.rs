use super::*;
use crate::instance::Encoder;

#[cfg(target_os = "macos")]
#[test]
fn readback_waits_for_a_render_queued_behind_a_blocked_worker() {
    use std::sync::mpsc;
    let (release, blocked) = mpsc::channel();
    let (queued, ready) = mpsc::channel();
    let (finished, result) = mpsc::channel();
    let owner = std::thread::spawn(move || {
        let mut d = Metal::new().unwrap();
        let target = d.create_target(4, 4).unwrap();
        d.render(&Frame::default(), target, Some([1., 0., 0., 1.]))
            .unwrap();
        assert_eq!(d.readback(target).unwrap(), [255, 0, 0, 255].repeat(16));
        let worker = Rc::new(crate::present::Thread::new().unwrap());
        worker.send(move || blocked.recv().unwrap()).unwrap();
        d.thread = Some(worker);
        let submission = d
            .render(&Frame::default(), target, Some([0., 1., 0., 1.]))
            .unwrap();
        queued.send(()).unwrap();
        let pixels = d.readback(target).unwrap();
        finished
            .send((pixels, d.completed().unwrap(), submission))
            .unwrap();
    });
    ready.recv().unwrap();
    // The worker cannot commit until released. Bound this observation so a
    // regression returns stale pixels instead of hanging the test's owner.
    let early = result
        .recv_timeout(std::time::Duration::from_millis(100))
        .ok();
    release.send(()).unwrap();
    owner.join().unwrap();
    assert!(
        early.is_none(),
        "readback returned before its command was committed"
    );
    let (pixels, done, submission) = result.recv().unwrap();
    assert_eq!(pixels, [0, 255, 0, 255].repeat(16));
    assert!(done >= submission);
}

#[cfg(target_os = "macos")]
#[test]
fn a_failed_worker_receipt_keeps_its_original_error() {
    let mut d = Metal::new().unwrap();
    let (sender, receipt) = std::sync::mpsc::channel();
    sender
        .send(Err(Error::DeviceLost("upload refused".into())))
        .unwrap();
    d.submitted = Submission(1);
    d.pending.insert(
        1,
        Pending {
            command: None,
            receipt: Some(receipt),
        },
    );
    for _ in 0..2 {
        assert_eq!(
            d.wait(Submission(1)).unwrap_err().to_string(),
            "device lost: upload refused"
        );
    }
    assert!(d.pending[&1].receipt.is_none());
    assert_eq!(d.done, Submission(0));
}

#[test]
fn readback_scissors_uploads_mips_and_retirement() {
    let mut d = Metal::new().unwrap();
    let target = d.create_target(12, 4).unwrap();
    let image = d
        .create_texture(TextureDesc {
            mipmapped: true,
            ..TextureDesc::rgba(8, 8)
        })
        .unwrap();
    let data: Vec<_> = (0..64)
        .flat_map(|i| {
            if (i % 8 + i / 8) % 2 == 0 {
                [128, 0, 0, 128]
            } else {
                [0, 0, 128, 128]
            }
        })
        .collect();
    d.upload_texture(image, [0, 0], [8, 8], &data).unwrap();
    d.generate_mips(image).unwrap();
    for level in 1..4 {
        for p in d.readback_level(image, level).unwrap().chunks_exact(4) {
            assert!(
                p.iter()
                    .zip([64u8, 0, 64, 128])
                    .all(|(a, b)| a.abs_diff(b) <= 1),
                "{p:?}"
            );
        }
    }
    let mut encoder = Encoder::new(12, 4);
    encoder
        .push(
            Instance::rounded([0., 0., 12., 4.], [0.; 4], [1., 0., 0., 1.]),
            Bindings::default(),
        )
        .unwrap();
    encoder
        .push_clip(crate::instance::Clip::Scissor(crate::instance::Scissor {
            x: 4,
            y: 0,
            width: 4,
            height: 4,
        }))
        .unwrap();
    encoder
        .push(
            Instance::new(Kind::RoundedRect, crate::Brush::Image, [0., 0., 12., 4.]),
            Bindings {
                image,
                ..Default::default()
            },
        )
        .unwrap();
    let frame = encoder.finish();
    let s = d.render(&frame, target, Some([0.; 4])).unwrap();
    d.retire_texture(image).unwrap();
    let pixels = d.readback(target).unwrap();
    assert_eq!(&pixels[..4], &[255, 0, 0, 255]);
    assert_ne!(&pixels[4 * 5..4 * 6], &[255, 0, 0, 255]);
    assert_eq!(&pixels[4 * 9..4 * 10], &[255, 0, 0, 255]);
    assert!(d.completed().unwrap() >= s);
    assert!(d.retired_textures.is_empty());
    assert!(d.render(&frame, target, None).is_err());
    let copy = d.create_target(3, 1).unwrap();
    d.copy_region(target, [3, 1], copy, [0, 0], [3, 1]).unwrap();
    assert_eq!(
        d.readback(copy).unwrap(),
        pixels[(12 + 3) * 4..(12 + 6) * 4]
    );
    assert!(d.upload_texture(copy, [2, 0], [2, 1], &[0; 8]).is_err());
    let resized = d.resize_target(target, 1, 1).unwrap();
    d.render(&Frame::default(), resized, Some([0.25, 0., 0., 0.5]))
        .unwrap();
    assert_eq!(d.readback(resized).unwrap(), [64, 0, 0, 128]);
    d.trim().unwrap();
    assert!(d.buffers.iter().all(|b| b.raw.length() == 192));
}

#[test]
fn odd_mips_include_every_source_pixel() {
    let mut d = Metal::new().unwrap();
    let t = d
        .create_texture(TextureDesc {
            mipmapped: true,
            ..TextureDesc::rgba(3, 1)
        })
        .unwrap();
    d.upload_texture(
        t,
        [0, 0],
        [3, 1],
        &[255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255],
    )
    .unwrap();
    d.generate_mips(t).unwrap();
    let p = d.readback_level(t, 1).unwrap();
    assert!(
        p.iter()
            .zip([85u8, 85, 85, 255])
            .all(|(a, b)| a.abs_diff(b) <= 1),
        "{p:?}"
    );
}

#[test]
fn cpu_written_textures_use_shared_storage() {
    let mut d = Metal::new().unwrap();
    let id = d.create_texture(TextureDesc::rgba(2, 2)).unwrap();
    assert_eq!(d.textures[&id].raw.storageMode(), MTLStorageMode::Shared);
    d.upload_texture(id, [0, 0], [2, 2], &[41; 16]).unwrap();
    assert_eq!(d.readback(id).unwrap(), [41; 16]);
}

#[cfg(target_os = "macos")]
#[test]
fn four_atlas_batches_retire_a_busy_slot_and_trim_releases_the_ring() {
    use crate::device::TextureUpload;
    use std::sync::mpsc;
    let (release, blocked) = mpsc::channel();
    let (queued, ready) = mpsc::channel();
    let owner = std::thread::spawn(move || {
        let mut d = Metal::new().unwrap();
        let texture = d.create_texture(TextureDesc::rgba(8, 8)).unwrap();
        let worker = Rc::new(crate::present::Thread::new().unwrap());
        worker
            .send(move || {
                let _ = blocked.recv();
            })
            .unwrap();
        d.thread = Some(worker);
        let mut last = Submission(0);
        let mut first = None;
        for n in 0..4 {
            last = d
                .upload_textures(&[TextureUpload {
                    texture,
                    origin: [n, 0],
                    size: [1, 1],
                    data: vec![31 + n as u8; 4],
                }])
                .unwrap();
            if n == 0 {
                first = Some(d.atlas_buffers[0].as_ref().unwrap().raw.clone());
            }
        }
        let retired = d.retired_buffers.len();
        let busy_replaced = !std::ptr::eq(
            &*d.atlas_buffers[0].as_ref().unwrap().raw,
            &**first.as_ref().unwrap(),
        );
        let floor = d
            .atlas_buffers
            .iter()
            .all(|b| b.as_ref().unwrap().raw.length() == 65536);
        queued
            .send((retired, busy_replaced, floor, d.done))
            .unwrap();
        d.wait(last).unwrap();
        assert!(d.retired_buffers.is_empty());
        let pixels = d.readback(texture).unwrap();
        assert_eq!(
            &pixels[..16],
            &[31, 31, 31, 31, 32, 32, 32, 32, 33, 33, 33, 33, 34, 34, 34, 34]
        );
        // Slots 1, 2 and 0 each see a larger, still sub-64-KiB batch.
        let slots: Vec<_> = d
            .atlas_buffers
            .iter()
            .map(|b| b.as_ref().unwrap().raw.clone())
            .collect();
        for n in 0..3 {
            let s = d
                .upload_textures(&[TextureUpload {
                    texture,
                    origin: [0, 0],
                    size: [2 + n, 2],
                    data: vec![71; ((2 + n) * 2 * 4) as usize],
                }])
                .unwrap();
            d.wait(s).unwrap();
        }
        for (b, old) in d.atlas_buffers.iter().zip(&slots) {
            assert!(std::ptr::eq(&*b.as_ref().unwrap().raw, &**old));
        }
        d.trim().unwrap();
        assert!(d.atlas_buffers.iter().all(Option::is_none));
        d.completed().unwrap();
        assert!(d.retired_buffers.is_empty());
    });
    let observed = ready.recv_timeout(std::time::Duration::from_secs(10));
    release.send(()).unwrap();
    owner.join().unwrap();
    let (retired, busy_replaced, floor, done) =
        observed.expect("batches must enqueue without waiting");
    assert_eq!(retired, 1);
    assert!(busy_replaced && floor);
    assert_eq!(done, Submission(0));
}
