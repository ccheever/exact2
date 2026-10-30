//! A list's edges: when `reachstart` and `reachend` fire, and how far ahead
//! of them `reach-margin` and the window's travel lead say (@ref LLP 1145 D4,
//! LLP 1027.004 D3).
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
    /// The edge band before and after a port of `port` moving at
    /// `velocity`: the margin, and toward the side it travels the lead the
    /// window takes past its overscan ([`travel_lead`]). The default band
    /// is then the window's own lead, so a row the window mounts for a fling
    /// is within it; a set margin keeps its distance from the rest band.
    fn band(self, port: f64, velocity: f64) -> [f64; 2] {
        let mut band = self.resolve(port);
        if velocity != 0.0 {
            band[usize::from(velocity > 0.0)] += travel_lead(port, velocity);
        }
        band
    }
}

impl Collection {
    /// Which edge rows are within the edge band ([`Margin::band`]) of a port
    /// moving at `velocity`. Pins never qualify an edge.
    ///
    /// An edge re-arms once outside the band of the report it is in, or,
    /// once its row is another than the one it fired at (a page landed past
    /// it), outside the band at rest: a page smaller than a fling's lead
    /// would otherwise leave the new edge inside the band, disarmed, until
    /// the reader left it. Rows that change elsewhere (a message appended to
    /// a chat whose reader is at its start) do not re-arm it. Either waits
    /// until the geometric window (one port each side, what a report at rest
    /// mounts) is measured: replacement estimates cannot manufacture a
    /// temporary edge exit. A band wider than that window reaches rows not
    /// mounted or not yet measured, so their estimates decide it.
    pub(super) fn geometric_edges(&mut self, velocity: f64) -> Result<[bool; 2], InstanceError> {
        let mut reached = [false; 2];
        if let Some(g) = &self.geometry {
            if self.index.len() > 0 {
                let window = self
                    .index
                    .window(g.offset, g.port_main, [None, None])
                    .map_err(index_error)?;
                let within = |band| -> Result<[bool; 2], InstanceError> {
                    let reach = self
                        .index
                        .window_led(g.offset, g.port_main, band, [None, None])
                        .map_err(index_error)?;
                    Ok([0, self.index.len() - 1]
                        .map(|i| reach.segments.iter().any(|range| range.contains(&i))))
                };
                reached = within(self.reach.band(g.port_main, velocity))?;
                if self.edge_armed != [true; 2] && self.index.range_measured(window.overscan) {
                    let at_rest = within(self.reach.band(g.port_main, 0.0))?;
                    for i in 0..2 {
                        let row = self.index.key([0, self.index.len() - 1][i]);
                        let paged = self.edge_fired[i].as_deref() != row;
                        self.edge_armed[i] |= !reached[i] || (paged && !at_rest[i]);
                    }
                }
            }
        }
        Ok(reached)
    }
    /// An edge fired: disarmed until it re-arms ([`Collection::geometric_edges`]).
    pub(super) fn disarm_edge(&mut self, i: usize) {
        self.edge_armed[i] = false;
        let row = [0, self.index.len().saturating_sub(1)][i];
        self.edge_fired[i] = self.index.shared_key(row).cloned();
    }
    /// The edge a report's geometry reached, if armed and handled.
    pub(super) fn edge_event(
        &mut self,
        fill: CollectionFill,
    ) -> Result<Option<CollectionEdges>, InstanceError> {
        let reached = self.geometric_edges(fill.velocity)?;
        let ready = [0, 1].map(|i| reached[i] && self.edge_armed[i] && self.edge_handlers[i]);
        let edge = (0..2).find(|&i| ready[i]);
        // A build-only report runs no action (LLP 1072 §5): the edge stays
        // armed and the report is pending, so the next report runs it.
        if fill.create_only && edge.is_some() {
            self.pending = true;
            return Ok(None);
        }
        Ok(edge.map(|i| {
            self.disarm_edge(i);
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
        let margin = Margin::parse("10px 50%").unwrap();
        assert_eq!(margin.band(300.0, 0.0), [10.0, 150.0]);
        assert_eq!(margin.band(300.0, -400.0), [110.0, 150.0]);
        assert_eq!(margin.band(300.0, 4000.0), [10.0, 750.0], "two ports");
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
