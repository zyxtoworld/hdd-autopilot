use super::{TileMove, apply_tile_move, solve};

fn apply_all(mut board: Vec<i32>, size: i32, path: &[TileMove]) -> Vec<i32> {
    for direction in path {
        board = apply_tile_move(&board, size, *direction).unwrap();
    }
    board
}

#[test]
fn tile_direction_matches_server_semantics() {
    let board = vec![8, 7, 1, 5, 0, 2, 4, 6, 3];

    assert_eq!(
        apply_tile_move(&board, 3, TileMove::Up).unwrap(),
        vec![8, 7, 1, 5, 6, 2, 4, 0, 3]
    );
}

#[test]
fn solves_user_easy_board() {
    let board = vec![8, 7, 1, 5, 0, 2, 4, 6, 3];
    let path = solve(&board, 3).unwrap();

    assert_eq!(apply_all(board, 3, &path), vec![1, 2, 3, 4, 5, 6, 7, 8, 0]);
}

#[test]
fn solved_board_has_empty_path() {
    let path = solve(&[1, 2, 3, 4, 5, 6, 7, 8, 0], 3).unwrap();

    assert!(path.is_empty());
}

#[test]
fn rejects_unsolvable_board() {
    let error = solve(&[1, 2, 3, 4, 5, 6, 8, 7, 0], 3).unwrap_err();

    assert!(error.contains("不可解"));
}
