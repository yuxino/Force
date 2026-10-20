use gpui::{ElementId, ParentElement as _, SharedString, Styled as _, rgb};
use gpui_component::{Icon, button::Button};

use crate::view::{ControlState as _, control_icon, control_label, primary_button};

pub(crate) const START_FOREGROUND: u32 = 0xffffff;
pub(crate) const DISABLED_START_FOREGROUND: u32 = 0x626262;
pub(crate) const DISABLED_START_BACKGROUND: u32 = 0xe9e9e9;

/// Keep the label and icon consistent with the disabled button surface.
/// Child colors are explicit because the component's hover foreground is red.
pub(crate) fn start_button(
    id: impl Into<ElementId>,
    label: impl Into<SharedString>,
    icon: Icon,
    disabled: bool,
) -> Button {
    let foreground = if disabled {
        DISABLED_START_FOREGROUND
    } else {
        START_FOREGROUND
    };
    let button = primary_button(id)
        .unavailable(disabled)
        .icon(control_icon(icon, foreground))
        .child(control_label(label, foreground));
    if disabled {
        // ButtonCustomVariant also fades disabled backgrounds in 0.5.1.
        // An explicit style override is applied after that component policy.
        button.bg(rgb(DISABLED_START_BACKGROUND))
    } else {
        button
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn relative_luminance(color: u32) -> f64 {
        let linear = |component: u32| {
            let value = f64::from(component & 0xff) / 255.;
            if value <= 0.04045 {
                value / 12.92
            } else {
                ((value + 0.055) / 1.055).powf(2.4)
            }
        };
        0.2126 * linear(color >> 16) + 0.7152 * linear(color >> 8) + 0.0722 * linear(color)
    }

    fn contrast_ratio(foreground: u32, background: u32) -> f64 {
        let foreground = relative_luminance(foreground);
        let background = relative_luminance(background);
        (foreground.max(background) + 0.05) / (foreground.min(background) + 0.05)
    }

    #[test]
    fn disabled_start_label_and_icon_remain_readable_on_their_actual_surface() {
        let mut control = start_button("contrast-check", "Start", Icon::default(), true);
        let style = control.style();
        assert_eq!(
            style.opacity.unwrap_or(1.),
            1.,
            "do not fade the whole control"
        );
        assert!(style.background == Some(rgb(DISABLED_START_BACKGROUND).into()));
        assert!(
            contrast_ratio(DISABLED_START_FOREGROUND, DISABLED_START_BACKGROUND) >= 4.5,
            "disabled start content must remain readable without group opacity"
        );
        // Guard the original failure: a white child label on this light surface.
        assert!(contrast_ratio(START_FOREGROUND, DISABLED_START_BACKGROUND) < 4.5);
    }
}
