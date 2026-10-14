//! Keep the hosting STA alive while Shell property handlers retain its thread reference.
use std::{
    marker::PhantomData,
    rc::Rc,
    sync::atomic::{AtomicI32, Ordering},
};
use windows::core::IUnknown;
use windows::Win32::UI::Shell::{SHCreateThreadRef, SHSetThreadRef};
use windows::Win32::UI::WindowsAndMessaging::*;

pub(super) struct ShellThread {
    counter: Box<AtomicI32>,
    reference: Option<IUnknown>,
    baseline: i32,
    _thread: PhantomData<Rc<()>>,
}

impl ShellThread {
    pub(super) fn new() -> Result<Self, String> {
        let counter = Box::new(AtomicI32::new(0));
        let mut reference = None;
        unsafe {
            SHCreateThreadRef(counter.as_ptr(), &mut reference).map_err(|e| e.to_string())?;
        }
        let reference = reference.ok_or("Missing Shell thread reference")?;
        unsafe {
            SHSetThreadRef(&reference).map_err(|e| e.to_string())?;
        }
        let baseline = counter.load(Ordering::SeqCst);
        Ok(Self {
            counter,
            reference: Some(reference),
            baseline,
            _thread: PhantomData,
        })
    }

    pub(super) fn wait(&self) {
        unsafe {
            while self.counter.load(Ordering::SeqCst) > self.baseline {
                let mut message = MSG::default();
                while PeekMessageW(&mut message, None, 0, 0, PM_REMOVE).as_bool() {
                    if message.message != WM_QUIT {
                        let _ = TranslateMessage(&message);
                        DispatchMessageW(&message);
                    }
                }
                let _ = MsgWaitForMultipleObjectsEx(None, 50, QS_ALLINPUT, MWMO_INPUTAVAILABLE);
            }
        }
    }
}

impl Drop for ShellThread {
    fn drop(&mut self) {
        unsafe {
            let _ = SHSetThreadRef(None);
        }
        // Release the COM object before the storage used by its reference count.
        // The field order alone would otherwise drop the counter first.
        drop(self.reference.take());
    }
}
