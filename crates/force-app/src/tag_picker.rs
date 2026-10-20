use gpui::{prelude::*, *};
use gpui_component::{
    Disableable, Icon, IconName,
    button::{Button, ButtonCustomVariant, ButtonVariants},
    menu::{PopupMenu, PopupMenuItem},
    tooltip::Tooltip,
};

use crate::label::single_line;

const WIDTH: f32 = 220.;

#[derive(Clone, Eq, PartialEq)]
pub(crate) struct FocusTagOption {
    pub(crate) id: i64,
    pub(crate) name: String,
}

pub(crate) struct TagPicked(pub(crate) i64);

pub(crate) struct TagPicker {
    options: Vec<FocusTagOption>,
    selected: Option<i64>,
    enabled: bool,
    focus_handle: FocusHandle,
    return_focus: Option<FocusHandle>,
    trigger_bounds: Option<Bounds<Pixels>>,
    trigger_was_open: bool,
    menu: Option<Entity<PopupMenu>>,
    dismiss_subscription: Option<Subscription>,
    blur_subscription: Option<Subscription>,
}

impl TagPicker {
    pub(crate) fn new(cx: &mut Context<Self>) -> Self {
        Self {
            options: Vec::new(),
            selected: None,
            enabled: false,
            focus_handle: cx.focus_handle().tab_stop(false),
            return_focus: None,
            trigger_bounds: None,
            trigger_was_open: false,
            menu: None,
            dismiss_subscription: None,
            blur_subscription: None,
        }
    }

    pub(crate) fn selected_value(&self) -> Option<i64> {
        self.selected
    }

    pub(crate) fn is_enabled(&self) -> bool {
        self.enabled
    }

    pub(crate) fn synchronize(
        &mut self,
        options: Vec<FocusTagOption>,
        selected: Option<i64>,
        enabled: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !enabled || self.options != options || self.selected != selected {
            self.dismiss(window, enabled, cx);
        }
        self.options = options;
        self.selected = selected;
        self.enabled = enabled;
        cx.notify();
    }

    pub(crate) fn dismiss(
        &mut self,
        window: &mut Window,
        restore_focus: bool,
        cx: &mut Context<Self>,
    ) {
        // Dismiss/blur may arrive while PopupMenu is dispatching an action.
        // Those paths never restore focus and must not reborrow that menu.
        let menu_was_focused = restore_focus
            && self
                .menu
                .as_ref()
                .is_some_and(|menu| menu.focus_handle(cx).is_focused(window));
        self.menu = None;
        self.dismiss_subscription = None;
        self.blur_subscription = None;
        if restore_focus && menu_was_focused {
            if let Some(focus) = self.return_focus.take() {
                focus.focus(window);
            }
        } else {
            self.return_focus = None;
        }
        cx.notify();
    }

    fn open(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.enabled || self.options.is_empty() {
            return;
        }
        let return_focus = window
            .focused(cx)
            .unwrap_or_else(|| self.focus_handle.clone());
        let menu_return_focus = return_focus.clone();
        let options = self.options.clone();
        let selected = self.selected;
        let picker = cx.entity().downgrade();
        let max_height = (f32::from(window.bounds().size.height) - 16.).clamp(32., 240.);
        let menu = PopupMenu::build(window, cx, move |mut menu, _, _| {
            menu = menu
                .action_context(menu_return_focus)
                .scrollable(true)
                .min_w(px(WIDTH - 2.))
                .max_w(px(WIDTH - 2.))
                .max_h(px(max_height));
            for option in options {
                let id = option.id;
                let name = option.name;
                let picker = picker.clone();
                menu = menu.item(
                    PopupMenuItem::element(move |_, _| {
                        let help = name.clone();
                        single_line(name.clone(), 14.)
                            .id(("focus-tag-option", id as u64))
                            .debug_selector(move || format!("focus-tag-option-{id}"))
                            .w(px(174.))
                            .text_color(rgb(0x202020))
                            .tooltip(move |window, cx| Tooltip::new(help.clone()).build(window, cx))
                    })
                    .checked(Some(id) == selected)
                    .on_click(move |_, _, cx| {
                        let _ = picker.update(cx, |this, cx| {
                            if this.options.iter().any(|option| option.id == id) {
                                this.selected = Some(id);
                                cx.emit(TagPicked(id));
                                cx.notify();
                            }
                        });
                    }),
                );
            }
            menu
        });
        let menu_focus = menu.focus_handle(cx);
        self.return_focus = Some(return_focus);
        self.dismiss_subscription = Some(cx.subscribe_in(
            &menu,
            window,
            |this, _, _: &DismissEvent, window, cx| this.dismiss(window, false, cx),
        ));
        self.blur_subscription = Some(cx.on_blur(&menu_focus, window, |this, window, cx| {
            // Navigation and shortcuts may have deliberately focused another
            // control. Close the menu without moving that new focus back.
            this.dismiss(window, false, cx);
        }));
        self.menu = Some(menu);
        menu_focus.focus(window);
        cx.notify();
    }

    fn toggle(&mut self, event: &ClickEvent, window: &mut Window, cx: &mut Context<Self>) {
        // PopupMenu closes on outside mouse-down. Remember whether this same
        // pointer gesture began on the open trigger, so mouse-up does not reopen it.
        let was_open = std::mem::take(&mut self.trigger_was_open);
        if !event.is_keyboard() && was_open {
            self.dismiss(window, false, cx);
        } else if self.menu.is_some() {
            self.dismiss(window, true, cx);
        } else {
            self.open(window, cx);
        }
    }
}

impl EventEmitter<TagPicked> for TagPicker {}

impl Render for TagPicker {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let title = self
            .options
            .iter()
            .find(|option| Some(option.id) == self.selected)
            .map_or("选择标签", |option| option.name.as_str())
            .to_owned();
        let help = title.clone();
        let view = cx.entity();
        div()
            .id("focus-tag-picker")
            .debug_selector(|| "focus-tag-picker".into())
            .relative()
            .size_full()
            .track_focus(&self.focus_handle)
            .tab_stop(false)
            .capture_any_mouse_down(cx.listener(|this, event: &MouseDownEvent, _, _| {
                if event.button == MouseButton::Left {
                    this.trigger_was_open = this.menu.is_some();
                }
            }))
            .child(
                Button::new("focus-tag-trigger")
                    .w_full()
                    .h(px(40.))
                    .px(px(10.))
                    .custom(
                        ButtonCustomVariant::new(cx)
                            .color(rgb(0xffffff).into())
                            .foreground(rgb(0x202020).into())
                            .border(rgb(0xe1e1e1).into())
                            .hover(rgb(0xf2f2f2).into())
                            .active(rgb(0xe9e9e9).into()),
                    )
                    .cursor_pointer()
                    .disabled(!self.enabled)
                    .when(!self.enabled, |button| {
                        button.bg(rgb(0xe9e9e9)).cursor_default()
                    })
                    .tooltip(help)
                    .child(
                        single_line(title, 14.)
                            .w(px(172.))
                            .text_color(rgb(0x202020)),
                    )
                    .child(
                        Icon::new(IconName::ChevronDown)
                            .size(px(13.))
                            .text_color(rgb(0x202020)),
                    )
                    .on_click(cx.listener(Self::toggle)),
            )
            .child(
                canvas(
                    move |bounds, _, cx| {
                        view.update(cx, |this, _| this.trigger_bounds = Some(bounds));
                    },
                    |_, _, _, _| {},
                )
                .absolute()
                .top_0()
                .left_0()
                .size_full(),
            )
            .when_some(self.menu.clone(), |body, menu| {
                body.when_some(self.trigger_bounds, |body, bounds| {
                    body.child(
                        deferred(
                            anchored()
                                .position(bounds.bottom_left() + point(px(0.), px(4.)))
                                .snap_to_window_with_margin(px(8.))
                                .child(menu),
                        )
                        .with_priority(1),
                    )
                })
            })
    }
}
