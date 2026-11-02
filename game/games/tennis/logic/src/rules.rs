//! Tennis scoring: 15/30/40, deuce and advantage, games to FIRST_TO with the
//! serve alternating each game and the court alternating each point.
use exact_game::{Data, Resource};

pub const FIRST_TO: u32 = 4;

/// Near is you (positive z, facing -z); Far is Jev (negative z, facing +z).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Data)]
pub enum Side {
    #[default]
    Near,
    Far,
}
impl Side {
    pub fn other(self) -> Side {
        match self {
            Side::Near => Side::Far,
            Side::Far => Side::Near,
        }
    }
    /// The sign of z on this side's half of the court.
    pub fn half(self) -> f32 {
        match self {
            Side::Near => 1.0,
            Side::Far => -1.0,
        }
    }
    pub fn index(self) -> usize {
        self as usize
    }
    pub fn name(self) -> &'static str {
        match self {
            Side::Near => "You",
            Side::Far => "Jev",
        }
    }
    pub fn of_z(z: f32) -> Side {
        if z >= 0.0 {
            Side::Near
        } else {
            Side::Far
        }
    }
}

/// Where the point is.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Data)]
pub enum Phase {
    /// The server has the ball in hand.
    #[default]
    Ready,
    /// The ball is in the air above the server.
    Toss,
    /// The ball is live.
    Rally,
    /// The point is decided; the call shows until the next point.
    Dead,
    /// Someone reached FIRST_TO games.
    Over,
}

#[derive(Clone, Debug, Default, Resource)]
pub struct Match {
    pub games: [u32; 2],
    pub points: [u32; 2],
    pub server: Side,
    /// The first serve of this point has faulted.
    pub fault: bool,
    pub phase: Phase,
    /// Tick the phase began.
    pub since: u64,
    /// The umpire's last call, shown on the HUD.
    pub call: String,
    pub winner: Option<Side>,
    /// Strokes in the current rally, serve included.
    pub rally: u32,
    pub points_played: u32,
    pub longest_rally: u32,
}

impl Match {
    /// Points alternate courts: deuce (the receiver's right) on even totals.
    pub fn deuce_court(&self) -> bool {
        (self.points[0] + self.points[1]).is_multiple_of(2)
    }

    pub fn enter(&mut self, phase: Phase, tick: u64) {
        self.phase = phase;
        self.since = tick;
    }

    /// Point labels for (you, Jev).
    pub fn labels(&self) -> (String, String) {
        let [a, b] = self.points;
        let name = |p: u32| {
            ["0", "15", "30", "40"]
                .get(p as usize)
                .copied()
                .unwrap_or("40")
        };
        if a >= 3 && b >= 3 {
            return match a.cmp(&b) {
                std::cmp::Ordering::Equal => ("40".into(), "40".into()),
                std::cmp::Ordering::Greater => ("AD".into(), "40".into()),
                std::cmp::Ordering::Less => ("40".into(), "AD".into()),
            };
        }
        (name(a).into(), name(b).into())
    }

    /// Award the point; returns the announcement that follows it.
    pub fn award(&mut self, to: Side) -> String {
        let (s, o) = (to.index(), to.other().index());
        self.points[s] += 1;
        self.points_played += 1;
        self.longest_rally = self.longest_rally.max(self.rally);
        self.fault = false;
        let (a, b) = (self.points[s], self.points[o]);
        if a >= 4 && a >= b + 2 {
            self.games[s] += 1;
            self.points = [0, 0];
            self.server = self.server.other();
            if self.games[s] >= FIRST_TO {
                self.winner = Some(to);
                return format!("Game, set and match {}", to.name());
            }
            return format!("Game {}", to.name());
        }
        if a >= 3 && b >= 3 {
            return if a == b {
                "Deuce".into()
            } else {
                format!("Advantage {}", to.name())
            };
        }
        String::new()
    }

    /// Break point, set point or match point for the receiver or server, if any.
    pub fn pressure(&self) -> &'static str {
        let game = |s: usize| {
            let (a, b) = (self.points[s], self.points[1 - s]);
            a >= 3 && a > b
        };
        for side in [Side::Near, Side::Far] {
            if game(side.index()) {
                if self.games[side.index()] + 1 >= FIRST_TO {
                    return "match point";
                }
                if side != self.server {
                    return "break point";
                }
                return "game point";
            }
        }
        ""
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deuce_advantage_and_game() {
        let mut m = Match::default();
        for _ in 0..3 {
            m.award(Side::Near);
            m.award(Side::Far);
        }
        assert_eq!(m.labels(), ("40".into(), "40".into()));
        assert_eq!(m.award(Side::Far), "Advantage Jev");
        assert_eq!(m.labels(), ("40".into(), "AD".into()));
        assert_eq!(m.award(Side::Near), "Deuce");
        assert_eq!(m.award(Side::Near), "Advantage You");
        assert_eq!(m.award(Side::Near), "Game You");
        assert_eq!(m.games, [1, 0]);
        assert_eq!(m.server, Side::Far, "the serve alternates each game");
        assert_eq!(m.points, [0, 0]);
    }

    #[test]
    fn first_to_four_games() {
        let mut m = Match::default();
        let mut last = String::new();
        for _ in 0..16 {
            last = m.award(Side::Far);
        }
        assert_eq!(m.games, [0, 4]);
        assert_eq!(m.winner, Some(Side::Far));
        assert_eq!(last, "Game, set and match Jev");
    }
}
