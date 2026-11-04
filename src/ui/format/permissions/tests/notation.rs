use super::{octal, paired, symbolic};

#[test]
fn octal_adds_a_digit_only_for_special_bits() {
    assert_eq!(octal(0o644), "644");
    assert_eq!(octal(0o700), "700");
    assert_eq!(octal(0o007), "007");
    assert_eq!(octal(0o1777), "1777");
    assert_eq!(octal(0o4755), "4755");
    assert_eq!(octal(0o100644), "644");
}

#[test]
fn paired_backs_octal_with_access_bits() {
    assert_eq!(paired(0o755), "755  rwxr-xr-x");
    assert_eq!(paired(0o1777), "1777  rwxrwxrwt");
}

#[test]
fn symbolic_matches_ls() {
    assert_eq!(symbolic(0o755, true, false), "drwxr-xr-x");
    assert_eq!(symbolic(0o644, false, false), "-rw-r--r--");
    assert_eq!(symbolic(0o1777, true, false), "drwxrwxrwt");
    assert_eq!(symbolic(0o4755, false, false), "-rwsr-xr-x");
    assert_eq!(symbolic(0o2644, false, false), "-rw-r-Sr--");
    assert_eq!(symbolic(0o777, false, true), "lrwxrwxrwx");
}
