use crate::model::MemoryCard;

use super::MemorySolver;

fn card(index: i32, symbol: i32) -> MemoryCard {
    MemoryCard { index, symbol }
}

#[test]
fn pairs_active_card_when_match_is_known() {
    let mut solver = MemorySolver::new();
    solver.remember(&card(0, 3));
    solver.remember(&card(5, 3));

    assert_eq!(solver.choose_next(12, &[], &[card(0, 3)]), Some(5));
}

#[test]
fn discovers_unknown_when_active_match_is_not_known() {
    let mut solver = MemorySolver::new();
    solver.remember(&card(0, 3));

    assert_eq!(solver.choose_next(12, &[], &[card(0, 3)]), Some(1));
}

#[test]
fn avoids_last_mismatch_and_opens_known_hidden_pair() {
    let mut solver = MemorySolver::new();
    solver.remember(&card(2, 2));
    solver.remember(&card(7, 2));
    solver.remember(&card(3, 0));

    assert_eq!(
        solver.choose_next(12, &[0, 1], &[card(2, 2), card(3, 0)]),
        Some(7)
    );
}

#[test]
fn skips_matched_cards() {
    let mut solver = MemorySolver::new();
    solver.remember(&card(0, 3));
    solver.remember(&card(1, 3));
    solver.remember(&card(2, 2));

    assert_eq!(solver.choose_next(6, &[0, 1], &[]), Some(3));
}
