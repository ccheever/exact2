//! Canvas 2D draws across the module seam (LLP 1056 D1, D4; LLP 1071 §7):
//! a host whose runner is not Rust (the web's JS target) asks the module to
//! draw one surface and replays the lists it answers. Its own export,
//! [`crate::export_draw!`], so a module whose source draws nothing links
//! none of the recorder.
//!
//! Request (after the ABI header, op 5): `u8` 0 draw / 1 retire, the canvas
//! lifetime (`u64`); a draw adds its generation (`u32`), sequence (`u64`),
//! surface name, arguments (a value list), authored names (a string list),
//! the frame (time, mounted, cause bits, width, height, pixel width and
//! height, scale), the canvas node's `currentColor` (`u8` flag, then r, g,
//! b as `u8` and alpha as `f64`) and `rtl` (`u8`). Reply (op 5): the lists
//! (a count, then bytes each), `u8` wants a frame, `u8` threw and its
//! message.

use super::{bytes, end, error, finish, header, read_bytes, reader};
use exact_plan::{bytes::Reader, Value};
use exact_runner::exact_canvas::{Causes, Context2d, Env, Frame, Rgba};
use exact_runner::{DataSource, DrawReply, DrawRequest, Drawn};

/// The recorders a module keeps, one per canvas lifetime, each for the
/// generation it was made in (a context's state persists across draws
/// within a generation, as a canvas element's does). A page has a few
/// canvases: a list, where a map would cost the module its code.
#[derive(Default)]
pub struct Recorders(Vec<(u64, u32, Context2d)>);

/// Encode a draw request (the host's half; the JS target writes the same
/// bytes in `host/web-js/rust-data.js`).
pub fn draw_request(r: &DrawRequest<'_>) -> Result<Vec<u8>, String> {
    let mut w = header(5);
    w.u8(0);
    w.u64(r.canvas);
    w.u32(r.generation);
    w.u64(r.seq);
    w.string(r.surface);
    Value::list(r.args.to_vec()).encode(&mut w);
    w.u32(r.names.len() as u32);
    for n in r.names {
        w.string(n);
    }
    let f = &r.frame;
    for v in [f.time, f.mounted] {
        w.f64(v);
    }
    w.u8(f.cause.0);
    w.f64(f.width);
    w.f64(f.height);
    w.u32(f.pixel_width);
    w.u32(f.pixel_height);
    w.f64(f.scale);
    match r.current_color {
        Some(c) => {
            w.u8(1);
            for v in [c.r, c.g, c.b] {
                w.u8(v);
            }
            w.f64(c.a);
        }
        None => w.u8(0),
    }
    // Bit 0 `rtl`, bit 1 a `display-p3` canvas (LLP 1100 D12a).
    w.u8(u8::from(r.rtl) | (u8::from(r.p3) << 1));
    finish(w)
}

/// A draw reply's bytes back as the runner's [`DrawReply`].
pub fn read_draw_reply(data: &[u8]) -> Result<DrawReply, String> {
    let mut r = reader(data)?;
    if r.u8().map_err(error)? != 5 {
        return Err("expected a draw reply".into());
    }
    let n = r.count().map_err(error)?;
    let lists = (0..n)
        .map(|_| read_bytes(&mut r).map(<[u8]>::to_vec))
        .collect::<Result<Vec<_>, _>>()?;
    let wants_frame = r.u8().map_err(error)? == 1;
    let error = match r.u8().map_err(error)? {
        0 => None,
        _ => Some(r.string().map_err(error)?),
    };
    end(&r)?;
    Ok(DrawReply {
        lists,
        wants_frame,
        error,
        notes: Vec::new(),
    })
}

fn frame(r: &mut Reader<'_>) -> Result<Frame, String> {
    let (time, mounted) = (r.f64().map_err(error)?, r.f64().map_err(error)?);
    let cause = Causes(r.u8().map_err(error)?);
    let (width, height) = (r.f64().map_err(error)?, r.f64().map_err(error)?);
    let (pixel_width, pixel_height) = (r.u32().map_err(error)?, r.u32().map_err(error)?);
    Ok(Frame {
        time,
        mounted,
        cause,
        width,
        height,
        pixel_width,
        pixel_height,
        scale: r.f64().map_err(error)?,
    })
}

impl<D: DataSource> super::Session<D> {
    /// One draw or retirement ([`draw_request`]'s bytes); the reply is the
    /// session's output, as [`super::Session::dispatch`]'s is.
    pub fn draw(&mut self, recorders: &mut Recorders, input: &[u8]) -> Result<(), String> {
        let mut r = reader(input)?;
        if r.u8().map_err(error)? != 5 {
            return Err("expected a draw request".into());
        }
        let kind = r.u8().map_err(error)?;
        let canvas = r.u64().map_err(error)?;
        let mut w = header(5);
        if kind == 1 {
            end(&r)?;
            recorders.0.retain(|r| r.0 != canvas);
            w.u32(0);
            w.u8(0);
            w.u8(0);
            self.output = finish(w)?;
            return Ok(());
        }
        let generation = r.u32().map_err(error)?;
        let seq = r.u64().map_err(error)?;
        let surface = r.string().map_err(error)?;
        let Value::List(args) = Value::decode(&mut r).map_err(error)? else {
            return Err("arguments must be a list".into());
        };
        let names = (0..r.count().map_err(error)?)
            .map(|_| r.string().map_err(error))
            .collect::<Result<Vec<_>, _>>()?;
        let frame = frame(&mut r)?;
        let current_color = match r.u8().map_err(error)? {
            0 => None,
            _ => Some(Rgba {
                r: r.u8().map_err(error)?,
                g: r.u8().map_err(error)?,
                b: r.u8().map_err(error)?,
                a: r.f64().map_err(error)?,
            }),
        };
        let flags = r.u8().map_err(error)?;
        let (rtl, p3) = (flags & 1 == 1, flags & 2 == 2);
        end(&r)?;
        let at = match recorders.0.iter().position(|r| r.0 == canvas) {
            Some(at) => at,
            None => {
                recorders.0.push((canvas, generation, Context2d::new()));
                recorders.0.len() - 1
            }
        };
        let slot = &mut recorders.0[at];
        if slot.1 != generation {
            *slot = (canvas, generation, Context2d::new());
        }
        let ctx = slot.2.clone();
        // No text engine or image table crosses this seam: a draw's
        // `measureText` and images are the runner's (LLP 1056 D8, D9).
        ctx.set_env(Env {
            canvas,
            text: None,
            images: Default::default(),
            current_color,
            rtl,
            p3,
        });
        let request = DrawRequest {
            canvas,
            generation,
            seq,
            surface: &surface,
            args: &args,
            names: &names,
            frame,
            current_color,
            rtl,
            p3,
        };
        let reply = match self.data.draw(&request, &ctx) {
            Drawn::Now(reply) => reply,
            Drawn::Later => DrawReply {
                error: Some("a module draw answers now".into()),
                ..Default::default()
            },
        };
        w.u32(reply.lists.len() as u32);
        for l in &reply.lists {
            bytes(&mut w, l);
        }
        w.u8(u8::from(reply.wants_frame));
        match &reply.error {
            Some(e) => {
                w.u8(1);
                w.string(e);
            }
            None => w.u8(0),
        }
        self.output = finish(w)?;
        Ok(())
    }
}

/// Export this module's Canvas 2D draws, beside [`crate::export!`]'s
/// exports for the same `$data`: `exact_logic_draw(session, ptr, len)`,
/// whose reply is read with `exact_logic_output` as a call's is.
#[macro_export]
macro_rules! export_draw {
    ($data:ty) => {
        #[no_mangle]
        #[allow(clippy::missing_safety_doc)]
        pub unsafe extern "C" fn exact_logic_draw(session: usize, ptr: usize, len: u32) -> u32 {
            thread_local! {
                static RECORDERS: ::std::cell::RefCell<::std::vec::Vec<(usize, $crate::draw::Recorders)>> = Default::default();
            }
            if session == 0 || ptr == 0 || len as usize > $crate::MAX_MESSAGE {
                return 1;
            }
            let input = std::slice::from_raw_parts(ptr as *const u8, len as usize);
            RECORDERS.with(|r| {
                let mut r = r.borrow_mut();
                let at = match r.iter().position(|(s, _)| *s == session) {
                    Some(at) => at,
                    None => {
                        r.push((session, Default::default()));
                        r.len() - 1
                    }
                };
                let recorders = &mut r[at].1;
                u32::from(
                    (&mut *(session as *mut $crate::Session<$data>))
                        .draw(recorders, input)
                        .is_err(),
                )
            })
        }
    };
}
