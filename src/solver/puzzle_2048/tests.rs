use super::*;

#[test]
fn apply_move_merges_left_once_per_pair() {
    let outcome = apply_move(
        &[
            vec![2, 2, 2, 0],
            vec![0, 0, 0, 0],
            vec![0, 0, 0, 0],
            vec![0, 0, 0, 0],
        ],
        Direction::Left,
    );

    assert_eq!(outcome.board[0], vec![4, 2, 0, 0]);
    assert_eq!(outcome.score_delta, 4);
    assert!(outcome.moved);
}

#[test]
fn apply_move_merges_two_pairs_without_chain() {
    let outcome = apply_move(
        &[
            vec![2, 2, 2, 2],
            vec![0, 0, 0, 0],
            vec![0, 0, 0, 0],
            vec![0, 0, 0, 0],
        ],
        Direction::Left,
    );

    assert_eq!(outcome.board[0], vec![4, 4, 0, 0]);
    assert_eq!(outcome.score_delta, 8);
}

#[test]
fn apply_move_handles_columns() {
    let board = vec![vec![2, 0, 0], vec![2, 0, 0], vec![4, 0, 0]];
    let outcome = apply_move(&board, Direction::Up);

    assert_eq!(
        outcome.board,
        vec![vec![4, 0, 0], vec![4, 0, 0], vec![0, 0, 0]]
    );
}

#[test]
fn legal_moves_filters_unchanged_directions() {
    let board = vec![vec![2, 4, 8], vec![16, 32, 64], vec![128, 256, 512]];

    assert!(legal_moves(&board, DEFAULT_DIRECTIONS).is_empty());
}

#[test]
fn choose_next_move_matches_reference_even_when_immediate_win_exists() {
    let board = vec![vec![256, 256, 0], vec![0, 0, 0], vec![0, 0, 0]];

    assert_eq!(
        choose_next_move(&board, 512, 0.1, DEFAULT_DIRECTIONS),
        Some(Direction::Down)
    );
}

#[test]
fn choose_next_move_matches_reference_cases() {
    let cases = [
        (
            vec![vec![2, 0, 0], vec![4, 0, 0], vec![0, 0, 0]],
            Some(Direction::Down),
        ),
        (
            vec![vec![2, 4, 8], vec![16, 32, 64], vec![128, 256, 0]],
            Some(Direction::Down),
        ),
        (
            vec![vec![128, 64, 32], vec![16, 8, 4], vec![2, 0, 0]],
            Some(Direction::Right),
        ),
        (
            vec![vec![4, 2, 4], vec![8, 16, 32], vec![64, 128, 256]],
            None,
        ),
        (
            vec![vec![2, 2, 4], vec![8, 0, 8], vec![4, 2, 0]],
            Some(Direction::Down),
        ),
        (
            vec![vec![0, 2, 4], vec![0, 8, 16], vec![0, 32, 64]],
            Some(Direction::Left),
        ),
        (
            vec![
                vec![0, 0, 0, 0],
                vec![0, 2, 4, 0],
                vec![0, 8, 16, 0],
                vec![0, 32, 64, 128],
            ],
            Some(Direction::Right),
        ),
        (
            vec![
                vec![1024, 512, 256, 128],
                vec![64, 32, 16, 8],
                vec![4, 2, 0, 0],
                vec![0, 0, 0, 0],
            ],
            Some(Direction::Right),
        ),
        (
            vec![
                vec![2, 4, 2, 4],
                vec![4, 2, 4, 2],
                vec![8, 16, 32, 64],
                vec![128, 256, 512, 1024],
            ],
            None,
        ),
        (
            vec![
                vec![0, 0, 0, 0, 0],
                vec![0, 2, 4, 8, 0],
                vec![0, 16, 32, 64, 0],
                vec![0, 128, 256, 512, 0],
                vec![0, 0, 0, 0, 0],
            ],
            Some(Direction::Down),
        ),
        (
            vec![
                vec![4096, 2048, 1024, 512, 256],
                vec![128, 64, 32, 16, 8],
                vec![4, 2, 0, 0, 0],
                vec![0, 0, 0, 0, 0],
                vec![0, 0, 0, 0, 0],
            ],
            Some(Direction::Right),
        ),
    ];

    for (board, expected) in cases {
        assert_eq!(
            choose_next_move(&board, 512, 0.1, DEFAULT_DIRECTIONS),
            expected,
            "board: {board:?}",
        );
    }
}

#[test]
fn choose_next_move_supports_5x5_board() {
    let board = vec![
        vec![2, 0, 0, 0, 0],
        vec![2, 0, 0, 0, 0],
        vec![0, 0, 0, 0, 0],
        vec![0, 0, 0, 0, 0],
        vec![0, 0, 0, 0, 0],
    ];

    assert!(choose_next_move(&board, 4096, 0.1, DEFAULT_DIRECTIONS).is_some());
}
