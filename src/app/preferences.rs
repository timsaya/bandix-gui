//! Persistent display preferences, independent of server-side traffic settings.

use super::BandixApp;
use crate::i18n::Language;
use crate::i18n::{tf, tr};
use crate::services::api::format_rate;
use gpui_kit::Context;
use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::{self, Write},
    path::PathBuf,
};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum SpeedUnit {
    #[default]
    Bytes,
    Bits,
}

impl SpeedUnit {
    pub fn format(self, bytes_per_second: u64) -> String {
        if self == Self::Bytes {
            return format_rate(bytes_per_second);
        }
        const UNITS: [&str; 7] = ["bps", "Kbps", "Mbps", "Gbps", "Tbps", "Pbps", "Ebps"];
        // The server always returns B/s. Use u128 so multiplying by eight cannot overflow.
        let bits = u128::from(bytes_per_second) * 8;
        let mut value = bits as f64;
        let mut unit = 0;
        while value >= 1000.0 && unit < UNITS.len() - 1 {
            value /= 1000.0;
            unit += 1;
        }
        if unit == 0 {
            format!("{bits} bps")
        } else {
            format!("{value:.1} {}", UNITS[unit])
        }
    }
}

#[derive(Serialize, Deserialize)]
#[serde(default)]
struct DisplayPreferences {
    speed_unit: SpeedUnit,
    language: Language,
    refresh_interval_secs: u64,
}

impl Default for DisplayPreferences {
    fn default() -> Self {
        Self {
            speed_unit: SpeedUnit::default(),
            language: Language::default(),
            refresh_interval_secs: 1,
        }
    }
}

pub(super) struct DisplayStorage {
    path: PathBuf,
}

impl DisplayStorage {
    pub fn for_user() -> Result<Self, String> {
        let directory = dirs::config_dir().ok_or(tr("无法获取用户配置目录"))?;
        Ok(Self {
            path: directory.join("bandix-gui").join("display.json"),
        })
    }

    pub fn load(&self) -> Result<SpeedUnit, String> {
        self.read().map(|preferences| preferences.speed_unit)
    }

    pub fn load_language(&self) -> Result<Language, String> {
        self.read().map(|preferences| preferences.language)
    }

    pub fn load_refresh_interval(&self) -> Result<u64, String> {
        self.read()
            .map(|preferences| preferences.refresh_interval_secs.clamp(1, 60))
    }

    fn read(&self) -> Result<DisplayPreferences, String> {
        let bytes = match fs::read(&self.path) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                return Ok(DisplayPreferences::default());
            }
            Err(error) => return Err(error.to_string()),
        };
        serde_json::from_slice::<DisplayPreferences>(&bytes).map_err(|error| error.to_string())
    }

    pub fn save(&self, speed_unit: SpeedUnit) -> Result<(), String> {
        let mut preferences = self.read().unwrap_or_default();
        preferences.speed_unit = speed_unit;
        self.write(preferences)
    }

    pub fn save_language(&self, language: Language) -> Result<(), String> {
        let mut preferences = self.read().unwrap_or_default();
        preferences.language = language;
        self.write(preferences)
    }

    pub fn save_refresh_interval(&self, seconds: u64) -> Result<(), String> {
        let mut preferences = self.read().unwrap_or_default();
        preferences.refresh_interval_secs = seconds.clamp(1, 60);
        self.write(preferences)
    }

    fn write(&self, preferences: DisplayPreferences) -> Result<(), String> {
        let parent = self.path.parent().ok_or(tr("配置路径缺少父目录"))?;
        let write = || -> io::Result<()> {
            fs::create_dir_all(parent)?;
            let mut temporary = tempfile::NamedTempFile::new_in(parent)?;
            serde_json::to_writer_pretty(temporary.as_file_mut(), &preferences)?;
            temporary.write_all(b"\n")?;
            temporary.as_file().sync_all()?;
            temporary.persist(&self.path).map_err(|error| error.error)?;
            Ok(())
        };
        write().map_err(|error| error.to_string())
    }
}

impl BandixApp {
    pub(super) fn set_language(
        &mut self,
        language: Language,
        window: &mut gpui_kit::Window,
        cx: &mut Context<Self>,
    ) {
        self.language = language;
        language.activate();
        self.display_settings_error = DisplayStorage::for_user()
            .and_then(|storage| storage.save_language(language))
            .err()
            .map(|error| tf("语言已切换，但保存失败：{0}", &[error]));
        for (state, key) in [
            (&self.service_address, "例如 http://192.168.1.1:8686"),
            (&self.connection_page.search, "昵称 / IP / MAC"),
            (&self.dns_page.domain, "域名"),
            (&self.dns_page.device, "设备名 / IP / MAC"),
            (&self.dns_page.server, "DNS 服务器"),
        ] {
            state.update(cx, |input, cx| input.set_placeholder(tr(key), window, cx));
        }
        self.localize_device_search(window, cx);
        cx.notify();
    }

    pub(super) fn set_speed_unit(&mut self, unit: SpeedUnit, cx: &mut Context<Self>) {
        self.speed_unit = unit;
        self.display_settings_error = DisplayStorage::for_user()
            .and_then(|storage| storage.save(unit))
            .err()
            .map(|error| tf("速度单位已切换，但保存失败：{0}", &[format!("{}", error)]));
        cx.notify();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bits_convert_bytes_by_eight_and_use_decimal_prefixes() {
        for (bytes, expected) in [
            (0, "0 bps"),
            (1, "8 bps"),
            (124, "992 bps"),
            (125, "1.0 Kbps"),
            (125_000, "1.0 Mbps"),
            (125_000_000, "1.0 Gbps"),
            (125_000_000_000, "1.0 Tbps"),
            (u64::MAX, "147.6 Ebps"),
        ] {
            assert_eq!(SpeedUnit::Bits.format(bytes), expected);
        }
        assert_eq!(SpeedUnit::Bytes.format(1024), "1.0 KB/s");
        assert_eq!(SpeedUnit::Bytes.format(1_048_576), "1.0 MB/s");
        assert_eq!(SpeedUnit::Bits.format(1_048_576), "8.4 Mbps");
    }

    #[test]
    fn display_preferences_survive_reopens_and_handle_missing_or_invalid_files() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("配置目录/display.json");
        let storage = DisplayStorage { path: path.clone() };
        assert_eq!(storage.load().unwrap(), SpeedUnit::Bytes);
        assert!(!path.parent().unwrap().exists());
        for unit in [SpeedUnit::Bits, SpeedUnit::Bytes] {
            storage.save(unit).unwrap();
            assert_eq!(DisplayStorage { path: path.clone() }.load().unwrap(), unit);
        }
        fs::write(&path, "{}").unwrap();
        assert_eq!(storage.load().unwrap(), SpeedUnit::Bytes);
        fs::write(&path, "invalid").unwrap();
        assert!(storage.load().is_err());
        storage.save(SpeedUnit::Bits).unwrap();
        assert_eq!(storage.load().unwrap(), SpeedUnit::Bits);
        assert_eq!(fs::read_dir(path.parent().unwrap()).unwrap().count(), 1);
    }

    #[test]
    fn language_and_speed_preferences_preserve_each_other_and_load_legacy_files() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("display.json");
        let storage = DisplayStorage { path: path.clone() };
        fs::write(&path, br#"{"speed_unit":"bits"}"#).unwrap();
        assert_eq!(
            storage.load_language().unwrap(),
            Language::SimplifiedChinese
        );
        assert_eq!(storage.load_refresh_interval().unwrap(), 1);
        storage.save_refresh_interval(10).unwrap();
        for language in Language::ALL {
            storage.save_language(language).unwrap();
            assert_eq!(storage.load().unwrap(), SpeedUnit::Bits);
            assert_eq!(
                DisplayStorage { path: path.clone() }
                    .load_language()
                    .unwrap(),
                language
            );
            storage.save(SpeedUnit::Bytes).unwrap();
            assert_eq!(storage.load_language().unwrap(), language);
            storage.save(SpeedUnit::Bits).unwrap();
            assert_eq!(storage.load_refresh_interval().unwrap(), 10);
        }
    }

    #[test]
    fn refresh_interval_survives_reopens_and_preserves_language_and_speed() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("display.json");
        let storage = DisplayStorage { path: path.clone() };
        assert_eq!(storage.load_refresh_interval().unwrap(), 1);
        assert!(!path.exists());
        storage.save_language(Language::English).unwrap();
        storage.save(SpeedUnit::Bits).unwrap();
        for seconds in [1, 2, 5, 10, 30, 0, u64::MAX] {
            storage.save_refresh_interval(seconds).unwrap();
            let reopened = DisplayStorage { path: path.clone() };
            assert_eq!(
                reopened.load_refresh_interval().unwrap(),
                seconds.clamp(1, 60)
            );
            assert_eq!(reopened.load_language().unwrap(), Language::English);
            assert_eq!(reopened.load().unwrap(), SpeedUnit::Bits);
        }
        for (stored, expected) in [(0, 1), (u64::MAX, 60)] {
            fs::write(&path, format!(r#"{{"refresh_interval_secs":{stored}}}"#)).unwrap();
            assert_eq!(storage.load_refresh_interval().unwrap(), expected);
        }
    }
}
