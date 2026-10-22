//! Exercise the COM ABI without moving the user's mouse or opening the clipboard.
use super::*;
use std::rc::Rc;
use windows::Win32::UI::WindowsAndMessaging::*;

struct TestWindow(HWND);
impl TestWindow {
    fn new() -> Self {
        Self(
            unsafe {
                CreateWindowExW(
                    WINDOW_EX_STYLE(0),
                    w!("STATIC"),
                    w!("Oxyde drop test"),
                    WS_OVERLAPPED,
                    0,
                    0,
                    100,
                    100,
                    None,
                    None,
                    None,
                    None,
                )
            }
            .unwrap(),
        )
    }
}
impl Drop for TestWindow {
    fn drop(&mut self) {
        let _ = unsafe { DestroyWindow(self.0) };
    }
}

fn files(paths: &[String]) -> IDataObject {
    super::super::data::FileData {
        formats: Rc::new(RefCell::new(std::collections::BTreeMap::from([(
            CF_HDROP.0,
            crate::platform::windows_clipboard_write::file_bytes(paths),
        )]))),
    }
    .into()
}

#[test]
fn native_move_has_one_drop_and_never_authorizes_foreign_source_deletion() {
    let window = TestWindow::new();
    let events = Rc::new(RefCell::new(Vec::new()));
    let received = events.clone();
    let target: IDropTarget = Target {
        window: window.0,
        callback: Box::new(move |event| {
            received.borrow_mut().push((
                event.phase,
                event.paths.len(),
                event.control,
                event.shift,
            ));
            FileDropEffect::Move
        }),
        paths: RefCell::default(),
        allowed: Cell::new(DROPEFFECT_NONE),
        helper: None,
    }
    .into();
    let object = files(&[r"C:\one.txt".into(), r"D:\two.txt".into()]);
    let point = POINTL { x: 20, y: 20 };
    let mut effect = DROPEFFECT_COPY | DROPEFFECT_MOVE;
    unsafe { target.DragEnter(&object, MK_LBUTTON | MK_SHIFT, point, &mut effect) }.unwrap();
    assert_eq!(effect, DROPEFFECT_MOVE);
    unsafe { target.Drop(&object, MK_SHIFT, point, &mut effect) }.unwrap();
    assert_eq!(effect, DROPEFFECT_NONE);
    assert_eq!(
        &*events.borrow(),
        &[
            (FileDropPhase::Enter, 2, false, true),
            (FileDropPhase::Drop, 0, false, true),
            (FileDropPhase::Leave, 0, false, true),
        ]
    );
    let id = data::registered(w!("Performed DropEffect"));
    let mut storage = unsafe { object.GetData(&data::format(id)) }.unwrap();
    assert_eq!(
        data::read_global(&storage).unwrap()[..4],
        0_u32.to_le_bytes()
    );
    unsafe {
        ReleaseStgMedium(&mut storage);
    }
}

#[test]
fn invalid_final_object_cancels_the_drop_and_clears_the_ui_session() {
    let window = TestWindow::new();
    let events = Rc::new(RefCell::new(Vec::new()));
    let received = events.clone();
    let target: IDropTarget = Target {
        window: window.0,
        callback: Box::new(move |event| {
            received.borrow_mut().push(event.phase);
            FileDropEffect::Copy
        }),
        paths: RefCell::default(),
        allowed: Cell::new(DROPEFFECT_NONE),
        helper: None,
    }
    .into();
    let before = files(&[r"C:\one.txt".into()]);
    let after = files(&[r"C:\other.txt".into()]);
    let point = POINTL::default();
    let mut effect = DROPEFFECT_COPY;
    unsafe { target.DragEnter(&before, MK_LBUTTON, point, &mut effect) }.unwrap();
    unsafe { target.Drop(&after, MODIFIERKEYS_FLAGS(0), point, &mut effect) }.unwrap();
    assert_eq!(effect, DROPEFFECT_NONE);
    assert_eq!(
        &*events.borrow(),
        &[FileDropPhase::Enter, FileDropPhase::Leave]
    );
}

#[test]
fn target_cannot_return_an_operation_the_source_did_not_offer() {
    let window = TestWindow::new();
    let target: IDropTarget = Target {
        window: window.0,
        callback: Box::new(|_| FileDropEffect::Move),
        paths: RefCell::default(),
        allowed: Cell::new(DROPEFFECT_NONE),
        helper: None,
    }
    .into();
    let object = files(&[r"C:\one.txt".into()]);
    let mut effect = DROPEFFECT_COPY;
    unsafe { target.DragEnter(&object, MK_LBUTTON, POINTL::default(), &mut effect) }.unwrap();
    assert_eq!(effect, DROPEFFECT_NONE);
    unsafe { target.DragLeave() }.unwrap();
}
