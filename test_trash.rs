use windows::Win32::UI::Shell::{SHFileOperationW, SHFILEOPSTRUCTW, FO_DELETE, FOF_ALLOWUNDO, FOF_NOCONFIRMATION, FOF_NOERRORUI, FOF_SILENT, FOF_NO_UI};
use windows::Win32::Foundation::HWND;
use windows::core::PCWSTR;
use std::path::PathBuf;

fn main() {
    let test_dir = "C:\\Temp\\OxydeTestTrash";
    std::fs::create_dir_all(test_dir).unwrap();
    let file_path = format!("{}\\huge_file.bin", test_dir);
    
    // Create a 100MB file to see what it does
    let mut f = std::fs::File::create(&file_path).unwrap();
    f.set_len(100 * 1024 * 1024).unwrap();
    drop(f);
    
    let mut buffer: Vec<u16> = Vec::new();
    let path_str = file_path.replace("/", "\\");
    buffer.extend(path_str.encode_utf16());
    buffer.push(0);
    buffer.push(0);

    let mut sh_op = SHFILEOPSTRUCTW {
        hwnd: HWND(std::ptr::null_mut()),
        wFunc: FO_DELETE,
        pFrom: PCWSTR(buffer.as_ptr()),
        pTo: PCWSTR(std::ptr::null()),
        fFlags: (FOF_ALLOWUNDO.0 | FOF_NOCONFIRMATION.0 | FOF_NOERRORUI.0 | FOF_SILENT.0) as u16,
        fAnyOperationsAborted: Default::default(),
        hNameMappings: std::ptr::null_mut(),
        lpszProgressTitle: PCWSTR(std::ptr::null()),
    };

    let result = unsafe { SHFileOperationW(&mut sh_op) };
    println!("Result 1: 0x{:X}", result);
    println!("Aborted 1: {}", sh_op.fAnyOperationsAborted.as_bool());
    println!("File exists 1: {}", std::path::Path::new(&file_path).exists());
}
