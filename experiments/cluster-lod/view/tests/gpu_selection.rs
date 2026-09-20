#[path = "../src/bin/oracle.rs"]
mod oracle;
#[path = "../src/bin/readback.rs"]
mod readback;
#[path = "../src/bin/selection_readback.rs"]
mod selection_readback;

#[test]
fn selection_oracles() {
    let (device, queue, adapter) = match pollster::block_on(clod_view::request_device(false)) {
        Ok(result) => result,
        Err(e) if e.starts_with("NO ADAPTER:") => {
            eprintln!("SKIP GPU selection oracles: {e}; cameras=0");
            return;
        }
        Err(e) => panic!("{e}"),
    };
    println!("adapter={} features={:?}", adapter.name, device.features());
    let mut mesh = clod_bake::procedural::octasphere(8).expect("mesh");
    let bake = clod_bake::bake(
        &mut mesh,
        clod_format::Config {
            page_bytes: 256 * 1024,
            ..Default::default()
        },
        [0; 32],
    )
    .expect("bake");
    let reader = clod_format::Reader::new(&bake.bytes).expect("reader");
    // Exercise the shared RGB difference metric as well as exact equality in the sweep.
    let zero = readback::difference(&[0, 0, 0, 255], &[0, 0, 0, 255]);
    assert_eq!(zero.max, 0);
    println!("difference_mean={} fraction={}", zero.mean, zero.fraction);
    oracle::run(&reader, &device, &queue, [192, 192], 64, true).expect("GPU oracles");
}
