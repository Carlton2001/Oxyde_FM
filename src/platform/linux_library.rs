//! System libraries loaded at first use: without one, only its feature is missing.
use std::ffi::{c_char, c_int, c_void};

/// Any GLib, GObject or cairo instance pointer.
pub(super) type Object = *mut c_void;

#[repr(C)]
pub(super) struct GError {
    pub domain: u32,
    pub code: c_int,
    pub message: *mut c_char,
}

/// A struct of entry points named after their C symbols, resolved together by `load`.
macro_rules! library {
    ($name:ident from $soname:literal { $($function:ident: fn($($argument:ty),*) $(-> $output:ty)?;)* }) => {
        #[allow(non_snake_case)]
        pub(super) struct $name {
            // Kept in a static: GLib types and modules cannot be unloaded.
            _library: libloading::Library,
            $(pub $function: unsafe extern "C" fn($($argument),*) $(-> $output)?,)*
        }

        impl $name {
            fn load() -> Result<Self, libloading::Error> {
                // SAFETY: these libraries run no constructor beyond GLib's type registration.
                let library = unsafe { libloading::Library::new($soname)? };
                $(
                    // SAFETY: each signature is the one of the library's headers. A symbol
                    // of a dependency is found too, as dlsym searches the whole tree.
                    let $function = unsafe {
                        *library.get::<unsafe extern "C" fn($($argument),*) $(-> $output)?>(
                            concat!(stringify!($function), "\0").as_bytes(),
                        )?
                    };
                )*
                Ok(Self { _library: library, $($function,)* })
            }
        }
    };
}
pub(super) use library;
