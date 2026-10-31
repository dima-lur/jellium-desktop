//! Localized strings, loaded from the Fluent files under `i18n/` and
//! negotiated against the system's requested languages. A message missing
//! from the negotiated languages falls back to the `i18n.toml` fallback
//! language.

use std::sync::Arc;

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
    LanguageLoader::new(&DesktopLanguageRequester::requested_languages())
}

/// The negotiated languages' bundles; clones share them.
#[derive(Clone)]
pub struct LanguageLoader(Arc<FluentLanguageLoader>);

impl LanguageLoader {
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
