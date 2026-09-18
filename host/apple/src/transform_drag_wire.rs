//! The frozen120-byte v2 transform packet.
use exact_kernel::{NodeKey, TransformDragBinding};
use exact_motion::Value;
use exact_runner::Event;

pub(super) struct Input {
    pub(super) op: u32,
    pub(super) runtime: u64,
    pub(super) binding: TransformDragBinding,
    pub(super) sequence: u64,
    pub(super) tokens: [u64; 2],
    pub(super) values: [f64; 6],
    pub(super) now_ms: f64,
}
impl Input {
    pub(super) fn decode(bytes: &[u8]) -> Result<Self, &'static str> {
        use exact_plan::bytes::Reader;
        if bytes.len() != 120 {
            return Err("malformed transform length");
        }
        let decode = || -> Result<Self, exact_plan::PlanError> {
            let mut r = Reader::new(bytes);
            if r.u32()? != 2 {
                return Err(exact_plan::PlanError::BadCount(0));
            }
            let op = r.u32()?;
            let runtime = r.u64()?;
            let key = |n: u64| NodeKey {
                index: n as u32,
                generation: (n >> 32) as u32,
            };
            let binding = TransformDragBinding {
                handle: key(r.u64()?),
                target: key(r.u64()?),
                clip: key(r.u64()?),
            };
            let sequence = r.u64()?;
            let tokens = [r.u64()?, r.u64()?];
            let mut values = [0.0; 6];
            for v in &mut values {
                *v = r.f64()?;
            }
            let now_ms = r.f64()?;
            Ok(Self {
                op,
                runtime,
                binding,
                sequence,
                tokens,
                values,
                now_ms,
            })
        };
        let input = decode().map_err(|_| "malformed transform input")?;
        if !(10..=14).contains(&input.op) {
            return Err("invalid transform operation");
        }
        Ok(input)
    }
    pub(super) fn event(&self) -> Event {
        let [x, y, scale, vx, vy, vscale] = self.values;
        if self.op == 10 {
            Event::TransformGeometry {
                box_width: x,
                box_height: y,
                port_width: scale,
                port_height: vx,
            }
        } else {
            Event::TransformRelease {
                x,
                y,
                scale,
                vx,
                vy,
                vscale,
            }
        }
    }
    pub(super) fn validate(&self, floor: f64) -> Result<(), &'static str> {
        if !self.now_ms.is_finite() || self.now_ms < 0.0 || self.now_ms / 1000.0 < floor {
            return Err("invalid transform clock");
        }
        if [10, 11, 14].contains(&self.op) && self.tokens != [0; 2] {
            return Err("unused transform tokens must be zero");
        }
        let unused = match self.op {
            10 => 4,
            11 | 12 => 3,
            13 => 6,
            _ => 0,
        };
        if self.values[unused..].iter().any(|v| *v != 0.0) {
            return Err("unused transform values must be zero");
        }
        if self.op != 14 && !valid_event(&self.event()) {
            return Err("invalid transform values");
        }
        Ok(())
    }
    pub(super) fn samples(&self) -> [Value; 2] {
        [
            Value::new(self.values[0], self.values[1]),
            Value::scalar(self.values[2]),
        ]
    }
}

pub(super) fn valid_event(event: &Event) -> bool {
    let pixel = |v: f64| v.is_finite() && v.abs() <= f32::MAX as f64;
    match *event {
        Event::TransformGeometry {
            box_width,
            box_height,
            port_width,
            port_height,
        } => [box_width, box_height, port_width, port_height]
            .into_iter()
            .all(|v| pixel(v) && v >= 0.0),
        Event::TransformRelease {
            x,
            y,
            scale,
            vx,
            vy,
            vscale,
        } => {
            pixel(x)
                && pixel(y)
                && pixel(scale)
                && scale > 0.0
                && (scale as f32) > 0.0
                && [vx, vy, vscale].into_iter().all(f64::is_finite)
        }
        _ => true,
    }
}
