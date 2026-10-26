//! Mouse notches use Windows preferences; precision deltas already name pixels.
use exact_linux::{presenter::Scroller, Presenter};
use exact_runner::DataSource;
use winit::event::MouseScrollDelta;

// WinUI ScrollPresenter's line-sized change, in logical pixels.
// https://github.com/microsoft/microsoft-ui-xaml/blob/main/controls/dev/ScrollPresenter/ScrollPresenter.cpp
const LINE: f32 = 16.;

pub(super) fn dispatch<D: DataSource>(
    presenter: &mut Presenter<D>,
    at: (f32, f32),
    delta: MouseScrollDelta,
    scale: f64,
) {
    let (dx, dy) = match delta {
        MouseScrollDelta::PixelDelta(at) => pixels(at, scale),
        MouseScrollDelta::LineDelta(x, y) => {
            let counts = preferences();
            let chain = presenter.scrollers_at(at.0, at.1).chain;
            notches((-x, -y), counts, &chain, presenter.viewport())
        }
    };
    presenter.wheel_at(at.0, at.1, dx, dy);
}

fn pixels(at: winit::dpi::PhysicalPosition<f64>, scale: f64) -> (f32, f32) {
    ((-at.x / scale) as f32, (-at.y / scale) as f32)
}

#[allow(unsafe_code)]
fn preferences() -> [u32; 2] {
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        SystemParametersInfoW, SPI_GETWHEELSCROLLCHARS, SPI_GETWHEELSCROLLLINES,
    };
    [SPI_GETWHEELSCROLLCHARS, SPI_GETWHEELSCROLLLINES].map(|setting| {
        let mut count = 3u32;
        // Read at the wheel event so a settings change also takes effect while
        // this window stays focused. These queries never modify the preference.
        unsafe {
            SystemParametersInfoW(setting, 0, (&mut count as *mut u32).cast(), 0);
        }
        count
    })
}

fn notches(
    delta: (f32, f32),
    counts: [u32; 2],
    chain: &[Scroller],
    viewport: (f32, f32),
) -> (f32, f32) {
    let delta = (
        if counts[0] == 0 { 0. } else { delta.0 },
        if counts[1] == 0 { 0. } else { delta.1 },
    );
    let takes = |enabled: bool, by: f32, offset: f32, max: f32| {
        enabled && ((by < 0. && offset > 0.) || (by > 0. && offset < max))
    };
    // The same innermost scroller takes the whole tick, including diagonal
    // input. At an edge, page-sized deltas use the next receiver's viewport.
    let port = chain
        .iter()
        .find(|s| {
            takes(s.axes.0, delta.0, s.offset.0, s.max.0)
                || takes(s.axes.1, delta.1, s.offset.1, s.max.1)
        })
        .map_or(viewport, |s| (s.port[2], s.port[3]));
    let step = |count: u32, page: f32| {
        // UINT_MAX is WHEEL_PAGESCROLL. Excessive line counts also mean one
        // page per notch; preserve fractions from high-resolution wheels.
        (count as f32 * LINE).min(page.max(0.))
    };
    (
        delta.0 * step(counts[0], port.0),
        delta.1 * step(counts[1], port.1),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scroller(port: (f32, f32), max: (f32, f32), offset: (f32, f32)) -> Scroller {
        Scroller {
            id: 1,
            axes: (max.0 > 0., max.1 > 0.),
            max,
            offset,
            port: [0., 0., port.0, port.1],
            node: false,
        }
    }

    #[test]
    fn fractional_notches_honor_independent_counts_and_disabled_axes() {
        let viewport = (800., 600.);
        assert_eq!(notches((0.25, -1.), [3, 5], &[], viewport), (12., -80.));
        assert_eq!(notches((1., 1.), [0, 0], &[], viewport), (0., 0.));
        assert_eq!(notches((1., 1.), [0, 2], &[], viewport), (0., 32.));
        assert_eq!(notches((1., 1.), [2, 0], &[], viewport), (32., 0.));
    }

    #[test]
    fn page_notches_follow_the_receiving_nested_viewport_and_direction() {
        let inner = scroller((200., 64.), (0., 240.), (0., 240.));
        let outer = scroller((800., 600.), (0., 1000.), (0., 0.));
        let chain = [inner, outer];
        let page = [u32::MAX; 2];
        assert_eq!(notches((0., -1.), page, &chain, (900., 700.)), (0., -64.));
        assert_eq!(notches((0., 1.), page, &chain, (900., 700.)), (0., 600.));
        assert_eq!(notches((0., -0.25), page, &chain, (900., 700.)), (0., -16.));
        assert_eq!(
            notches((0., -1.), [3, 100], &chain, (900., 700.)),
            (0., -64.)
        );
    }

    #[test]
    fn a_diagonal_tick_keeps_one_receivers_page_and_disabled_axes_do_not_claim_it() {
        let strip = scroller((200., 80.), (1000., 0.), (0., 0.));
        let outer = scroller((800., 600.), (0., 1000.), (0., 0.));
        let chain = [strip, outer];
        assert_eq!(
            notches((1., 1.), [u32::MAX; 2], &chain, (900., 700.)),
            (200., 80.)
        );
        assert_eq!(
            notches((1., 1.), [0, u32::MAX], &chain, (900., 700.)),
            (0., 600.)
        );
    }

    #[test]
    fn precision_wheels_preserve_distance_at_each_display_scale() {
        for scale in [1., 1.25, 1.5, 2.] {
            assert_eq!(
                pixels(
                    winit::dpi::PhysicalPosition::new(3.5 * scale, -12.25 * scale),
                    scale
                ),
                (-3.5, 12.25)
            );
        }
    }
}
