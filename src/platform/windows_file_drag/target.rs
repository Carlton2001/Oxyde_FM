use super::{data, hwnd, streams};
use crate::platform::{FileDropEffect, FileDropPhase, NativeFileDrop, StreamedFiles};
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
    /// Set when the source offers streams instead of paths; `paths` then name
    /// its top-level items in a staging folder written at Drop.
    listing: RefCell<Option<streams::Listing>>,
    /// The object `paths` were read from. A source that renders its files on
    /// request (an Explorer ZIP folder extracts them) is read once per drag,
    /// not at each entry of the pointer; the reference keeps its identity unique.
    source: RefCell<Option<IUnknown>>,
    allowed: Cell<DROPEFFECT>,
    helper: Option<IDropTargetHelper>,
}

impl Target {
    fn read_from(&self, object: &IDataObject) -> Option<IUnknown> {
        let identity = object.cast::<IUnknown>().ok()?;
        let same = self
            .source
            .borrow()
            .as_ref()
            .is_some_and(|source| source.as_raw() == identity.as_raw());
        same.then_some(identity)
    }
    /// Paths, or else the descriptor of streamed files. Never their contents.
    fn read(object: &IDataObject) -> (Vec<String>, Option<streams::Listing>) {
        match data::read_paths(object) {
            Ok(paths) => (paths, None),
            Err(_) => match streams::read_listing(object) {
                Some(listing) => (listing.top.clone(), Some(listing)),
                None => (Vec::new(), None),
            },
        }
    }
    fn dispatch(
        &self,
        phase: FileDropPhase,
        point: &POINTL,
        keys: MODIFIERKEYS_FLAGS,
        streamed: Option<StreamedFiles>,
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
            streamed,
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
        let offered = *output;
        *output = DROPEFFECT_NONE;
        let object = object.ok()?;
        if self.read_from(object).is_none() {
            let (paths, listing) = Target::read(object);
            *self.paths.borrow_mut() = paths;
            *self.listing.borrow_mut() = listing;
            *self.source.borrow_mut() = object.cast::<IUnknown>().ok();
        }
        // Streamed files are written by Oxyde: the source keeps them.
        self.allowed.set(if self.listing.borrow().is_some() {
            DROPEFFECT(offered.0 & DROPEFFECT_COPY.0)
        } else {
            offered
        });
        if !self.paths.borrow().is_empty() {
            *output = self.dispatch(FileDropPhase::Enter, point, keys, None);
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
        *output = self.dispatch(FileDropPhase::Over, point, keys, None);
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
            None,
        );
        // Paths stay with their source: the pointer may come back in this drag.
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
        // Validate the final object, rather than trusting a previous DragEnter
        // made with another one.
        if self.read_from(object).is_none() {
            let (paths, listing) = Target::read(object);
            // A streamed object read again would name another staging folder.
            if listing.is_some() || self.listing.borrow().is_some() || paths != *self.paths.borrow() {
                self.paths.borrow_mut().clear();
            }
        }
        let listing = self.listing.borrow_mut().take();
        let streamed = match listing {
            Some(listing) if !self.paths.borrow().is_empty() => {
                let streamed = streams::streamed(object, listing);
                if streamed.is_none() {
                    self.paths.borrow_mut().clear();
                }
                streamed
            }
            _ => None,
        };
        let accepted = self.dispatch(FileDropPhase::Drop, point, keys, streamed);
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
        self.dispatch(FileDropPhase::Leave, point, keys, None);
        self.paths.borrow_mut().clear();
        self.listing.borrow_mut().take();
        self.source.borrow_mut().take();
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
        listing: RefCell::default(),
        source: RefCell::default(),
        allowed: Cell::new(DROPEFFECT_NONE),
        helper: unsafe { CoCreateInstance(&CLSID_DragDropHelper, None, CLSCTX_INPROC_SERVER) }.ok(),
    }
    .into();
    // Windows cannot swap targets in one call. Keep GPUI's target unless it is
    // really gone, and retry once if the window is left without any.
    match unsafe { RevokeDragDrop(window) } {
        Err(error) if error.code() != DRAGDROP_E_NOTREGISTERED => return Err(error.to_string()),
        _ => {}
    }
    unsafe { RegisterDragDrop(window, &target) }
        .or_else(|_| unsafe { RegisterDragDrop(window, &target) })
        .map_err(|error| format!("{error}; this window accepts no file drop"))
    // GPUI revokes the replacement target when destroying its HWND.
}
