use super::*;
#[test]
fn submitted_frames_join_multiple_ticks_named_spans_and_gpu_passes() {
    let Some(gpu) = gpu() else { return };
    let mut surface = surface();
    fixture::render(&gpu, &mut surface, &frame(0.)).unwrap();
    surface.agent(r#"{"op":"state","perf":true}"#).unwrap();
    for i in 1..=12 {
        fixture::render(&gpu, &mut surface, &frame(i as f64 * 167.)).unwrap();
    }
    let reply: serde_json::Value = serde_json::from_str(
        &surface
            .agent(r#"{"op":"state","trace_export":true}"#)
            .unwrap(),
    )
    .unwrap();
    let rows = reply["world"]["traceEvents"].as_array().unwrap();
    let ticks: Vec<_> = rows.iter().filter(|r| r["name"] == "tick").collect();
    assert!(ticks.len() >= 120);
    let frames: Vec<_> = rows.iter().filter(|r| r["name"] == "frame").collect();
    assert_eq!(frames.len(), 12);
    for tick in ticks {
        let number = tick["args"]["tick"].as_u64().unwrap();
        let frame = frames
            .iter()
            .find(|f| f["args"]["frame"] == tick["args"]["frame"])
            .unwrap();
        assert_eq!(tick["args"]["frame"].as_u64().unwrap(), number.div_ceil(10));
        let ts = |r: &serde_json::Value| r["ts"].as_f64().unwrap();
        let end = |r: &serde_json::Value| ts(r) + r["dur"].as_f64().unwrap();
        assert!(ts(frame) <= ts(tick) && end(frame) >= end(tick));
        let span = rows
            .iter()
            .find(|r| r["name"] == "move" && r["args"]["tick"] == number)
            .unwrap();
        assert_eq!(span["args"]["frame"], tick["args"]["frame"]);
        assert!(ts(span) >= ts(tick) && end(span) <= end(tick));
    }
    if gpu
        .device
        .features()
        .contains(wgpu::Features::TIMESTAMP_QUERY)
    {
        let gpu_rows: Vec<_> = rows.iter().filter(|r| r["cat"] == "world.gpu").collect();
        assert!(
            !gpu_rows.is_empty(),
            "GPU timestamps must reach the same export"
        );
        for pass in gpu_rows {
            assert!(frames
                .iter()
                .any(|f| f["args"]["frame"] == pass["args"]["frame"]));
            assert!(pass["args"]["clock"]
                .as_str()
                .unwrap()
                .contains("independent"));
        }
    }
}
