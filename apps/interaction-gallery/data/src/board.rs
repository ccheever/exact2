//! A three-column board for the cross-list reorder fixture (LLP 1094 D12,
//! `host/web-js/conformance/reorder-group.contract`): cards keyed by string
//! ids, the third column empty, and one move that puts a card before
//! another or at a column's end.

/// The cards in board order, each `(id, title, column)`.
pub struct Board {
    cards: Vec<(String, String, String)>,
}

impl Default for Board {
    fn default() -> Board {
        let cards = [
            ("c1", "Draft the brief", "todo"),
            ("c2", "Pick the venue", "todo"),
            ("c3", "Book the band", "todo"),
            ("c4", "Print the posters", "todo"),
            ("c5", "Write the toasts", "todo"),
            ("c6", "Send invitations", "doing"),
            ("c7", "Order the cake", "doing"),
        ];
        Board {
            cards: cards
                .iter()
                .map(|(id, title, col)| (id.to_string(), title.to_string(), col.to_string()))
                .collect(),
        }
    }
}

impl Board {
    /// The cards in order: `id`, `title`, `col`.
    pub fn cards(&self) -> Vec<(&str, &str, &str)> {
        self.cards
            .iter()
            .map(|(id, title, col)| (id.as_str(), title.as_str(), col.as_str()))
            .collect()
    }

    /// Move `item` into `col` before the card `before`, or to the column's
    /// end when `before` is empty or names no card. An unknown item or an
    /// empty column name changes nothing.
    pub fn move_card(&mut self, item: &str, col: &str, before: &str) {
        let Some(from) = self.cards.iter().position(|(id, ..)| id == item) else {
            return;
        };
        if col.is_empty() || item == before {
            return;
        }
        let (id, title, _) = self.cards.remove(from);
        let at = self
            .cards
            .iter()
            .position(|(id, ..)| id == before)
            .or_else(|| {
                self.cards
                    .iter()
                    .rposition(|(.., c)| c == col)
                    .map(|i| i + 1)
            })
            .unwrap_or(self.cards.len());
        self.cards.insert(at, (id, title, col.to_string()));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_card_moves_before_another_or_to_a_column_end() {
        let mut b = Board::default();
        b.move_card("c1", "doing", "c7");
        let doing: Vec<_> = b
            .cards()
            .into_iter()
            .filter(|c| c.2 == "doing")
            .map(|c| c.0)
            .collect();
        assert_eq!(doing, ["c6", "c1", "c7"]);
        b.move_card("c2", "done", "");
        assert_eq!(b.cards().last().map(|c| (c.0, c.2)), Some(("c2", "done")));
        b.move_card("c3", "todo", "");
        let todo: Vec<_> = b
            .cards()
            .into_iter()
            .filter(|c| c.2 == "todo")
            .map(|c| c.0)
            .collect();
        assert_eq!(todo, ["c4", "c5", "c3"]);
    }
}
