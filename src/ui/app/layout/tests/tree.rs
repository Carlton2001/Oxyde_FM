use super::*;
use crate::ui::workspace::{LayoutAxis, LayoutNode, PaneState, TabState};

fn pane(path: &str) -> (String, LayoutNode) {
    let pane = PaneState::new(TabState::new(path.into(), path.into()));
    (pane.id.clone(), LayoutNode::Pane(Box::new(pane)))
}

fn split(axis: LayoutAxis, children: Vec<LayoutNode>) -> LayoutNode {
    let weights = vec![1.0; children.len()];
    LayoutNode::Split { id: "split".into(), axis, children, weights }
}

#[::core::prelude::v1::test]
fn only_the_last_pane_of_every_split_reaches_the_window_corner() {
    let (a, node_a) = pane("a");
    let (b, node_b) = pane("b");
    let (c, node_c) = pane("c");
    let layout = split(LayoutAxis::Horizontal, vec![node_a, split(LayoutAxis::Vertical, vec![node_b, node_c])]);
    assert_eq!(ends_layout(&layout, &a), Some(false));
    assert_eq!(ends_layout(&layout, &b), Some(false));
    assert_eq!(ends_layout(&layout, &c), Some(true));
    assert_eq!(ends_layout(&layout, "missing"), None);
}

#[::core::prelude::v1::test]
fn a_single_pane_reaches_the_window_corner() {
    let (a, layout) = pane("a");
    assert_eq!(ends_layout(&layout, &a), Some(true));
}

#[::core::prelude::v1::test]
fn equalizing_a_separator_only_shares_its_two_neighbours_space() {
    let mut weights = [1.0, 3.0, 2.0];
    assert!(equalize_pair(&mut weights, 0));
    assert_eq!(weights, [2.0, 2.0, 2.0]);
    let mut weights = [1.0, 3.0, 5.0];
    assert!(equalize_pair(&mut weights, 1));
    assert_eq!(weights, [1.0, 4.0, 4.0]);
    assert!(!equalize_pair(&mut weights, 2));
    assert_eq!(weights, [1.0, 4.0, 4.0]);
}
