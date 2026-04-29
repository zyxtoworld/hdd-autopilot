use super::*;

fn sample_givens() -> Vec<i32> {
    vec![
        6, 0, 5, 1, 0, 0, 0, 0, 0, 0, 0, 8, 4, 3, 0, 0, 1, 5, 0, 1, 9, 2, 5, 6, 0, 0, 0, 1, 0, 4,
        8, 9, 0, 2, 0, 6, 9, 5, 2, 0, 0, 1, 4, 0, 3, 8, 3, 0, 0, 2, 0, 7, 9, 1, 0, 0, 0, 6, 0, 2,
        0, 4, 9, 2, 9, 0, 3, 4, 7, 0, 0, 0, 0, 6, 0, 9, 0, 0, 1, 2, 7,
    ]
}

#[test]
fn solves_user_example() {
    let solution = solve(&sample_givens(), 9, 3).unwrap();

    assert_eq!(solution.len(), 81);
    assert!(solution.iter().all(|value| (1..=9).contains(value)));
    assert_eq!(solution[8], 2);
}

#[test]
fn plans_only_empty_or_wrong_editable_cells() {
    let givens = sample_givens();
    let mut user_board = givens.clone();
    user_board[8] = 2;
    let fills = solve_fills(&givens, &user_board, 9, 3).unwrap();

    assert_eq!(fills.len(), 35);
    assert!(!fills.iter().any(|fill| fill.row == 0 && fill.col == 8));
    assert!(fills.iter().all(|fill| {
        let index = fill.row as usize * 9 + fill.col as usize;
        givens[index] == 0
    }));
}

#[test]
fn directly_replaces_wrong_editable_cells() {
    let givens = sample_givens();
    let mut user_board = givens.clone();
    user_board[1] = 9;

    let fills = solve_fills(&givens, &user_board, 9, 3).unwrap();

    assert!(
        fills
            .iter()
            .any(|fill| fill.row == 0 && fill.col == 1 && fill.value != 9)
    );
}

#[test]
fn rejects_conflicting_givens() {
    let mut givens = sample_givens();
    givens[1] = 6;

    assert!(solve(&givens, 9, 3).is_err());
}

#[test]
fn rejects_wrong_fixed_cell_in_user_board() {
    let givens = sample_givens();
    let mut user_board = givens.clone();
    user_board[0] = 5;

    assert!(solve_fills(&givens, &user_board, 9, 3).is_err());
}
