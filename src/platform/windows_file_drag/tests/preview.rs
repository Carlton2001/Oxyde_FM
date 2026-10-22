//! The source must hide its ghost before entering a potentially blocking foreign Drop.
use super::*;
use std::num::NonZeroIsize;

struct TestWindow(HWND);
impl TestWindow {
    fn new() -> Self {
        Self(
            unsafe {
                CreateWindowExW(
                    WINDOW_EX_STYLE(0),
                    w!("STATIC"),
                    w!("Oxyde drag lifetime test"),
                    WS_POPUP,
                    0,
                    0,
                    1,
                    1,
                    None,
                    None,
                    None,
                    None,
                )
            }
            .unwrap(),
        )
    }
    fn raw(&self) -> RawWindowHandle {
        RawWindowHandle::Win32(raw_window_handle::Win32WindowHandle::new(
            NonZeroIsize::new(self.0 .0 as isize).unwrap(),
        ))
    }
}
impl Drop for TestWindow {
    fn drop(&mut self) {
        let _ = unsafe { DestroyWindow(self.0) };
    }
}

#[test]
fn release_hides_the_preview_before_drop_without_ending_the_transfer() {
    for cancel in [false, true] {
        let owner = TestWindow::new();
        let ghost = TestWindow::new();
        let raw = ghost.raw();
        // Show a one-pixel, nonactivating surface; never move the user's pointer.
        unsafe { SetWindowPos(ghost.0, None, 0, 0, 1, 1, SWP_NOACTIVATE | SWP_SHOWWINDOW) }
            .unwrap();
        let events = Rc::new(RefCell::new(Vec::new()));
        let observed = events.clone();
        let source: IDropSource = Source {
            owner: owner.0,
            preview_dismissed: Cell::new(false),
            feedback: Rc::new(RefCell::new(Box::new(move |feedback: FileDragFeedback| {
                observed
                    .borrow_mut()
                    .push((feedback.dismiss_preview, feedback.finished));
                if feedback.dismiss_preview {
                    dismiss_ghost(raw).unwrap();
                }
            }))),
        }
        .into();
        assert_eq!(unsafe { source.QueryContinueDrag(false, MK_LBUTTON) }, S_OK);
        assert!(events.borrow().is_empty());
        let keys = if cancel {
            MK_LBUTTON
        } else {
            MODIFIERKEYS_FLAGS(0)
        };
        assert_eq!(
            unsafe { source.QueryContinueDrag(cancel, keys) },
            if cancel {
                DRAGDROP_S_CANCEL
            } else {
                DRAGDROP_S_DROP
            }
        );
        // No message pump and no foreign Drop completion is needed to hide it.
        assert!(!unsafe { IsWindowVisible(ghost.0) }.as_bool());
        assert_eq!(&*events.borrow(), &[(true, false)]);
        unsafe { source.GiveFeedback(DROPEFFECT_COPY) }
            .ok()
            .unwrap();
        assert_eq!(events.borrow().last(), Some(&(true, false)));
    }
}

#[test]
fn a_dismissed_preview_cannot_be_reshown_by_queued_or_late_placements() {
    let ghost = TestWindow::new();
    place_ghost(ghost.raw(), [1.0, 1.0], 1.0, true).unwrap();
    dismiss_ghost(ghost.raw()).unwrap();
    unsafe {
        SendMessageW(ghost.0, PLACE, Some(WPARAM(0)), Some(LPARAM(0)));
    }
    flush_dismissals();
    assert!(!unsafe { IsWindowVisible(ghost.0) }.as_bool());
    place_ghost(ghost.raw(), [1.0, 1.0], 1.0, true).unwrap();
    unsafe {
        SendMessageW(ghost.0, PLACE, Some(WPARAM(0)), Some(LPARAM(0)));
    }
    assert!(!unsafe { IsWindowVisible(ghost.0) }.as_bool());
    assert!(!PLACEMENTS.with(|placements| placements.borrow().contains_key(&(ghost.0 .0 as isize))));
    let id = ghost.0 .0 as isize;
    drop(ghost);
    assert!(!DISMISSED.with(|dismissed| dismissed.borrow().contains(&id)));
}
