//! Redirect queued keyboard input before GPUI translates accelerators.
//! The real form retains Windows activation/focus; each menu retains GPUI focus.
use super::*;
use windows::Win32::System::Threading::GetCurrentThreadId;

#[derive(Default)]
struct Routing {
    hook: Option<HHOOK>,
    // Opening order selects the latest visible interactive submenu per form.
    menus: Vec<(isize, isize)>,
}
thread_local! {
    static ROUTING: RefCell<Routing> = RefCell::new(Routing::default());
}

pub(super) fn register(window: HWND, owner: HWND, interactive: bool) -> Result<(), String> {
    if !interactive {
        return Ok(());
    }
    ROUTING.with(|routing| {
        let mut routing = routing.borrow_mut();
        if routing.hook.is_none() {
            routing.hook = Some(
                unsafe {
                    SetWindowsHookExW(WH_GETMESSAGE, Some(route), None, GetCurrentThreadId())
                }
                .map_err(|error| error.to_string())?,
            );
        }
        routing.menus.retain(|(menu, _)| *menu != window.0 as isize);
        routing.menus.push((window.0 as isize, owner.0 as isize));
        Ok(())
    })
}

pub(super) fn remove(window: HWND) {
    let hook = ROUTING.with(|routing| {
        let mut routing = routing.borrow_mut();
        routing
            .menus
            .retain(|(menu, owner)| *menu != window.0 as isize && *owner != window.0 as isize);
        if routing.menus.is_empty() {
            routing.hook.take()
        } else {
            None
        }
    });
    if let Some(hook) = hook {
        let _ = unsafe { UnhookWindowsHookEx(hook) };
    }
}

fn target(owner: HWND) -> Option<HWND> {
    let menus = ROUTING.with(|routing| routing.borrow().menus.clone());
    menus.into_iter().rev().find_map(|(menu, source)| {
        let menu = HWND(menu as *mut _);
        (source == owner.0 as isize && unsafe { IsWindowVisible(menu) }.as_bool()).then_some(menu)
    })
}

unsafe extern "system" fn route(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code == HC_ACTION as i32 && wparam.0 == PM_REMOVE.0 as usize {
        let message = unsafe { &mut *(lparam.0 as *mut MSG) };
        if matches!(
            message.message,
            WM_KEYDOWN
                | WM_KEYUP
                | WM_SYSKEYDOWN
                | WM_SYSKEYUP
                | WM_CHAR
                | WM_SYSCHAR
                | WM_DEADCHAR
                | WM_SYSDEADCHAR
                | WM_UNICHAR
                | WM_IME_STARTCOMPOSITION
                | WM_IME_COMPOSITION
                | WM_IME_ENDCOMPOSITION
                | WM_IME_CHAR
        ) {
            if let Some(menu) = target(message.hwnd) {
                message.hwnd = menu;
            }
        }
    }
    unsafe { CallNextHookEx(None, code, wparam, lparam) }
}
