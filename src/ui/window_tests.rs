use super::window::{
    test_filtered_paths, test_next_selected_index, test_normalized_selected_index,
};

#[test]
fn filters_paths_case_insensitively() {
    let paths = vec![
        "C:\\Work".to_string(),
        "D:\\Games".to_string(),
        "C:\\workspace\\PathWrap".to_string(),
    ];

    let result = test_filtered_paths(&paths, "WORK");
    assert_eq!(
        result,
        vec![
            "C:\\Work".to_string(),
            "C:\\workspace\\PathWrap".to_string()
        ]
    );
}

#[test]
fn keeps_all_paths_when_query_empty() {
    let paths = vec!["A".to_string(), "B".to_string()];
    assert_eq!(test_filtered_paths(&paths, ""), paths);
}

#[test]
fn normalizes_selected_index_when_out_of_range() {
    assert_eq!(test_normalized_selected_index(10, 3), 2);
    assert_eq!(test_normalized_selected_index(1, 0), 0);
}

#[test]
fn moves_selection_with_bounds() {
    assert_eq!(test_next_selected_index(0, 3, true, false), 0);
    assert_eq!(test_next_selected_index(0, 3, false, true), 1);
    assert_eq!(test_next_selected_index(2, 3, false, true), 2);
    assert_eq!(test_next_selected_index(5, 3, false, false), 2);
    assert_eq!(test_next_selected_index(0, 0, false, true), 0);
}
