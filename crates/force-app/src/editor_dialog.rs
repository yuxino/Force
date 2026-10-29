//! Shared chrome for creation editors; each caller keeps its own draft lifecycle.
use gpui::{prelude::*, *};
use gpui_component::button::Button;

use crate::{
    label::single_line,
    view::{CONTROL_LINE, DESTRUCTIVE, LINE, PAPER},
};

const WIDTH: f32 = 560.;
const PADDING: f32 = 20.;
const HEADER_HEIGHT: f32 = 68.;

pub(crate) fn width(window: &Window) -> f32 {
    WIDTH.min(f32::from(window.viewport_size().width) - 48.)
}

pub(crate) fn content_width(window: &Window) -> f32 {
    width(window) - PADDING * 2. - 2.
}

pub(crate) fn panel(id: &'static str, window: &Window) -> Stateful<Div> {
    let height = (f32::from(window.viewport_size().height) - HEADER_HEIGHT - 48.).min(600.);
    div()
        .id(id)
        .debug_selector(move || id.into())
        .w(px(width(window)))
        .h(px(height))
        .p(px(PADDING))
        .rounded(px(8.))
        .border_1()
        .border_color(rgb(CONTROL_LINE))
        .bg(rgb(PAPER))
        .occlude()
        .flex()
        .flex_col()
        .gap(px(12.))
}

pub(crate) fn overlay(id: &'static str, window: &Window, content: impl IntoElement) -> AnyElement {
    let size = window.viewport_size();
    // Do not defer the parent: pickers and inputs defer their own menus.
    anchored()
        .position(point(px(0.), px(HEADER_HEIGHT)))
        .child(
            div()
                .id(id)
                .debug_selector(move || id.into())
                .w(size.width)
                .h(size.height - px(HEADER_HEIGHT))
                .occlude()
                .bg(rgba(0x00000016))
                .flex()
                .items_center()
                .justify_center()
                .child(content),
        )
        .into_any_element()
}

pub(crate) fn heading(title: impl Into<SharedString>, close: Button) -> Div {
    div()
        .debug_selector(|| "editor-dialog-heading".into())
        .h(px(28.))
        .flex_shrink_0()
        .flex()
        .items_center()
        .gap(px(8.))
        .child(
            single_line(title, 18.)
                .flex_1()
                .font_weight(FontWeight::MEDIUM),
        )
        .child(close)
}

pub(crate) fn fields(id: &'static str, scroll: &ScrollHandle) -> Stateful<Div> {
    div()
        .id(id)
        .debug_selector(move || id.into())
        .flex_1()
        .min_h_0()
        .overflow_y_scroll()
        .track_scroll(scroll)
        .flex()
        .flex_col()
        .gap(px(8.))
}

pub(crate) fn validation(id: &'static str, error: Option<&str>) -> Div {
    div()
        .debug_selector(move || id.into())
        .min_h(px(22.))
        .flex_shrink_0()
        .text_size(px(13.))
        .line_height(px(18.))
        .text_color(rgb(DESTRUCTIVE))
        .whitespace_normal()
        .when_some(error, |slot, error| slot.child(error.to_owned()))
}

pub(crate) fn footer(id: &'static str) -> Div {
    div()
        .debug_selector(move || id.into())
        .flex_shrink_0()
        .pt(px(12.))
        .border_t_1()
        .border_color(rgb(LINE))
        .flex()
        .items_center()
        .justify_end()
        .gap(px(8.))
}
