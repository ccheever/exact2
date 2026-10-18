use super::*;
#[test]
fn submitted_frames_join_multiple_ticks_named_spans_and_gpu_passes() {
    let Some(gpu) = gpu() else { return };
    let mut surface = surface();
    fixture::render(&gpu, &mut surface, &frame(0.)).unwrap();
    surface.agent(r#"{"op":"state","perf":true}"#).unwrap();
    let timestamps = gpu
        .device
        .features()
        .contains(wgpu::Features::TIMESTAMP_QUERY);
    let mut gpu_count = 0;
    for i in 1..=12 {
        fixture::render(&gpu, &mut surface, &frame(i as f64 * 167.)).unwrap();
        drain_timings(&gpu, &mut surface);
        if timestamps {
            let events = export(&mut surface);
            let passes: Vec<_> = events.iter().filter(|r| r["cat"] == "world.gpu").collect();
            assert!(
                passes.len() > gpu_count,
                "submission {i} must contribute GPU rows"
            );
            for pass in &passes[gpu_count..] {
                assert_eq!(
                    pass["args"]["frame"], i,
                    "GPU row must name its own submission"
                );
            }
            gpu_count = passes.len();
        }
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

fn export(surface: &mut WorldSurface<Move>) -> Vec<serde_json::Value> {
    let reply: serde_json::Value = serde_json::from_str(
        &surface
            .agent(r#"{"op":"state","trace_export":true}"#)
            .unwrap(),
    )
    .unwrap();
    reply["world"]["traceEvents"].as_array().unwrap().clone()
}
fn drain_timings(gpu: &Gpu, surface: &mut WorldSurface<Move>) {
    // Complete submission, resolve and map without adding another rendered frame.
    for _ in 0..3 {
        gpu.device
            .poll(wgpu::PollType::wait_indefinitely())
            .unwrap();
        if let Some(timing) = &mut surface.gpu_timing {
            timing.poll(&gpu.device, &gpu.queue);
        }
    }
}
#[test]
fn restore_and_argument_restart_discard_completed_and_inflight_gpu_recordings() {
    let Some(gpu) = gpu() else { return };
    for restart in [false, true] {
        let mut s = surface();
        fixture::render(&gpu, &mut s, &frame(0.)).unwrap();
        let saved = s.carry().unwrap().unwrap();
        s.agent(r#"{"op":"state","perf":true}"#).unwrap();
        fixture::render(&gpu, &mut s, &frame(167.)).unwrap();
        drain_timings(&gpu, &mut s);
        let old = export(&mut s);
        assert!(old.iter().any(|r| r["name"] == "tick"));
        if gpu
            .device
            .features()
            .contains(wgpu::Features::TIMESTAMP_QUERY)
        {
            assert!(old.iter().any(|r| r["cat"] == "world.gpu"));
        }
        // Leave another submission in flight when replacing the recording.
        fixture::render(&gpu, &mut s, &frame(334.)).unwrap();
        if restart {
            s.bind(&[Value::Bool(true), Value::Number(1.)], None)
                .unwrap();
        } else {
            s.restore(&saved, Restore::Open).unwrap();
        }
        assert!(
            s.gpu_timing.is_none(),
            "readbacks must belong to the old recording"
        );
        s.agent(r#"{"op":"state","perf":true}"#).unwrap();
        drain_timings(&gpu, &mut s);
        assert!(
            export(&mut s).is_empty(),
            "immediate rearm/export must be empty"
        );
        fixture::render(&gpu, &mut s, &frame(501.)).unwrap();
        fixture::render(&gpu, &mut s, &frame(668.)).unwrap();
        drain_timings(&gpu, &mut s);
        let rows = export(&mut s);
        let frames: Vec<_> = rows
            .iter()
            .filter(|r| r["name"] == "frame")
            .map(|r| r["args"]["frame"].clone())
            .collect();
        assert!(!frames.is_empty());
        assert!(rows
            .iter()
            .filter(|r| r["cat"] == "world.gpu")
            .all(|r| frames.contains(&r["args"]["frame"])));
    }
}
