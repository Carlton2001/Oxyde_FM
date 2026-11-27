//! Queries against the current process token.
use windows::Win32::Foundation::{CloseHandle, HANDLE, HLOCAL, LocalFree};
use windows::Win32::Security::Authorization::ConvertSidToStringSidW;
use windows::Win32::Security::{
    GetTokenInformation, TOKEN_INFORMATION_CLASS, TOKEN_QUERY, TOKEN_USER, TokenUser,
};
use windows::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};
use windows::core::PWSTR;

/// What this process's token says of `class`, in a buffer aligned for its structure.
pub(super) fn token_information(class: TOKEN_INFORMATION_CLASS) -> Option<Vec<u64>> {
    unsafe {
        let mut token = HANDLE::default();
        OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token).ok()?;
        let mut size = 0u32;
        let _ = GetTokenInformation(token, class, None, 0, &mut size);
        let mut buffer = vec![0u64; (size as usize).div_ceil(8)];
        let read = GetTokenInformation(
            token,
            class,
            Some(buffer.as_mut_ptr().cast()),
            size,
            &mut size,
        );
        let _ = CloseHandle(token);
        read.ok().map(|()| buffer)
    }
}

pub(super) fn current_user_sid() -> Option<String> {
    let user = token_information(TokenUser)?;
    unsafe {
        let mut text = PWSTR::null();
        ConvertSidToStringSidW((*user.as_ptr().cast::<TOKEN_USER>()).User.Sid, &mut text).ok()?;
        let user = text.to_string();
        let _ = LocalFree(Some(HLOCAL(text.0.cast())));
        user.ok()
    }
}
