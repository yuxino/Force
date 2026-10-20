use gpui::{
    Div, ParentElement as _, Pixels, ShapedLine, SharedString, Styled as _, TextRun, TextStyle,
    WindowTextSystem, canvas, div, px, relative,
};

/// Shape against final bounds so an intrinsic-width measurement cannot shorten
/// the text runs reused by GPUI 0.2.2's built-in ellipsis layout.
pub(crate) fn single_line(value: impl Into<SharedString>, font_size: f32) -> Div {
    let value = value.into();
    let value: SharedString = if value.contains(['\r', '\n']) {
        value.replace(['\r', '\n'], " ").into()
    } else {
        value
    };
    let line_height = px(font_size * 1.5);
    div()
        .w_full()
        .min_w_0()
        .h(line_height)
        .text_size(px(font_size))
        .line_height(relative(1.5))
        .overflow_hidden()
        .child(
            canvas(
                move |bounds, window, _| {
                    let style = window.text_style();
                    let font_size = style.font_size.to_pixels(window.rem_size());
                    shape_single_line(
                        value,
                        bounds.size.width,
                        &style,
                        font_size,
                        window.text_system(),
                    )
                },
                move |bounds, line, window, cx| {
                    let _ = line.paint(bounds.origin, line_height, window, cx);
                },
            )
            .size_full(),
        )
}

fn shape_single_line(
    value: SharedString,
    width: Pixels,
    style: &TextStyle,
    font_size: Pixels,
    text_system: &WindowTextSystem,
) -> ShapedLine {
    let (text, runs) = fitted_text(value, width, style, font_size, text_system);
    text_system.shape_line(text, font_size, &runs, None)
}

fn fitted_text(
    value: SharedString,
    width: Pixels,
    style: &TextStyle,
    font_size: Pixels,
    text_system: &WindowTextSystem,
) -> (SharedString, Vec<TextRun>) {
    // truncate_line mutates runs. Each pass must start from the original text's
    // full UTF-8 length, even after an earlier pass reduced it to just "…".
    let mut runs = vec![style.to_run(value.len())];
    let text = text_system
        .line_wrapper(style.font(), font_size)
        .truncate_line(value, width.max(px(0.)), "…", &mut runs);
    (text, runs)
}

#[cfg(all(test, feature = "ui-test-support"))]
mod tests {
    use super::*;
    use gpui::TestAppContext;

    #[gpui::test]
    fn single_line_restores_full_text_after_a_zero_width_pass(cx: &mut TestAppContext) {
        cx.add_empty_window().update(|window, _| {
            let style = TextStyle::default();
            for value in ["Review focus workspace", "计时中的完整任务名称"] {
                let value: SharedString = value.into();
                // NoopTextSystem ignores font runs, so inspect the run coverage
                // directly as well as the shaped result. Bounds/len alone miss
                // the native bug that left only the first three UTF-8 bytes.
                for width in [0., 80., 600.] {
                    let (text, runs) = fitted_text(
                        value.clone(),
                        px(width),
                        &style,
                        px(14.),
                        window.text_system(),
                    );
                    assert_eq!(runs.iter().map(|run| run.len).sum::<usize>(), text.len());
                    if width == 600. {
                        assert_eq!(text, value);
                    }
                }
                let narrow =
                    shape_single_line(value.clone(), px(0.), &style, px(14.), window.text_system());
                assert_eq!(narrow.text.as_ref(), "…");
                let wide = shape_single_line(
                    value.clone(),
                    px(600.),
                    &style,
                    px(14.),
                    window.text_system(),
                );
                assert_eq!(wide.text, value);
                assert_eq!(wide.len(), value.len());
                assert!(wide.width > narrow.width);
            }
            window.remove_window();
        });
    }

    #[gpui::test]
    fn single_line_truncates_long_chinese_with_an_ellipsis(cx: &mut TestAppContext) {
        cx.add_empty_window().update(|window, _| {
            let value: SharedString =
                "这是需要完整保存但只在任务列表中显示一行的长中文任务名称".into();
            let width = px(80.);
            let line = shape_single_line(
                value.clone(),
                width,
                &TextStyle::default(),
                px(14.),
                window.text_system(),
            );
            assert!(line.text.ends_with('…'));
            assert!(line.text.len() < value.len());
            assert!(value.starts_with(line.text.trim_end_matches('…')));
            assert_eq!(line.len(), line.text.len());
            assert!(line.width <= width + px(1.));
            window.remove_window();
        });
    }
}
