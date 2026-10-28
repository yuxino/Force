use chrono::{Datelike, Local, NaiveDate, TimeZone};
use gpui::{prelude::*, *};

use crate::view::MUTED;

pub(crate) const BACKGROUND_QUOTE_HEIGHT: f32 = 44.;
pub(crate) const MIN_BACKGROUND_QUOTE_HEIGHT: f32 = 700.;

const QUOTES: [&str; 2] = [
    "鸡蛋从外面打破是食物，从里面打破才是新生。",
    "我的未来居然全部取决于我自己吗？会不会有点太不靠谱了？",
];

fn quote_for_date(date: NaiveDate) -> &'static str {
    QUOTES[date.num_days_from_ce().rem_euclid(QUOTES.len() as i32) as usize]
}

pub(crate) fn background_quote(now: i64) -> Div {
    let date = Local
        .timestamp_opt(now, 0)
        .single()
        .map_or_else(|| Local::now().date_naive(), |time| time.date_naive());
    div()
        .debug_selector(|| "background-quote".into())
        .w_full()
        .max_w(px(976.))
        .mx_auto()
        .px(px(40.))
        .pt(px(8.))
        .pb(px(16.))
        .flex_shrink_0()
        .child(
            div()
                .debug_selector(|| "background-quote-text".into())
                .w_full()
                .text_size(px(13.))
                .line_height(px(20.))
                .text_color(rgb(MUTED))
                .whitespace_normal()
                .child(quote_for_date(date)),
        )
}

#[cfg(test)]
mod tests {
    use super::quote_for_date;
    use chrono::NaiveDate;

    #[test]
    fn daily_quotes_keep_alternating_across_the_new_year() {
        let last_day = NaiveDate::from_ymd_opt(2026, 12, 31).unwrap();
        let next_day = last_day.succ_opt().unwrap();
        assert_ne!(quote_for_date(last_day), quote_for_date(next_day));
        assert_eq!(
            quote_for_date(last_day),
            quote_for_date(next_day.succ_opt().unwrap())
        );
    }
}

#[cfg(all(test, feature = "ui-test-support"))]
mod ui_tests {
    use super::BACKGROUND_QUOTE_HEIGHT;
    use crate::{
        headless_ui_tests::ManagerHarness,
        model::{Page, day_bounds},
    };
    use chrono::NaiveDate;
    use gpui::{TestAppContext, px};

    #[gpui::test]
    fn background_quotes_preserve_actions_at_minimum_and_boundary_sizes(cx: &mut TestAppContext) {
        let first_date = NaiveDate::from_ymd_opt(2026, 10, 28).unwrap();
        for (width, height, quote_visible) in [
            (1040., 780., true),
            (780., 620., false),
            (1040., 699., false),
            (1040., 700., true),
            (780., 700., true),
        ] {
            for page in [Page::Agenda, Page::Focus, Page::Records, Page::Settings] {
                let mut manager = ManagerHarness::new_on_page(cx, width, height, page);
                for date in [first_date, first_date.succ_opt().unwrap()] {
                    manager.view.update(&mut manager.cx, |view, cx| {
                        view.now = day_bounds(date).0 + 12 * 3_600;
                        cx.notify();
                    });
                    manager.cx.update(|window, _| window.refresh());
                    manager.cx.run_until_parked();

                    let content_bottom = if quote_visible {
                        let footer = manager.bounds("background-quote");
                        let text = manager.bounds("background-quote-text");
                        let header = manager.bounds("manager-header");
                        let content = manager.bounds("manager-content");
                        assert!(footer.left() >= px(0.) && footer.right() <= px(width));
                        assert!(footer.top() >= header.bottom());
                        assert!((footer.bottom() - px(height)).abs() <= px(1.));
                        assert!((footer.size.height - px(BACKGROUND_QUOTE_HEIGHT)).abs() <= px(1.));
                        assert!(text.left() >= footer.left() + px(40.) - px(1.));
                        assert!(text.right() <= footer.right() - px(40.) + px(1.));
                        assert!(text.top() >= footer.top() && text.bottom() <= footer.bottom());
                        assert!(text.size.height >= px(20.));
                        assert!((footer.left() - content.left()).abs() <= px(1.));
                        footer.top()
                    } else {
                        assert!(manager.cx.debug_bounds("background-quote").is_none());
                        assert!(manager.cx.debug_bounds("background-quote-text").is_none());
                        px(height)
                    };

                    if matches!(page, Page::Agenda | Page::Focus) {
                        assert!(
                            manager.bounds("manager-content").bottom() <= content_bottom + px(1.)
                        );
                    } else {
                        assert!(manager.bounds("page-heading").bottom() <= content_bottom);
                    }
                    if page == Page::Focus {
                        let operation = manager.bounds("focus-operation");
                        let start = manager.bounds("start-focus");
                        assert!(start.bottom() <= operation.bottom() + px(1.));
                        assert!(start.bottom() <= content_bottom);
                    }
                }
            }
        }
    }
}
