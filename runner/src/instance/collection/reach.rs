//! A list's edges: when `reachstart` and `reachend` fire, and how far ahead
//! of them `reach-margin` says (@ref LLP 1145 D4).
use super::*;

/// One side of a `reach-margin`.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Length {
    Px(f64),
    /// Of the port's extent along the list's axis.
    Percent(f64),
}

/// How far past the port, before and after it, an edge row may be when its
/// edge fires. The default is one port each side.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Margin([Length; 2]);

impl Default for Margin {
    fn default() -> Self {
        Self([Length::Percent(100.0); 2])
    }
}

impl Margin {
    /// One or two nonnegative px or % lengths (the compiler's
    /// `lower-reach-margin`); one value is both sides.
    pub(super) fn parse(text: &str) -> Option<Self> {
        let length = |part: &str| {
            let part = part.to_ascii_lowercase();
            let (number, unit): (&str, fn(f64) -> Length) = match part.strip_suffix("px") {
                Some(n) => (n, Length::Px),
                None => (part.strip_suffix('%')?, Length::Percent),
            };
            let n = number.parse::<f64>().ok()?;
            (n.is_finite() && n >= 0.0).then(|| unit(n))
        };
        let parts: Vec<_> = text.split_ascii_whitespace().map(length).collect();
        match parts[..] {
            [Some(both)] => Some(Self([both; 2])),
            [Some(before), Some(after)] => Some(Self([before, after])),
            _ => None,
        }
    }
    /// The margin in px for a port of `port` along the list's axis.
    pub(super) fn resolve(self, port: f64) -> [f64; 2] {
        self.0.map(|side| match side {
            Length::Px(n) => n,
            Length::Percent(n) => port * n / 100.0,
        })
    }
}

impl Collection {
    /// Which edge rows are within the reach margin of the port. Pins never
    /// qualify an edge. Re-arm only after the geometric window (one port
    /// each side, what a report mounts) is measured: replacement estimates
    /// cannot manufacture a temporary edge exit. A margin wider than that
    /// window reaches rows never mounted, so their estimates decide it.
    pub(super) fn geometric_edges(&mut self) -> Result<[bool; 2], InstanceError> {
        let mut reached = [false; 2];
        if let Some(g) = &self.geometry {
            if self.index.len() > 0 {
                let window = self
                    .index
                    .window(g.offset, g.port_main, [None, None])
                    .map_err(index_error)?;
                let reach = self
                    .index
                    .window_led(
                        g.offset,
                        g.port_main,
                        self.reach.resolve(g.port_main),
                        [None, None],
                    )
                    .map_err(index_error)?;
                reached = [0, self.index.len() - 1]
                    .map(|i| reach.segments.iter().any(|range| range.contains(&i)));
                if self.edge_armed != [true; 2] && self.index.range_measured(window.overscan) {
                    for (armed, reached) in self.edge_armed.iter_mut().zip(reached) {
                        *armed |= !reached;
                    }
                }
            }
        }
        Ok(reached)
    }
    /// The edge a report's geometry reached, if armed and handled.
    pub(super) fn edge_event(
        &mut self,
        fill: CollectionFill,
    ) -> Result<Option<CollectionEdges>, InstanceError> {
        let reached = self.geometric_edges()?;
        let ready = [0, 1].map(|i| reached[i] && self.edge_armed[i] && self.edge_handlers[i]);
        let edge = (0..2).find(|&i| ready[i]);
        // A build-only report runs no action (LLP 1072 §5): the edge stays
        // armed and the report is pending, so the next report runs it.
        if fill.create_only && edge.is_some() {
            self.pending = true;
            return Ok(None);
        }
        Ok(edge.map(|i| {
            self.edge_armed[i] = false;
            CollectionEdges {
                first: [EventKind::Reachstart, EventKind::Reachend][i],
                // End remains armed until an action actually consumes it.
                end_after_noop: ready[0] && ready[1],
            }
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_margin_is_one_or_two_nonnegative_lengths() {
        assert_eq!(Margin::parse("100%"), Some(Margin::default()));
        assert_eq!(Margin::parse("200%").unwrap().resolve(300.0), [600.0; 2]);
        assert_eq!(
            Margin::parse(" 0PX  50% ").unwrap().resolve(300.0),
            [0.0, 150.0]
        );
        assert_eq!(Margin::parse("1.5e2px").unwrap().resolve(10.0), [150.0; 2]);
        for bad in [
            "",
            "10",
            "10em",
            "-1px",
            "1px 2px 3px",
            "auto",
            "NaN%",
            "infpx",
        ] {
            assert_eq!(Margin::parse(bad), None, "{bad}");
        }
    }
}
