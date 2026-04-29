use std::collections::{BTreeMap, BTreeSet};

use crate::model::MemoryCard;

#[derive(Debug, Clone, Default)]
pub struct MemorySolver {
    known_by_symbol: BTreeMap<i32, BTreeSet<i32>>,
}

impl MemorySolver {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn remember(&mut self, card: &MemoryCard) {
        if card.index < 0 || card.symbol < 0 {
            return;
        }
        self.known_by_symbol
            .entry(card.symbol)
            .or_default()
            .insert(card.index);
    }

    pub fn remember_many<'a>(&mut self, cards: impl IntoIterator<Item = &'a MemoryCard>) {
        for card in cards {
            self.remember(card);
        }
    }

    pub fn choose_next(
        &self,
        total_cards: i32,
        matched_indices: &[i32],
        currently_revealed: &[MemoryCard],
    ) -> Option<i32> {
        if total_cards <= 0 {
            return None;
        }
        let matched = matched_indices.iter().copied().collect::<BTreeSet<_>>();
        let revealed = currently_revealed
            .iter()
            .map(|card| card.index)
            .collect::<BTreeSet<_>>();

        if let [active] = currently_revealed {
            if let Some(other) = self
                .known_by_symbol
                .get(&active.symbol)
                .and_then(|indices| {
                    indices
                        .iter()
                        .copied()
                        .find(|index| *index != active.index && !matched.contains(index))
                })
            {
                return Some(other);
            }
            return self.first_unknown(total_cards, &matched, &revealed);
        }

        if let Some(index) = self.first_known_pair(&matched, &revealed) {
            return Some(index);
        }
        self.first_unknown(total_cards, &matched, &revealed)
            .or_else(|| self.first_available(total_cards, &matched, &revealed))
    }

    fn first_known_pair(&self, matched: &BTreeSet<i32>, revealed: &BTreeSet<i32>) -> Option<i32> {
        for indices in self.known_by_symbol.values() {
            let unmatched = indices
                .iter()
                .copied()
                .filter(|index| !matched.contains(index))
                .collect::<Vec<_>>();
            if unmatched.len() < 2 {
                continue;
            }
            if let Some(index) = unmatched
                .iter()
                .copied()
                .find(|index| !revealed.contains(index))
            {
                return Some(index);
            }
        }
        None
    }

    fn first_unknown(
        &self,
        total_cards: i32,
        matched: &BTreeSet<i32>,
        revealed: &BTreeSet<i32>,
    ) -> Option<i32> {
        let known = self.known_indices();
        (0..total_cards).find(|index| {
            !matched.contains(index) && !revealed.contains(index) && !known.contains(index)
        })
    }

    fn first_available(
        &self,
        total_cards: i32,
        matched: &BTreeSet<i32>,
        revealed: &BTreeSet<i32>,
    ) -> Option<i32> {
        (0..total_cards).find(|index| !matched.contains(index) && !revealed.contains(index))
    }

    fn known_indices(&self) -> BTreeSet<i32> {
        self.known_by_symbol
            .values()
            .flat_map(|indices| indices.iter().copied())
            .collect()
    }
}
