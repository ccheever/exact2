//! Windows' click interval and screen-pixel rectangle, including higher-order clicks.
//! https://devblogs.microsoft.com/oldnewthing/20041018-00/?p=37543
use std::time::{Duration, Instant};

#[derive(Default)]
pub(super) struct Clicks {
    previous: Option<(Instant, (f64, f64), u32)>,
    size: (i32, i32),
}

impl Clicks {
    pub(super) fn clear(&mut self) {
        self.previous = None;
    }

    pub(super) fn moved(&mut self, point: (f64, f64)) {
        if self
            .previous
            .is_some_and(|(_, origin, _)| !inside(point, origin, self.size))
        {
            self.clear();
        }
    }

    pub(super) fn down(&mut self, point: (f64, f64), now: Instant) -> u32 {
        use windows_sys::Win32::UI::Input::KeyboardAndMouse::GetDoubleClickTime;
        use windows_sys::Win32::UI::WindowsAndMessaging::*;
        // These settings are live user preferences; reading them changes no state.
        #[allow(unsafe_code)]
        let (timeout, size) = unsafe {
            (
                GetDoubleClickTime(),
                (
                    GetSystemMetrics(SM_CXDOUBLECLK),
                    GetSystemMetrics(SM_CYDOUBLECLK),
                ),
            )
        };
        self.at(point, now, Duration::from_millis(u64::from(timeout)), size)
    }

    fn at(&mut self, point: (f64, f64), now: Instant, timeout: Duration, size: (i32, i32)) -> u32 {
        self.size = size;
        let count = self
            .previous
            .filter(|(then, origin, _)| {
                now.saturating_duration_since(*then) <= timeout && inside(point, *origin, size)
            })
            .map_or(1, |(_, _, count)| count % 3 + 1);
        self.previous = Some((now, point, count));
        count
    }
}

fn inside(point: (f64, f64), origin: (f64, f64), size: (i32, i32)) -> bool {
    point.0 >= origin.0 - f64::from(size.0 / 2)
        && point.0 < origin.0 + f64::from(size.0 / 2)
        && point.1 >= origin.1 - f64::from(size.1 / 2)
        && point.1 < origin.1 + f64::from(size.1 / 2)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn clicks_follow_system_time_and_rectangle_and_reset_after_interruption() {
        let mut clicks = Clicks::default();
        let start = Instant::now();
        let limit = Duration::from_millis(400);
        assert_eq!(clicks.at((10., 10.), start, limit, (4, 6)), 1);
        assert_eq!(clicks.at((11., 12.), start + limit, limit, (4, 6)), 2);
        assert_eq!(clicks.at((11., 12.), start + limit * 2, limit, (4, 6)), 3);
        assert_eq!(clicks.at((11., 12.), start + limit * 2, limit, (4, 6)), 1);
        assert_eq!(clicks.at((13., 12.), start + limit * 2, limit, (4, 6)), 1);
        assert_eq!(clicks.at((13., 12.), start + limit * 4, limit, (4, 6)), 1);
        clicks.clear();
        assert_eq!(clicks.at((13., 12.), start + limit * 4, limit, (4, 6)), 1);
        clicks.moved((20., 12.));
        assert_eq!(clicks.at((13., 12.), start + limit * 4, limit, (4, 6)), 1);
    }
}
