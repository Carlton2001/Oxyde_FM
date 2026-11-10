//! Stands for Oxyde's launcher in the tests: keeps the request it reads, appended to
//! the file `OXYDE_FAKE_REQUESTS` names, and answers with the next of the answers
//! `OXYDE_FAKE_ANSWERS` holds, separated by a line of `--`. An answer of `unavailable`
//! ends it as the launcher ends when Oxyde does not answer.
#![windows_subsystem = "windows"]
use std::io::{Read, Write};

fn main() {
    let mut request = String::new();
    let _ = std::io::stdin().read_to_string(&mut request);
    let asked = std::env::var_os("OXYDE_FAKE_REQUESTS").map(std::path::PathBuf::from).expect("requests");
    let before = std::fs::read_to_string(&asked).unwrap_or_default();
    let turn = before.matches("\n--\n").count();
    std::fs::write(&asked, format!("{before}{request}\n--\n")).expect("request kept");
    let answers = std::env::var("OXYDE_FAKE_ANSWERS").unwrap_or_default();
    let answer = answers.split("\n--\n").nth(turn).unwrap_or("unavailable");
    if answer.trim() == "unavailable" {
        std::process::exit(2);
    }
    let mut output = std::io::stdout();
    let _ = output.write_all(answer.as_bytes());
    let _ = output.flush();
}
