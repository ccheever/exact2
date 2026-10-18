//! A late drawable may finish only the display frame this canvas still owes.
#[derive(Default)]
pub(super) struct Retry {
    pub frame: Option<(f32, f32, f32, f64)>,
    drawn: bool,
}

impl Retry {
    pub fn begin(&mut self) {
        self.frame = None;
        self.drawn = false;
    }

    pub fn recorded(&mut self, frame: (f32, f32, f32, f64), starved: bool) {
        self.frame = Some(frame);
        self.drawn |= !starved;
    }

    pub fn eligible(&self, buffered: bool, starved: bool, landed: bool) -> bool {
        self.frame.is_some() && !self.drawn && !buffered && starved && landed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn staggered_windows_finish_once_and_leave_buffered_siblings_alone() {
        let frame = (100., 200., 2., 10.);
        let mut a = Retry::default();
        let mut b = Retry::default();
        let mut buffered = Retry::default();
        for canvas in [&mut a, &mut b, &mut buffered] {
            canvas.begin();
            canvas.recorded(frame, true);
        }
        assert!(!a.eligible(false, true, false));
        assert!(b.eligible(false, true, true));
        b.recorded(frame, false);
        assert!(!b.eligible(false, false, true));
        assert!(!b.eligible(false, true, true));
        assert!(a.eligible(false, true, true));
        a.recorded(frame, false);
        assert!(!a.eligible(false, true, true));
        assert!(!buffered.eligible(true, true, true));
        a.begin();
        assert!(!a.eligible(false, true, true));
        a.recorded((100., 200., 2., 20.), true);
        assert!(a.eligible(false, true, true));
        assert_eq!(a.frame.unwrap().3, 20.);
    }
}
