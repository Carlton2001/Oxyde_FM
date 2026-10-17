use super::centered_origin;
use windows::Win32::Foundation::RECT;

fn rect(left: i32, top: i32, right: i32, bottom: i32) -> RECT {
    RECT { left, top, right, bottom }
}

#[test]
fn dialog_is_centered_over_its_owner() {
    let work = rect(0, 0, 1920, 1040);
    assert_eq!(centered_origin(rect(100, 100, 1100, 900), work, 400, 200), (400, 400));
}

#[test]
fn dialog_stays_inside_the_owner_work_area() {
    let work = rect(0, 0, 1920, 1040);
    assert_eq!(centered_origin(rect(-900, -50, 100, 300), work, 400, 200), (0, 25));
    assert_eq!(centered_origin(rect(1800, 900, 2400, 1300), work, 400, 200), (1520, 840));
}
