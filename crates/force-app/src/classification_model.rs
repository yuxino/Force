use crate::model::{ForceView, Page};
use force_core::{ClassColors, Session, Tag, TagClass};
use gpui::{AppContext as _, Context, Entity, PromptButton, PromptLevel, Subscription, Window};
use gpui_component::input::{InputEvent, InputState};

pub(crate) struct ClassificationState {
    pub(crate) colors: ClassColors,
    pub(crate) editing: Option<TagClass>,
    pub(crate) input: Entity<InputState>,
    pub(crate) error: Option<String>,
    baseline: String,
    _subscription: Subscription,
}

impl ClassificationState {
    pub(crate) fn new(window: &mut Window, cx: &mut Context<ForceView>) -> Self {
        let input = cx.new(|cx| InputState::new(window, cx).placeholder("#RRGGBB"));
        let subscription =
            cx.subscribe_in(&input, window, |this, _, event: &InputEvent, window, cx| {
                if matches!(event, InputEvent::Change) {
                    this.classification.error = None;
                    cx.notify();
                } else if matches!(event, InputEvent::PressEnter { secondary: false }) {
                    this.save_class_color_edit(window, cx);
                }
            });
        Self {
            colors: ClassColors::default(),
            editing: None,
            input,
            error: None,
            baseline: String::new(),
            _subscription: subscription,
        }
    }

    fn changed(&self, cx: &gpui::App) -> bool {
        self.editing.is_some()
            && self.input.read(cx).value().trim().to_ascii_uppercase() != self.baseline
    }
}

impl ForceView {
    pub(crate) fn display_color<'a>(
        &'a self,
        tag_id: Option<i64>,
        snapshot_class: TagClass,
        snapshot_color: Option<&'a str>,
    ) -> Option<&'a str> {
        match tag_id.and_then(|id| self.tags.iter().find(|tag| tag.id == id)) {
            Some(tag) => self
                .classification
                .colors
                .resolve(tag.class, tag.color.as_deref()),
            None => self
                .classification
                .colors
                .resolve(snapshot_class, snapshot_color),
        }
    }

    pub(crate) fn display_tag_color<'a>(&'a self, tag: &'a Tag) -> Option<&'a str> {
        self.display_color(Some(tag.id), tag.class, tag.color.as_deref())
    }

    pub(crate) fn display_session_color<'a>(&'a self, session: &'a Session) -> Option<&'a str> {
        self.display_color(
            Some(session.tag_id),
            session.tag_class,
            session.tag_color.as_deref(),
        )
    }

    pub(crate) fn begin_class_color_edit(
        &mut self,
        class: TagClass,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if class == TagClass::Legacy {
            return;
        }
        if self.classification.editing == Some(class) {
            self.classification
                .input
                .update(cx, |input, cx| input.focus(window, cx));
            return;
        }
        if self.classification.changed(cx) {
            let editing = self.classification.editing;
            let draft = self.classification.input.read(cx).value();
            let answer = window.prompt(
                PromptLevel::Info,
                "保存分类颜色？",
                None,
                &[
                    PromptButton::cancel("取消"),
                    PromptButton::new("不保存"),
                    PromptButton::new("保存"),
                ],
                cx,
            );
            cx.spawn_in(window, async move |this, cx| {
                if let Ok(choice) = answer.await {
                    let _ = cx.update(|window, cx| {
                        let _ = this.update(cx, |this, cx| {
                            if this.classification.editing != editing
                                || this.classification.input.read(cx).value() != draft
                            {
                                return;
                            }
                            if choice == 0 {
                                if this.page == Page::Settings && !this.compact {
                                    this.classification
                                        .input
                                        .update(cx, |input, cx| input.focus(window, cx));
                                }
                                return;
                            }
                            if choice == 2 && !this.save_class_color_edit(window, cx) {
                                return;
                            }
                            this.classification.editing = None;
                            this.begin_class_color_edit(class, window, cx);
                        });
                    });
                }
            })
            .detach();
            return;
        }
        let color = self
            .classification
            .colors
            .color(class)
            .unwrap_or_default()
            .to_owned();
        self.classification.editing = Some(class);
        self.classification.baseline = color.clone();
        self.classification.error = None;
        self.classification.input.update(cx, |input, cx| {
            input.set_value(color, window, cx);
            input.focus(window, cx);
        });
        cx.notify();
    }

    pub(crate) fn save_class_color_edit(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some(class) = self.classification.editing else {
            return false;
        };
        if self.page != Page::Settings || self.compact || self.moments.visible {
            return false;
        }
        let color = match parse_class_color(self.classification.input.read(cx).value().as_ref()) {
            Ok(color) => color,
            Err(message) => {
                self.classification.error = Some(message.to_owned());
                cx.notify();
                return false;
            }
        };
        if self.write(|store| store.set_class_color(class, color.as_deref()), cx) {
            self.cancel_class_color_edit(window, cx);
            true
        } else {
            self.classification.error = Some("颜色未保存，请重试。".to_owned());
            cx.notify();
            false
        }
    }

    pub(crate) fn cancel_class_color_edit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.classification.editing = None;
        self.classification.error = None;
        window.focus(&self.focus_handle);
        cx.notify();
    }
}

fn parse_class_color(value: &str) -> Result<Option<String>, &'static str> {
    let value = value.trim();
    if value.is_empty() {
        return Ok(None);
    }
    if value.len() != 7
        || !value.starts_with('#')
        || !value[1..].bytes().all(|byte| byte.is_ascii_hexdigit())
    {
        return Err("颜色格式为 #RRGGBB。");
    }
    Ok(Some(value.to_ascii_uppercase()))
}

#[cfg(test)]
mod tests {
    use super::parse_class_color;
    #[test]
    fn custom_class_colors_accept_defaults_and_valid_hex_only() {
        assert_eq!(parse_class_color("  "), Ok(None));
        assert_eq!(
            parse_class_color(" #abc123 "),
            Ok(Some("#ABC123".to_owned()))
        );
        for value in ["#ABC", "ABC123", "#ABC12G", "#1234567", "中文颜色"] {
            assert!(parse_class_color(value).is_err());
        }
    }
}

#[cfg(all(test, feature = "ui-test-support"))]
mod ui_tests {
    use super::*;
    use crate::headless_ui_tests::ManagerHarness;
    use force_core::TagKind;
    use gpui::TestAppContext;

    #[gpui::test]
    fn classification_ui_validates_custom_colors_and_updates_inherited_tags(
        cx: &mut TestAppContext,
    ) {
        let mut manager = ManagerHarness::new_on_page(cx, 1040., 780., Page::Settings);
        manager.click("edit-class-color-positive");
        manager.cx.simulate_input("#bad");
        manager.click("save-class-color");
        manager.view.read_with(&manager.cx, |view, cx| {
            assert!(view.classification.error.is_some());
            assert_eq!(view.classification.input.read(cx).value(), "#bad");
            assert_eq!(view.classification.colors, ClassColors::default());
        });
        manager.click("class-color-input");
        manager.cx.simulate_keystrokes("secondary-a");
        manager.cx.simulate_input("#6c9079");
        manager.cx.simulate_keystrokes("enter");
        manager.cx.run_until_parked();
        manager.view.read_with(&manager.cx, |view, _| {
            assert!(view.classification.editing.is_none());
            assert_eq!(
                view.classification.colors.positive.as_deref(),
                Some("#6C9079")
            );
            let work = view.tags.iter().find(|tag| tag.name == "工作").unwrap();
            assert_eq!(work.color, None);
            assert_eq!(view.display_tag_color(work), Some("#6C9079"));
            assert_eq!(
                view.store.as_ref().unwrap().class_colors().unwrap(),
                view.classification.colors
            );
        });
    }

    #[gpui::test]
    fn classification_ui_retains_dirty_color_when_switch_save_fails(cx: &mut TestAppContext) {
        let mut manager = ManagerHarness::new_on_page(cx, 1040., 780., Page::Settings);
        manager.click("edit-class-color-positive");
        manager.cx.simulate_input("#bad");
        manager.view.update_in(&mut manager.cx, |view, window, cx| {
            view.begin_class_color_edit(TagClass::Negative, window, cx);
        });
        manager.cx.run_until_parked();
        assert!(manager.cx.has_pending_prompt());
        manager.cx.simulate_prompt_answer("保存");
        manager.cx.run_until_parked();
        manager.view.read_with(&manager.cx, |view, cx| {
            assert_eq!(view.classification.editing, Some(TagClass::Positive));
            assert_eq!(view.classification.input.read(cx).value(), "#bad");
            assert!(view.classification.error.is_some());
        });
        manager.view.update_in(&mut manager.cx, |view, window, cx| {
            view.begin_class_color_edit(TagClass::Negative, window, cx);
        });
        manager.cx.run_until_parked();
        manager.cx.simulate_prompt_answer("不保存");
        manager.cx.run_until_parked();
        manager.view.read_with(&manager.cx, |view, cx| {
            assert_eq!(view.classification.editing, Some(TagClass::Negative));
            assert!(view.classification.input.read(cx).value().is_empty());
            assert_eq!(
                view.store.as_ref().unwrap().class_colors().unwrap(),
                ClassColors::default()
            );
        });
    }

    #[gpui::test]
    fn classification_display_resolves_current_overrides_and_archived_snapshot_defaults(
        cx: &mut TestAppContext,
    ) {
        let mut manager = ManagerHarness::new_on_page(cx, 1040., 780., Page::Settings);
        manager.view.update_in(&mut manager.cx, |view, _, cx| {
            let store = view.store.as_mut().unwrap();
            let inherited = store.create_tag("Inherited", TagKind::Focus).unwrap();
            let custom = store
                .create_tag_with_details(
                    "Custom",
                    TagKind::Focus,
                    TagClass::Positive,
                    Some("#123456"),
                )
                .unwrap();
            let session = store.start("Old record", inherited.id, 100).unwrap();
            let session = store.finish(session.id, 160).unwrap();
            store
                .set_class_color(TagClass::Positive, Some("#6C9079"))
                .unwrap();
            store
                .set_class_color(TagClass::Negative, Some("#B56F67"))
                .unwrap();
            view.refresh(cx);
            assert_eq!(view.display_tag_color(&inherited), Some("#6C9079"));
            assert_eq!(view.display_tag_color(&custom), Some("#123456"));
            assert_eq!(view.display_session_color(&session), Some("#6C9079"));
            view.store
                .as_mut()
                .unwrap()
                .update_tag(inherited.id, &inherited.name, TagClass::Negative, None)
                .unwrap();
            view.refresh(cx);
            assert_eq!(view.display_session_color(&session), Some("#B56F67"));
            view.store
                .as_mut()
                .unwrap()
                .delete_tag(inherited.id)
                .unwrap();
            view.refresh(cx);
            assert_eq!(view.display_session_color(&session), Some("#6C9079"));
            assert_eq!(view.display_color(None, TagClass::Legacy, None), None);
            assert_eq!(
                view.display_color(None, TagClass::Positive, Some("#123456")),
                Some("#123456")
            );
        });
    }
}
