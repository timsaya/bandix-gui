use super::BandixApp;
use crate::app::traffic_stats::date_range_ms;
use crate::i18n::tr;
use chrono::{Datelike, Duration, Local, Months, NaiveDate};
use gpui_kit::component::{
    Sizable,
    select::{Select, SelectItem},
};
use gpui_kit::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(in crate::app) enum DevicePeriod {
    Today,
    Week,
    Month,
    Year,
    #[default]
    #[serde(other)]
    All,
}

impl DevicePeriod {
    pub const ALL: [Self; 5] = [Self::All, Self::Today, Self::Week, Self::Month, Self::Year];
    fn label(self) -> &'static str {
        match self {
            Self::All => tr("全部"),
            Self::Today => tr("今天"),
            Self::Week => tr("本周"),
            Self::Month => tr("本月"),
            Self::Year => tr("今年"),
        }
    }
    fn dates(self, today: NaiveDate) -> Option<(NaiveDate, NaiveDate)> {
        match self {
            Self::All => None,
            Self::Today => Some((today, today)),
            Self::Week => {
                let start = today - Duration::days(today.weekday().num_days_from_monday() as i64);
                Some((start, start + Duration::days(6)))
            }
            Self::Month => {
                let start = today.with_day(1)?;
                Some((start, start.checked_add_months(Months::new(1))?.pred_opt()?))
            }
            Self::Year => Some((
                NaiveDate::from_ymd_opt(today.year(), 1, 1)?,
                NaiveDate::from_ymd_opt(today.year(), 12, 31)?,
            )),
        }
    }
    pub fn range_ms(self) -> Option<(u64, u64)> {
        let (start, end) = self.dates(Local::now().date_naive())?;
        date_range_ms(start, end)
    }
}

impl SelectItem for DevicePeriod {
    type Value = Self;
    fn title(&self) -> SharedString {
        self.label().into()
    }
    fn value(&self) -> &Self {
        self
    }
}

pub(super) fn render(app: &BandixApp, expanded: bool) -> AnyElement {
    if !expanded && app.device_list.view.expanded {
        return div().h(px(28.0)).into_any_element();
    }
    div()
        .w(px(100.0))
        .h(px(28.0))
        .child(
            Select::new(&app.device_period)
                .id("device-period")
                .small()
                .text_size(px(11.0))
                .title_prefix(tr("周期 "))
                .accessibility_label(tr("设备流量统计周期"))
                .w_full()
                .h(px(28.0))
                .menu_width(px(100.0)),
        )
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[::core::prelude::v1::test]
    fn matches_luci_calendar_periods_including_sunday_leap_february_and_year_end() {
        let sunday = NaiveDate::from_ymd_opt(2024, 2, 25).unwrap();
        assert_eq!(DevicePeriod::All.dates(sunday), None);
        assert_eq!(DevicePeriod::Today.dates(sunday), Some((sunday, sunday)));
        assert_eq!(
            DevicePeriod::Week.dates(sunday),
            Some((NaiveDate::from_ymd_opt(2024, 2, 19).unwrap(), sunday))
        );
        assert_eq!(
            DevicePeriod::Month.dates(sunday),
            Some((
                NaiveDate::from_ymd_opt(2024, 2, 1).unwrap(),
                NaiveDate::from_ymd_opt(2024, 2, 29).unwrap()
            ))
        );
        assert_eq!(
            DevicePeriod::Year.dates(sunday),
            Some((
                NaiveDate::from_ymd_opt(2024, 1, 1).unwrap(),
                NaiveDate::from_ymd_opt(2024, 12, 31).unwrap()
            ))
        );
        let december = NaiveDate::from_ymd_opt(2026, 12, 31).unwrap();
        assert_eq!(DevicePeriod::Month.dates(december).unwrap().1, december);
    }
}
