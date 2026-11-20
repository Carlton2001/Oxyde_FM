use super::{AdvancedSearchWindow, OxydeApp};
use crate::ui::format::size_in_bytes;
use gpui::*;
use oxyde_lib::commands::search::AdvancedSearchOptions;

pub fn parse_advanced_path(path: &str) -> Option<(String, AdvancedSearchOptions)> {
    let (kind, root, options): (String, String, AdvancedSearchOptions) =
        serde_json::from_str(path.strip_prefix("search://")?).ok()?;
    (kind == "advanced").then_some((root, options))
}

impl AdvancedSearchWindow {
    pub(super) fn submit_advanced_search(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        let Some(d) = &self.advanced_search else {
            return;
        };
        let parse = || -> Result<AdvancedSearchOptions, ()> {
            let size = |index: usize| -> Result<Option<u64>, ()> {
                size_in_bytes(&d.values[index], d.unit)
            };
            let date = |index: usize| -> Result<Option<u64>, ()> {
                let v = d.values[index].trim();
                if v.is_empty() {
                    return Ok(None);
                }
                let day = chrono::NaiveDate::parse_from_str(v, "%Y-%m-%d").map_err(|_| ())?;
                let time = day
                    .and_hms_opt(
                        if index == 6 { 23 } else { 0 },
                        if index == 6 { 59 } else { 0 },
                        if index == 6 { 59 } else { 0 },
                    )
                    .ok_or(())?;
                use chrono::TimeZone;
                // Midnight skipped by a clock change: the day starts an hour later.
                let timestamp = chrono::Local
                    .from_local_datetime(&time)
                    .earliest()
                    .or_else(|| chrono::Local.from_local_datetime(&(time + chrono::Duration::hours(1))).earliest())
                    .ok_or(())?
                    .timestamp_millis();
                // Dates before 1970 bound nothing finer than the earliest representable time.
                Ok(Some(u64::try_from(timestamp + if index == 6 { 999 } else { 0 }).unwrap_or(0)))
            };
            let o = AdvancedSearchOptions {
                query: d.values[0].trim().into(),
                content: d.values[2].trim().into(),
                regex: d.flags[0],
                content_regex: d.flags[1],
                recursive: d.flags[2],
                archives: d.flags[3],
                case_sensitive: d.flags[4],
                ignore_accents: d.flags[5],
                min_size: size(3)?,
                max_size: size(4)?,
                min_date: date(5)?,
                max_date: date(6)?,
            };
            o.validate().map_err(|_| ())?;
            if d.values[1].trim().is_empty() {
                return Err(());
            }
            Ok(o)
        };
        let Ok(options) = parse() else {
            self.advanced_search.as_mut().unwrap().error = true;
            cx.notify();
            return;
        };
        let pane = d.pane.clone();
        let root = d.values[1].trim().to_string();
        let path = format!(
            "search://{}",
            serde_json::to_string(&("advanced", root, options)).expect("search options")
        );
        self.advanced_search.as_mut().unwrap().error = false;
        cx.notify();
        let owner = self.owner.clone();
        cx.defer(move |cx| {
            let _ = owner.update(cx, |owner, cx| {
                // The pane closed since the dialog opened: search in the active one.
                let pane = if owner.state.find_pane(&pane).is_some() {
                    pane
                } else {
                    owner.state.active_pane_id.clone()
                };
                // The same search again runs again, as a refresh would.
                if owner.state.find_pane(&pane).is_some_and(|found| found.active_tab().path == path) {
                    owner.load_path(&pane, path, cx);
                } else {
                    owner.navigate_to(&pane, path, cx);
                }
                for handle in cx.windows() {
                    if let Some(handle) = handle.downcast::<OxydeApp>() {
                        cx.defer(move |cx| {
                            let _ = handle.update(cx, |_, window, _| window.activate_window());
                        });
                    }
                }
            });
        });
    }
}

#[cfg(test)]
mod tests {
    use super::size_in_bytes;

    #[test]
    fn size_bounds_accept_decimal_units_and_reject_overflow() {
        assert_eq!(size_in_bytes("1,5", 1), Ok(Some(1536)));
        assert_eq!(size_in_bytes("1.5", 2), Ok(Some(1572864)));
        assert_eq!(size_in_bytes("", 1), Ok(None));
        assert!(size_in_bytes("-1", 1).is_err());
        assert!(size_in_bytes("18446744073709551615", 1).is_err());
    }
}
