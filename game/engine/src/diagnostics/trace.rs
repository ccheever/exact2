use crate::{data::text::Float, values::quote};
use std::{collections::VecDeque, fmt::Write};
const LIMIT: usize = 65536;
struct Sample {
    name: &'static str,
    at: &'static str,
    start: f64,
    end: f64,
    tick: u64,
    frame: u64,
}
#[derive(Default)]
pub(super) struct Trace {
    rows: VecDeque<Sample>,
    pub frames: u64,
    dropped: u64,
}
impl Trace {
    pub fn armed() -> Self {
        Self {
            rows: VecDeque::with_capacity(LIMIT),
            ..Self::default()
        }
    }
    pub fn record(
        &mut self,
        name: &'static str,
        at: &'static str,
        start: f64,
        end: f64,
        tick: u64,
    ) {
        if self.rows.len() == LIMIT {
            self.rows.pop_front();
            self.dropped += 1;
        }
        self.rows.push_back(Sample {
            name,
            at,
            start,
            end,
            tick,
            frame: 0,
        });
    }
    pub fn frame(&mut self, end: f64) {
        self.frames += 1;
        let mut start = end;
        for row in self.rows.iter_mut().rev().take_while(|r| r.frame == 0) {
            row.frame = self.frames;
            start = start.min(row.start);
        }
        self.record("frame", "", start, end, 0);
        self.rows.back_mut().unwrap().frame = self.frames;
    }
    pub fn json(&self) -> String {
        let mut out = String::from("[");
        for (i, row) in self.rows.iter().enumerate() {
            if i > 0 {
                out.push(',');
            }
            write!(out, "{{\"name\":{},\"cat\":\"world.cpu\",\"ph\":\"X\",\"pid\":0,\"tid\":1,\"ts\":{},\"dur\":{},\"args\":{{\"tick\":{},\"frame\":{},\"at\":{},\"clock\":\"host monotonic\",\"dropped\":{}}}}}", quote(row.name), Float(row.start*1000.), Float((row.end-row.start).max(0.)*1000.), row.tick, row.frame, quote(row.at), self.dropped).unwrap();
        }
        out.push(']');
        out
    }
}
