use super::{Context, OxydeApp};
use crate::ui::date_groups::CalendarReference;
use std::time::Duration;

impl OxydeApp {
    pub(super) fn start_calendar_task(&mut self, cx: &mut Context<Self>) {
        self.calendar_task = Some(cx.spawn(async move |this, cx| {
            let mut previous = None;
            loop {
                let Ok(reference) = tokio::task::spawn_blocking(CalendarReference::current).await
                else {
                    break;
                };
                let Ok(requests) = this.update(cx, |this, _| {
                    this.state.layout.calendar_filter_requests(&reference)
                }) else {
                    break;
                };
                let Ok((results, current)) = tokio::task::spawn_blocking(move || {
                    let results = requests
                        .into_iter()
                        .map(|request| request.calculate())
                        .collect();
                    (results, CalendarReference::current())
                })
                .await
                else {
                    break;
                };
                if current == reference {
                    let clock_changed = previous.as_ref() != Some(&reference);
                    if this
                        .update(cx, |this, cx| {
                            let filtered = this.state.layout.apply_calendar_filters(results);
                            if filtered || (clock_changed && this.state.layout.has_date_groups()) {
                                cx.notify();
                            }
                        })
                        .is_err()
                    {
                        break;
                    }
                    previous = Some(reference);
                }
                cx.background_executor().timer(Duration::from_secs(5)).await;
            }
        }));
    }
}
