//! 显式根优先于邻格扫描，并且左右两轴独立。
use rpx_engine::arena::CellId;
use rpx_engine::heads::{find_left_head, find_top_head, HeadCell, HeadState};

fn grid(left_head: Option<&str>, top_head: Option<&str>) -> (HeadState, CellId, CellId, CellId) {
    let mut g = HeadState::new();
    let mut root = HeadCell::new(0, 0, 16, -91);
    root.is_extender = true;
    g.push(root);
    let left = g.push(HeadCell::new(2, 1, 18, -91));
    let top = g.push(HeadCell::new(1, 2, 19, -91));
    let mut child = HeadCell::new(2, 2, 16, -91);
    child.declared_left = left_head.map(str::to_string);
    child.declared_top = top_head.map(str::to_string);
    let cell = g.push(child);
    g.set_current(cell);
    (g, left, top, cell)
}

#[test]
fn explicit_left_root_stops_inheritance_without_changing_the_top_head() {
    let (mut g, left, top, cell) = grid(None, None);
    assert_eq!(find_left_head(&mut g, cell).unwrap(), left);
    assert_eq!(find_top_head(&mut g, cell).unwrap(), top);
    let (mut g, _, top, cell) = grid(Some("`0"), None);
    assert_eq!(find_left_head(&mut g, cell).unwrap(), CellId::ROOT);
    assert_eq!(find_top_head(&mut g, cell).unwrap(), top);
}

#[test]
fn explicit_top_root_stops_inheritance_without_changing_the_left_head() {
    let (mut g, _, top, cell) = grid(None, None);
    assert_eq!(find_top_head(&mut g, cell).unwrap(), top);
    let (mut g, left, _, cell) = grid(None, Some(" `0 "));
    assert_eq!(find_top_head(&mut g, cell).unwrap(), CellId::ROOT);
    assert_eq!(find_left_head(&mut g, cell).unwrap(), left);
}
