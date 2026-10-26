//! Application translations. UI keys remain stable when the display language changes.

use gpui_kit::{SharedString, component::select::SelectItem};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    sync::{
        OnceLock,
        atomic::{AtomicU8, Ordering},
    },
};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Language {
    #[default]
    #[serde(rename = "zh-Hans")]
    SimplifiedChinese,
    #[serde(rename = "en")]
    English,
    #[serde(rename = "zh-Hant")]
    TraditionalChinese,
    #[serde(rename = "ja")]
    Japanese,
    #[serde(rename = "fr")]
    French,
    #[serde(rename = "es")]
    Spanish,
    #[serde(rename = "pl")]
    Polish,
    #[serde(rename = "ru")]
    Russian,
}

type Catalog = HashMap<String, [String; 8]>;
static CATALOG: OnceLock<Catalog> = OnceLock::new();
static CURRENT: AtomicU8 = AtomicU8::new(0);

impl Language {
    pub const ALL: [Self; 8] = [
        Self::SimplifiedChinese,
        Self::English,
        Self::TraditionalChinese,
        Self::Japanese,
        Self::French,
        Self::Spanish,
        Self::Polish,
        Self::Russian,
    ];

    pub fn native_name(self) -> &'static str {
        match self {
            Self::SimplifiedChinese => "简体中文",
            Self::English => "English",
            Self::TraditionalChinese => "繁體中文",
            Self::Japanese => "日本語",
            Self::French => "Français",
            Self::Spanish => "Español",
            Self::Polish => "Polski",
            Self::Russian => "Русский",
        }
    }

    pub fn activate(self) {
        install_widget_translations();
        CURRENT.store(self as u8, Ordering::Relaxed);
        gpui_kit::component::set_locale(match self {
            Self::SimplifiedChinese => "zh-CN",
            Self::TraditionalChinese => "zh-TW",
            Self::English => "en",
            Self::Japanese => "ja",
            Self::French => "fr",
            Self::Spanish => "es",
            Self::Polish => "pl",
            Self::Russian => "ru",
        });
    }

    pub fn current() -> Self {
        Self::ALL[usize::from(CURRENT.load(Ordering::Relaxed)).min(Self::ALL.len() - 1)]
    }

    pub fn translate(self, key: &str) -> &str {
        let catalog = CATALOG.get_or_init(|| {
            serde_json::from_str(include_str!("../locales/messages.json"))
                .expect("bundled translations must be valid JSON")
        });
        catalog
            .get(key)
            .map_or(key, |values| values[self as usize].as_str())
    }
}

struct WidgetBackend;
static WIDGET_BACKEND: WidgetBackend = WidgetBackend;
static WIDGET_CATALOG: OnceLock<Catalog> = OnceLock::new();
static WIDGET_TRANSLATIONS_INSTALLED: OnceLock<()> = OnceLock::new();

fn install_widget_translations() {
    // GPUI Component exposes rust-i18n's extension hook for application overrides.
    WIDGET_TRANSLATIONS_INSTALLED.get_or_init(|| {
        gpui_kit::component::_rust_i18n_extend(&WIDGET_BACKEND, "bandix_widgets");
    });
}

impl rust_i18n::Backend for WidgetBackend {
    fn available_locales(&self) -> Vec<std::borrow::Cow<'_, str>> {
        ["zh-CN", "en", "zh-TW", "ja", "fr", "es", "pl", "ru"]
            .into_iter()
            .map(std::borrow::Cow::Borrowed)
            .collect()
    }
    fn translate(&self, locale: &str, key: &str) -> Option<std::borrow::Cow<'_, str>> {
        let key = key.strip_prefix("bandix_widgets.")?;
        let index = ["zh-CN", "en", "zh-TW", "ja", "fr", "es", "pl", "ru"]
            .iter()
            .position(|item| *item == locale)?;
        let catalog = WIDGET_CATALOG.get_or_init(|| {
            serde_json::from_str(include_str!("../locales/widgets.json"))
                .expect("valid widget translations")
        });
        catalog
            .get(key)
            .map(|values| std::borrow::Cow::Borrowed(values[index].as_str()))
    }
    fn messages_for_locale(
        &self,
        locale: &str,
    ) -> Option<Vec<(std::borrow::Cow<'_, str>, std::borrow::Cow<'_, str>)>> {
        let catalog = WIDGET_CATALOG.get_or_init(|| {
            serde_json::from_str(include_str!("../locales/widgets.json"))
                .expect("valid widget translations")
        });
        let messages = catalog
            .keys()
            .filter_map(|key| {
                let namespaced = format!("bandix_widgets.{key}");
                self.translate(locale, &namespaced)
                    .map(|value| (std::borrow::Cow::Owned(namespaced), value))
            })
            .collect::<Vec<_>>();
        (!messages.is_empty()).then_some(messages)
    }
}

impl SelectItem for Language {
    type Value = Self;
    fn title(&self) -> SharedString {
        self.native_name().into()
    }
    fn value(&self) -> &Self {
        self
    }
}

pub fn tr(key: &str) -> &str {
    Language::current().translate(key)
}

pub fn text(key: &str) -> SharedString {
    tr(key).to_owned().into()
}

pub fn tf(key: &str, arguments: &[String]) -> String {
    format_message(tr(key), arguments)
}

fn format_message(template: &str, arguments: &[String]) -> String {
    let mut result = String::with_capacity(template.len());
    let mut remaining = template;
    while let Some(start) = remaining.find('{') {
        result.push_str(&remaining[..start].replace("}}", "}"));
        remaining = &remaining[start + 1..];
        if remaining.starts_with('{') {
            result.push('{');
            remaining = &remaining[1..];
            continue;
        }
        let Some(end) = remaining.find('}') else {
            result.push('{');
            break;
        };
        let name = &remaining[..end];
        if let Some(value) = name
            .parse::<usize>()
            .ok()
            .and_then(|index| arguments.get(index))
        {
            result.push_str(value);
        } else {
            result.push('{');
            result.push_str(name);
            result.push('}');
        }
        remaining = &remaining[end + 1..];
    }
    result.push_str(&remaining.replace("}}", "}"));
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_language_has_complete_messages_and_matching_placeholders() {
        let catalog: Catalog =
            serde_json::from_str(include_str!("../locales/messages.json")).unwrap();
        for (key, translations) in &catalog {
            let fields = |s: &str| {
                let mut fields = s
                    .split('{')
                    .skip(1)
                    .filter_map(|part| part.split_once('}'))
                    .map(|(name, _)| name.to_string())
                    .collect::<Vec<_>>();
                fields.sort();
                fields
            };
            for (index, translation) in translations.iter().enumerate() {
                assert!(!translation.is_empty(), "empty {key}: {index}");
                assert_eq!(
                    fields(key),
                    fields(translation),
                    "placeholders {key}: {index}"
                );
            }
        }
        assert_eq!(Language::English.translate("设置"), "Settings");
        assert_eq!(Language::Japanese.translate("设备列表"), "デバイス一覧");
        assert_eq!(Language::French.translate("设置"), "Paramètres");
        assert_eq!(
            Language::English.translate("a device name"),
            "a device name"
        );
        assert_eq!(
            format_message("{1}: {0}", &["one".into(), "two".into()]),
            "two: one"
        );
        assert_eq!(format_message("{0}", &["a{1}b".into()]), "a{1}b");
        assert_eq!(format_message("{0}", &["a}}b".into()]), "a}}b");
    }

    #[test]
    fn date_picker_and_input_controls_have_all_supported_languages() {
        use rust_i18n::Backend;
        let catalog: Catalog =
            serde_json::from_str(include_str!("../locales/widgets.json")).unwrap();
        for values in catalog.values() {
            assert!(values.iter().all(|value| !value.is_empty()));
        }
        assert_eq!(
            WIDGET_BACKEND
                .translate("fr", "bandix_widgets.Calendar.month.October")
                .unwrap(),
            "octobre"
        );
        assert_eq!(
            WIDGET_BACKEND
                .translate("ja", "bandix_widgets.Input.Copy")
                .unwrap(),
            "コピー"
        );
        assert_eq!(WIDGET_BACKEND.available_locales().len(), 8);
    }
}
