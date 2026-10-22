use super::{data, hwnd};
use crate::platform::{FileDropEffect, FileDropPhase, NativeFileDrop};
use raw_window_handle::RawWindowHandle;
use std::cell::{Cell, RefCell};
use windows::{
    core::*,
    Win32::{
        Foundation::*,
        Graphics::Gdi::ScreenToClient,
        System::{Com::*, Ole::*, SystemServices::*},
        UI::Shell::*,
    },
};

#[implement(IDropTarget)]
struct Target {
    window: HWND,
    callback: Box<dyn Fn(NativeFileDrop) -> FileDropEffect>,
    paths: RefCell<Vec<String>>,
    allowed: Cell<DROPEFFECT>,
    helper: Option<IDropTargetHelper>,
}

impl Target {
    fn dispatch(
        &self,
        phase: FileDropPhase,
        point: &POINTL,
        keys: MODIFIERKEYS_FLAGS,
    ) -> DROPEFFECT {
        let mut position = POINT {
            x: point.x,
            y: point.y,
        };
        if !unsafe { ScreenToClient(self.window, &mut position) }.as_bool() {
            return DROPEFFECT_NONE;
        }
        if phase != FileDropPhase::Leave && self.paths.borrow().is_empty() {
            return DROPEFFECT_NONE;
        }
        let allowed = self.allowed.get();
        let paths = if phase == FileDropPhase::Enter {
            self.paths.borrow().clone()
        } else {
            Vec::new()
        };
        let effect = (self.callback)(NativeFileDrop {
            phase,
            paths,
            position: [position.x, position.y],
            control: keys.0 & MK_CONTROL.0 != 0,
            shift: keys.0 & MK_SHIFT.0 != 0,
            can_copy: allowed.0 & DROPEFFECT_COPY.0 != 0,
            can_move: allowed.0 & DROPEFFECT_MOVE.0 != 0,
        });
        match effect {
            FileDropEffect::Copy if allowed.0 & DROPEFFECT_COPY.0 != 0 => DROPEFFECT_COPY,
            FileDropEffect::Move if allowed.0 & DROPEFFECT_MOVE.0 != 0 => DROPEFFECT_MOVE,
            _ => DROPEFFECT_NONE,
        }
    }
}

#[allow(non_snake_case)]
impl IDropTarget_Impl for Target_Impl {
    fn DragEnter(
        &self,
        object: Ref<IDataObject>,
        keys: MODIFIERKEYS_FLAGS,
        point: &POINTL,
        output: *mut DROPEFFECT,
    ) -> Result<()> {
        let output = unsafe { output.as_mut() }.ok_or_else(|| Error::from(E_POINTER))?;
        self.allowed.set(*output);
        *output = DROPEFFECT_NONE;
        self.paths.borrow_mut().clear();
        let object = object.ok()?;
        if let Ok(paths) = data::read_paths(object) {
            *self.paths.borrow_mut() = paths;
            *output = self.dispatch(FileDropPhase::Enter, point, keys);
        }
        if let Some(helper) = &self.helper {
            let _ = unsafe {
                helper.DragEnter(
                    self.window,
                    object,
                    &POINT {
                        x: point.x,
                        y: point.y,
                    },
                    *output,
                )
            };
        }
        Ok(())
    }
    fn DragOver(
        &self,
        keys: MODIFIERKEYS_FLAGS,
        point: &POINTL,
        output: *mut DROPEFFECT,
    ) -> Result<()> {
        let output = unsafe { output.as_mut() }.ok_or_else(|| Error::from(E_POINTER))?;
        *output = self.dispatch(FileDropPhase::Over, point, keys);
        if let Some(helper) = &self.helper {
            let _ = unsafe {
                helper.DragOver(
                    &POINT {
                        x: point.x,
                        y: point.y,
                    },
                    *output,
                )
            };
        }
        Ok(())
    }
    fn DragLeave(&self) -> Result<()> {
        self.dispatch(
            FileDropPhase::Leave,
            &POINTL::default(),
            MODIFIERKEYS_FLAGS(0),
        );
        self.paths.borrow_mut().clear();
        if let Some(helper) = &self.helper {
            let _ = unsafe { helper.DragLeave() };
        }
        Ok(())
    }
    fn Drop(
        &self,
        object: Ref<IDataObject>,
        keys: MODIFIERKEYS_FLAGS,
        point: &POINTL,
        output: *mut DROPEFFECT,
    ) -> Result<()> {
        let output = unsafe { output.as_mut() }.ok_or_else(|| Error::from(E_POINTER))?;
        *output = DROPEFFECT_NONE;
        let object = object.ok()?;
        // Validate the final object, rather than trusting a previous DragEnter.
        let paths = data::read_paths(object).unwrap_or_default();
        if paths != *self.paths.borrow() {
            self.paths.borrow_mut().clear();
        }
        let accepted = self.dispatch(FileDropPhase::Drop, point, keys);
        // Oxyde owns asynchronous copies AND moves, including conflict handling.
        // An optimized move never authorizes the foreign source to delete files.
        *output = if accepted == DROPEFFECT_MOVE {
            DROPEFFECT_NONE
        } else {
            accepted
        };
        data::set_effect(
            object,
            data::registered(w!("Performed DropEffect")),
            *output,
        );
        if let Some(helper) = &self.helper {
            let _ = unsafe {
                helper.Drop(
                    object,
                    &POINT {
                        x: point.x,
                        y: point.y,
                    },
                    *output,
                )
            };
        }
        self.dispatch(FileDropPhase::Leave, point, keys);
        self.paths.borrow_mut().clear();
        Ok(())
    }
}

#[cfg(test)]
#[path = "tests/target.rs"]
mod tests;

pub(in crate::platform) fn install_target(
    raw: RawWindowHandle,
    callback: Box<dyn Fn(NativeFileDrop) -> FileDropEffect>,
) -> std::result::Result<(), String> {
    let window = hwnd(raw)?;
    let target: IDropTarget = Target {
        window,
        callback,
        paths: RefCell::default(),
        allowed: Cell::new(DROPEFFECT_NONE),
        helper: unsafe { CoCreateInstance(&CLSID_DragDropHelper, None, CLSCTX_INPROC_SERVER) }.ok(),
    }
    .into();
    unsafe {
        RevokeDragDrop(window).map_err(|error| error.to_string())?;
        RegisterDragDrop(window, &target).map_err(|error| error.to_string())
    }
    // GPUI revokes the replacement target when destroying its HWND.
}
