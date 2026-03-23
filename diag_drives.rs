use windows::Win32::Storage::FileSystem::GetDiskFreeSpaceExW;
use windows::core::PCWSTR;

fn get_estimate(drive: &str) -> (u64, u64) {
    let wide_path: Vec<u16> = drive.encode_utf16().chain(std::iter::once(0)).collect();
    let mut total_bytes = 0u64;
    unsafe {
        let _ = GetDiskFreeSpaceExW(
            PCWSTR(wide_path.as_ptr()),
            None,
            Some(&mut total_bytes),
            None
        );
    }
    let capacity = (total_bytes as f64 * 0.05) as u64;
    (total_bytes, capacity)
}

fn format_size(bytes: u64) -> String {
    if bytes > 1024 * 1024 * 1024 {
        format!("{:.2} GB", bytes as f64 / (1024.0 * 1024.0 * 1024.0))
    } else {
        format!("{:.2} MB", bytes as f64 / (1024.0 * 1024.0))
    }
}

fn main() {
    for drive in &["C:\\", "D:\\", "E:\\"] {
        let (total, cap) = get_estimate(drive);
        println!("Drive {}: Total={}, RecycleBinCap (5%)={}", drive, format_size(total), format_size(cap));
    }
}
