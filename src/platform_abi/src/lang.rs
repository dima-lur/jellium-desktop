//! Localized strings, loaded from the Fluent files under `i18n/` and
//! negotiated against the system's requested languages. A message missing
//! from the negotiated languages falls back to the `i18n.toml` fallback
//! language.

use std::collections::BTreeMap;
use std::sync::{Arc, OnceLock};

use i18n_embed::fluent::{FluentLanguageLoader, fluent_language_loader};
use i18n_embed::unic_langid::LanguageIdentifier;
use i18n_embed::{DesktopLanguageRequester, I18nEmbedError};
use i18n_embed_fl::fl;
use rust_embed::RustEmbed;

#[derive(RustEmbed)]
#[folder = "i18n"]
struct Localizations;

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum ContextMenuLabel {
    Back,
    Forward,
    Reload,
    Undo,
    Redo,
    Cut,
    Copy,
    Paste,
    SelectAll,
    Fullscreen,
    Settings,
    Exit,
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Label {
    ServerHost,
    ServerHostHelp,
    Submit,
    Failure,
    UnableToConnect,
    GotIt,
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum SettingsLabel {
    Language,
    LanguageHelp,
    LanguageSystem,
    HardwareDecodingAuto,
    HardwareDecodingDisabled,
    Version,
    ConfigDirectory,
    CurrentLog,
    Title,
    About,
    RestartNotice,
    Playback,
    Audio,
    Transcode,
    Advanced,
    MpvConfig,
    Server,
    HardwareDecoding,
    HardwareDecodingHelp,
    AudioPassthrough,
    AudioPassthroughHelp,
    ExclusiveAudio,
    ExclusiveAudioHelp,
    ChannelLayout,
    ChannelLayoutHelp,
    ChannelLayoutAuto,
    ChannelLayoutStereo,
    ChannelLayoutSurround51,
    ChannelLayoutSurround71,
    ForceTranscoding,
    ForceTranscodingHelp,
    WindowDecorations,
    WindowDecorationsHelp,
    WindowDecorationsAuto,
    WindowDecorationsCsd,
    WindowDecorationsServer,
    WindowDecorationsServerThemed,
    TransparentTitlebar,
    TransparentTitlebarHelp,
    HideScrollbar,
    HideScrollbarHelp,
    DeviceName,
    DeviceNameHelp,
    LogLevel,
    LogLevelHelp,
    LogLevelDefault,
    LogLevelVerbose,
    LogLevelDebug,
    LogLevelWarn,
    LogLevelError,
    OpenMpvConfig,
    ResetServer,
}

pub fn detect() -> Result<LanguageLoader, I18nEmbedError> {
    detect_with_preference("")
}

pub fn detect_with_preference(preferred: &str) -> Result<LanguageLoader, I18nEmbedError> {
    LanguageLoader::with_preference(preferred, &DesktopLanguageRequester::requested_languages())
}

/// Names are written in each language so the picker remains usable after a
/// language change. Only the names are retained, not a loader for every locale.
pub fn available_languages() -> &'static BTreeMap<String, String> {
    static LANGUAGES: OnceLock<BTreeMap<String, String>> = OnceLock::new();
    LANGUAGES.get_or_init(|| {
        Localizations::iter()
            .filter_map(|path| {
                let (id, _) = path.split_once('/')?;
                let language = id.parse().ok()?;
                let loader = LanguageLoader::new(&[language]).ok()?;
                Some((id.to_owned(), loader.0.get("language-name")))
            })
            .collect()
    })
}

/// Empty means follow the OS. Unknown or malformed overrides are ignored by
/// the settings store; a stale config must not prevent application startup.
pub fn normalize_language(value: &str) -> Option<String> {
    let value = value.trim();
    if value.is_empty() {
        return Some(String::new());
    }
    let id = value.parse::<LanguageIdentifier>().ok()?.to_string();
    available_languages().contains_key(&id).then_some(id)
}

/// The negotiated languages' bundles; clones share them.
#[derive(Clone)]
pub struct LanguageLoader(Arc<FluentLanguageLoader>);

impl LanguageLoader {
    pub fn with_preference(
        preferred: &str,
        system: &[LanguageIdentifier],
    ) -> Result<LanguageLoader, I18nEmbedError> {
        if let Some(preference) = normalize_language(preferred).and_then(|id| id.parse().ok()) {
            Self::new(&[preference])
        } else {
            Self::new(system)
        }
    }

    pub fn new(requested: &[LanguageIdentifier]) -> Result<LanguageLoader, I18nEmbedError> {
        let loader = fluent_language_loader!();
        i18n_embed::select(&loader, &Localizations, requested)?;
        Ok(LanguageLoader(Arc::new(loader)))
    }

    pub fn language(&self) -> LanguageIdentifier {
        i18n_embed::LanguageLoader::current_language(&*self.0)
    }

    pub fn menu_label(&self, label: ContextMenuLabel) -> String {
        let loader: &FluentLanguageLoader = &self.0;
        match label {
            ContextMenuLabel::Back => fl!(loader, "menu-back"),
            ContextMenuLabel::Forward => fl!(loader, "menu-forward"),
            ContextMenuLabel::Reload => fl!(loader, "menu-reload"),
            ContextMenuLabel::Undo => fl!(loader, "menu-undo"),
            ContextMenuLabel::Redo => fl!(loader, "menu-redo"),
            ContextMenuLabel::Cut => fl!(loader, "menu-cut"),
            ContextMenuLabel::Copy => fl!(loader, "menu-copy"),
            ContextMenuLabel::Paste => fl!(loader, "menu-paste"),
            ContextMenuLabel::SelectAll => fl!(loader, "menu-select-all"),
            ContextMenuLabel::Fullscreen => fl!(loader, "menu-fullscreen"),
            ContextMenuLabel::Settings => fl!(loader, "menu-settings"),
            ContextMenuLabel::Exit => fl!(loader, "menu-exit"),
        }
    }

    pub fn label(&self, label: Label) -> String {
        let loader: &FluentLanguageLoader = &self.0;
        match label {
            Label::ServerHost => fl!(loader, "connect-server-host"),
            Label::ServerHostHelp => fl!(loader, "connect-server-host-help"),
            Label::Submit => fl!(loader, "connect-submit"),
            Label::Failure => fl!(loader, "connect-failure"),
            Label::UnableToConnect => fl!(loader, "connect-unable"),
            Label::GotIt => fl!(loader, "connect-got-it"),
        }
    }

    pub fn settings_label(&self, label: SettingsLabel) -> String {
        let loader: &FluentLanguageLoader = &self.0;
        match label {
            SettingsLabel::Language => fl!(loader, "settings-language"),
            SettingsLabel::LanguageHelp => fl!(loader, "settings-language-help"),
            SettingsLabel::LanguageSystem => fl!(loader, "settings-language-system"),
            SettingsLabel::HardwareDecodingAuto => fl!(loader, "settings-hardware-decoding-auto"),
            SettingsLabel::HardwareDecodingDisabled => {
                fl!(loader, "settings-hardware-decoding-disabled")
            }
            SettingsLabel::Version => fl!(loader, "about-version"),
            SettingsLabel::ConfigDirectory => fl!(loader, "about-config-directory"),
            SettingsLabel::CurrentLog => fl!(loader, "about-current-log"),
            SettingsLabel::Title => fl!(loader, "settings-title"),
            SettingsLabel::About => fl!(loader, "settings-about"),
            SettingsLabel::RestartNotice => fl!(loader, "settings-restart-notice"),
            SettingsLabel::Playback => fl!(loader, "settings-section-playback"),
            SettingsLabel::Audio => fl!(loader, "settings-section-audio"),
            SettingsLabel::Transcode => fl!(loader, "settings-section-transcode"),
            SettingsLabel::Advanced => fl!(loader, "settings-section-advanced"),
            SettingsLabel::MpvConfig => fl!(loader, "settings-section-mpv-config"),
            SettingsLabel::Server => fl!(loader, "settings-section-server"),
            SettingsLabel::HardwareDecoding => fl!(loader, "settings-hardware-decoding"),
            SettingsLabel::HardwareDecodingHelp => fl!(loader, "settings-hardware-decoding-help"),
            SettingsLabel::AudioPassthrough => fl!(loader, "settings-audio-passthrough"),
            SettingsLabel::AudioPassthroughHelp => fl!(loader, "settings-audio-passthrough-help"),
            SettingsLabel::ExclusiveAudio => fl!(loader, "settings-exclusive-audio"),
            SettingsLabel::ExclusiveAudioHelp => fl!(loader, "settings-exclusive-audio-help"),
            SettingsLabel::ChannelLayout => fl!(loader, "settings-channel-layout"),
            SettingsLabel::ChannelLayoutHelp => fl!(loader, "settings-channel-layout-help"),
            SettingsLabel::ChannelLayoutAuto => fl!(loader, "settings-channel-layout-auto"),
            SettingsLabel::ChannelLayoutStereo => fl!(loader, "settings-channel-layout-stereo"),
            SettingsLabel::ChannelLayoutSurround51 => {
                fl!(loader, "settings-channel-layout-surround-5-1")
            }
            SettingsLabel::ChannelLayoutSurround71 => {
                fl!(loader, "settings-channel-layout-surround-7-1")
            }
            SettingsLabel::ForceTranscoding => fl!(loader, "settings-force-transcoding"),
            SettingsLabel::ForceTranscodingHelp => fl!(loader, "settings-force-transcoding-help"),
            SettingsLabel::WindowDecorations => fl!(loader, "settings-window-decorations"),
            SettingsLabel::WindowDecorationsHelp => {
                fl!(loader, "settings-window-decorations-help")
            }
            SettingsLabel::WindowDecorationsAuto => {
                fl!(loader, "settings-window-decorations-auto")
            }
            SettingsLabel::WindowDecorationsCsd => fl!(loader, "settings-window-decorations-csd"),
            SettingsLabel::WindowDecorationsServer => {
                fl!(loader, "settings-window-decorations-server")
            }
            SettingsLabel::WindowDecorationsServerThemed => {
                fl!(loader, "settings-window-decorations-server-themed")
            }
            SettingsLabel::TransparentTitlebar => fl!(loader, "settings-transparent-titlebar"),
            SettingsLabel::TransparentTitlebarHelp => {
                fl!(loader, "settings-transparent-titlebar-help")
            }
            SettingsLabel::HideScrollbar => fl!(loader, "settings-hide-scrollbar"),
            SettingsLabel::HideScrollbarHelp => fl!(loader, "settings-hide-scrollbar-help"),
            SettingsLabel::DeviceName => fl!(loader, "settings-device-name"),
            SettingsLabel::DeviceNameHelp => fl!(loader, "settings-device-name-help"),
            SettingsLabel::LogLevel => fl!(loader, "settings-log-level"),
            SettingsLabel::LogLevelHelp => fl!(loader, "settings-log-level-help"),
            SettingsLabel::LogLevelDefault => fl!(loader, "settings-log-level-default"),
            SettingsLabel::LogLevelVerbose => fl!(loader, "settings-log-level-verbose"),
            SettingsLabel::LogLevelDebug => fl!(loader, "settings-log-level-debug"),
            SettingsLabel::LogLevelWarn => fl!(loader, "settings-log-level-warn"),
            SettingsLabel::LogLevelError => fl!(loader, "settings-log-level-error"),
            SettingsLabel::OpenMpvConfig => fl!(loader, "settings-open-mpv-config"),
            SettingsLabel::ResetServer => fl!(loader, "settings-reset-server"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    #[test]
    fn every_catalog_parses_and_every_message_resolves() {
        let mut catalogs = BTreeMap::new();
        for path in Localizations::iter() {
            let asset = Localizations::get(&path).unwrap();
            let source = std::str::from_utf8(&asset.data).unwrap();
            let resource = fluent_syntax::parser::parse(source).unwrap();
            let mut keys = BTreeSet::new();
            for entry in resource.body {
                if let fluent_syntax::ast::Entry::Message(message) = entry {
                    assert!(message.value.is_some(), "{path}: {}", message.id.name);
                    assert!(
                        keys.insert(message.id.name.to_owned()),
                        "duplicate in {path}"
                    );
                }
            }
            let (id, _) = path.split_once('/').unwrap();
            catalogs.insert(id.to_owned(), keys);
        }
        let english = &catalogs["en"];
        assert_eq!(catalogs.len(), available_languages().len());
        for (id, keys) in &catalogs {
            assert!(keys.contains("language-name"), "{id}");
            assert!(keys.is_subset(english), "unknown key in {id}");
            let loader = LanguageLoader::new(&[id.parse().unwrap()]).unwrap();
            assert_eq!(loader.language().to_string(), *id);
            for key in english {
                let value = loader.0.get(key);
                assert!(!value.is_empty() && value != *key, "{id}: {key}");
            }
        }
        for id in ["ru", "be-BY"] {
            assert_eq!(
                &catalogs[id], english,
                "incomplete primary translation: {id}"
            );
        }
    }

    #[test]
    fn explicit_language_overrides_system_and_invalid_override_is_ignored() {
        let system = ["ru-RU".parse().unwrap()];
        let selected = LanguageLoader::with_preference("en", &system).unwrap();
        assert_eq!(selected.settings_label(SettingsLabel::Title), "Settings");
        // An explicit language must not borrow untranslated messages from the
        // OS language: its fallback is English, independent of this machine.
        let german = LanguageLoader::with_preference("de", &system).unwrap();
        assert_eq!(
            german.settings_label(SettingsLabel::Language),
            "Shell language"
        );
        for preference in ["", "unknown-language", "../../ru"] {
            let loader = LanguageLoader::with_preference(preference, &system).unwrap();
            assert_eq!(loader.settings_label(SettingsLabel::Title), "Настройки");
        }
    }

    #[test]
    fn unsupported_system_language_and_missing_messages_use_english() {
        let loader = LanguageLoader::new(&["zz-ZZ".parse().unwrap()]).unwrap();
        assert_eq!(loader.settings_label(SettingsLabel::Title), "Settings");
        let german = LanguageLoader::new(&["de-DE".parse().unwrap()]).unwrap();
        assert_eq!(german.language().to_string(), "de");
        assert_eq!(
            german.settings_label(SettingsLabel::Language),
            "Shell language"
        );
    }
}
