use super::{DriveAbsences, GRACE};
use std::time::{Duration, Instant};

fn shown(absences: &mut DriveAbsences, missing: &[&str], now: Instant) -> Vec<String> {
    let missing = missing.iter().map(|path| path.to_string()).collect();
    absences.still_shown(missing, |path: &String| path.as_str(), now)
}

#[test]
fn a_burst_of_surveys_does_not_remove_a_drive_missing_for_a_moment() {
    let start = Instant::now();
    let mut absences = DriveAbsences::default();
    for offset in [0, 150, 500, 1250] {
        assert_eq!(shown(&mut absences, &["K:\\"], start + Duration::from_millis(offset)), ["K:\\"]);
    }
    assert!(shown(&mut absences, &["K:\\"], start + Duration::from_millis(2750)).is_empty());
}

#[test]
fn a_drive_back_in_a_survey_starts_again_from_zero() {
    let start = Instant::now();
    let mut absences = DriveAbsences::default();
    shown(&mut absences, &["K:\\"], start);
    shown(&mut absences, &[], start + Duration::from_millis(500));
    let later = start + GRACE + Duration::from_millis(100);
    assert_eq!(shown(&mut absences, &["K:\\"], later), ["K:\\"]);
    assert!(shown(&mut absences, &["K:\\"], later + GRACE).is_empty());
}
