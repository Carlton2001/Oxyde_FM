//! UI messages are resolved at render time. Catalogs are parsed at startup and
//! when switching language, never inside render functions.
//! French is the fallback; invalid/missing messages fall back to French, then
//! to a visible `[message-id]` marker and a diagnostic (never a runtime panic).

pub use fluent_bundle::FluentArgs;
use fluent_bundle::{FluentBundle, FluentResource};
use gpui::{App, Context, Global, ReadGlobal, SharedString};
pub use oxyde_lib::models::preferences::Language;
use unic_langid::LanguageIdentifier;

const FRENCH: &str = include_str!("../../locales/fr.ftl");
const ENGLISH: &str = include_str!("../../locales/en.ftl");

/// Stable identifiers, independent of the text displayed to users.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Message {
    EjectDrive,
    EjectDriveFailed,
    MountDiskImage,
    UnmountDiskImage,
    DiskImageMountFailed,
    DiskImageUnmountFailed,
    DiskImageOpenFailed,
    DiskImageBusy,
    DiskImageNoVolume,
    Cancel,
    RenameTitle,
    RenameNamePlaceholder,
    RenameEmptyName,
    RenameAlreadyExists,
    RenameNotFound,
    RenameAccessDenied,
    RenameInvalidName,
    RenameUnavailable,
    RenameTaskFailed,
    ConflictTitle,
    ConflictDescription,
    ConflictSkipAll,
    ConflictReplaceAll,
    Close,
    Retry,
    SettingsTitle,
    SettingsAppearance,
    SettingsTheme,
    SettingsScale,
    SettingsScaleValue,
    SettingsScaleReset,
    SettingsLanguage,
    SettingsRegionLanguage,
    SettingsPerformance,
    SettingsTurbo,
    SettingsSearchLimit,
    SettingsData,
    SettingsClearCache,
    SettingsResetPreferences,
    SettingsComingSoon,

    SettingsDateFormat,
    DateFormatUs,
    DateFormatEuropean,
    DateFormatIso,
    SettingsNetwork,
    SettingsNetworkDescription,

    ThemeDark,
    ThemeLight,
    SettingsFiles,
    SettingsHidden,
    SettingsHiddenDescription,
    SettingsSystem,
    SettingsSystemDescription,
    SettingsPreviews,
    SettingsPreviewsDescription,
    SettingsWindowsIcons,
    SettingsWindowsIconsDescription,
    SettingsPathTracking,
    SettingsPathTrackingDescription,
    SettingsCheckboxes,
    SettingsCheckboxesDescription,
    SettingsGridThumbnails,
    SettingsGridThumbnailsDescription,
    SettingsAbout,
    SettingsVersion,
    SettingsDescription,
    SettingsSaveError,
    SettingsLoadError,
    StatusFolders,
    StatusFiles,
    ViewDetails,
    ViewGrid,
    SelectedFiles,
    SelectedFolders,
    SelectedMixed,
    TooltipType,
    TooltipSize,
    TooltipModified,
    TooltipLocation,
    Open,
    OpenNewTab,
    PinQuickAccess,
    UnpinQuickAccess,
    Cut,
    Copy,
    Paste,
    Rename,
    Delete,
    Properties,
    MoreOptions,
    NewFolder,
    DuplicateTab,
    CloseOtherTabs,
    CloseTab,
    SelectAll,
    ExpandAll,
    CollapseAll,
    CopyPath,
    SelectAllChecks,
    DeselectAllChecks,
    SortBy,
    SortAscending,
    SortDescending,
    SortExtension,
    ColumnName,
    ColumnType,
    ColumnSize,
    ColumnModified,
    FileTypeFolder,
    FileTypeFile,
    FileTypeSymlink,
    FileTypeJunction,
    EmptyFiltered,
    FolderLoading,
    FolderEmpty,
    PreviewLoading,
    OperationFailed,
    PathPlaceholder,
    Back,
    Forward,
    Refresh,
    NewPanel,
    NewTab,
    Tools,
    ToolSizes,
    ToolSearch,
    QuickSearch,
    QuickSearchTitle,
    QuickSearchResults,
    QuickSearchRunning,
    QuickSearchLimited,
    QuickSearchEmpty,
    QuickSearchFailed,
    QuickSearchClear,
    QuickSearchStop,
    OpenFileLocation,
    ColumnLocation,
    ToolDuplicates,
    Undo,
    Redo,
    OpCopy,
    OpMove,
    OpRename,
    OpDelete,
    OpCompress,
    OpExtract,
    Compress,
    ArchiveFormat,
    Compression,
    CompressionFormat,
    CompressionFast,
    CompressionNormal,
    CompressionBest,
    OpExtractHere,
    OpNewFolder,
    OpRestore,
    OpsTitle,
    OpsShowPanel,
    OpsHidePanel,
    OpsQueued,
    OpsCalculating,
    OpsRunning,
    OpsPaused,
    OpsCancelled,
    OpsCompleted,
    OpsError,
    OpsFrom,
    OpsTo,
    OpsItems,
    OpsRemaining,
    OpsDuration,
    OpsAverage,
    OpsSpeedWaiting,
    OpsPause,
    OpsResume,
    OpsStop,
    OpsFast,
    OpsNormal,
    OpsDismiss,
    OpsDismissAll,
    OpsEmpty,
    NameFilterPlaceholder,
    UndoOperation,
    UndoOverwriteUnavailable,
    RedoOperation,
    ShortcutBack,
    ShortcutForward,
    SidebarCollapseAll,
    SidebarScrollTop,
    SidebarCollapse,
    SidebarExpand,
    SidebarQuickAccess,
    SidebarDrives,
    SidebarLocations,
    SidebarNetwork,
    MapNetworkDrive,
    DisconnectNetworkDrive,
    DisconnectThisNetworkDrive,
    NetworkForcePrompt,
    NetworkCategory,
    TooltipNetworkCategory,
    TooltipNetworkWorkgroup,
    TooltipNetworkLocation,
    NetworkWorkgroup,
    NetworkLocation,
    NetworkComputer,
    NetworkOtherDevices,
    NetworkForceAction,
    NetworkDriveFailed,
    OpenItemFailed,
    NetworkMapDescription,
    NetworkMapHint,
    NetworkDisconnectDescription,
    NetworkLetter,
    NetworkFolder,
    NetworkUsername,
    NetworkPassword,
    NetworkReconnect,
    NetworkFinish,
    NetworkDisconnect,
    NetworkNoDrives,
    NetworkNoLetter,
    NetworkInvalidPath,
    NetworkConnected,
    NetworkDisconnected,
    NetworkWorking,
    NetworkLoading,
    SidebarTrash,
    SizeTiny,
    SizeSmall,
    SizeMedium,
    SizeLarge,
    SizeHuge,
    DateThisMonth,
    SettingsGroupByDate,
    SettingsGroupByDateDescription,
    DateGroupCount,
    DateToday,
    DateYesterday,
    DateThisWeek,
    DateLastWeek,
    DateOlder,
    PathNotFound,
    PathAccessDenied,
    PathLoadFailed,
    UndoFailed,
    RedoFailed,
    PrepareFailed,
    TransferFailed,
    DragCopy,
    DragMove,
    NewFolderName,
    NewFolderNumbered,
    NativeMenuEmpty,
    NativeMenuFailed,
    DragItems,
    NativeMenuLoading,
    LocalDrive,
    UnknownItem,
}

impl Message {
    pub fn key(self) -> &'static str {
        match self {
            Self::EjectDrive => "drive-eject",
            Self::EjectDriveFailed => "drive-eject-failed",
            Self::MountDiskImage => "disk-image-mount",
            Self::UnmountDiskImage => "disk-image-unmount",
            Self::DiskImageMountFailed => "disk-image-mount-failed",
            Self::DiskImageUnmountFailed => "disk-image-unmount-failed",
            Self::DiskImageOpenFailed => "disk-image-open-failed",
            Self::DiskImageBusy => "disk-image-busy",
            Self::DiskImageNoVolume => "disk-image-no-volume",
            Self::Cancel => "common-cancel",
            Self::RenameTitle => "rename-title",
            Self::RenameNamePlaceholder => "rename-name-placeholder",
            Self::RenameEmptyName => "rename-error-empty-name",
            Self::RenameAlreadyExists => "rename-error-already-exists",
            Self::RenameNotFound => "rename-error-not-found",
            Self::RenameAccessDenied => "rename-error-access-denied",
            Self::RenameInvalidName => "rename-error-invalid-name",
            Self::RenameUnavailable => "rename-error-unavailable",
            Self::RenameTaskFailed => "rename-error-task-failed",
            Self::ConflictTitle => "conflict-title",
            Self::ConflictDescription => "conflict-description",
            Self::ConflictSkipAll => "conflict-skip-all",
            Self::ConflictReplaceAll => "conflict-replace-all",
            Self::Close => "common-close",
            Self::Retry => "common-retry",
            Self::SettingsTitle => "settings-title",
            Self::SettingsAppearance => "settings-appearance",
            Self::SettingsTheme => "settings-theme",
            Self::SettingsScale => "settings-scale",
            Self::SettingsScaleValue => "settings-scale-value",
            Self::SettingsScaleReset => "settings-scale-reset",
            Self::SettingsLanguage => "settings-language",
            Self::SettingsRegionLanguage => "settings-region-language",
            Self::SettingsPerformance => "settings-performance",
            Self::SettingsTurbo => "settings-turbo",
            Self::SettingsSearchLimit => "settings-search-limit",
            Self::SettingsData => "settings-data",
            Self::SettingsClearCache => "settings-clear-cache",
            Self::SettingsResetPreferences => "settings-reset-preferences",
            Self::SettingsComingSoon => "settings-coming-soon",

            Self::SettingsDateFormat => "settings-date-format",
            Self::DateFormatUs => "date-format-us",
            Self::DateFormatEuropean => "date-format-european",
            Self::DateFormatIso => "date-format-iso",
            Self::SettingsNetwork => "settings-network",
            Self::SettingsNetworkDescription => "settings-network-description",

            Self::ThemeDark => "theme-dark",
            Self::ThemeLight => "theme-light",
            Self::SettingsFiles => "settings-files",
            Self::SettingsHidden => "settings-hidden",
            Self::SettingsHiddenDescription => "settings-hidden-description",
            Self::SettingsSystem => "settings-system",
            Self::SettingsSystemDescription => "settings-system-description",
            Self::SettingsPreviews => "settings-previews",
            Self::SettingsPreviewsDescription => "settings-previews-description",
            Self::SettingsWindowsIcons => "settings-windows-icons",
            Self::SettingsWindowsIconsDescription => "settings-windows-icons-description",
            Self::SettingsPathTracking => "settings-path-tracking",
            Self::SettingsPathTrackingDescription => "settings-path-tracking-description",
            Self::SettingsCheckboxes => "settings-checkboxes",
            Self::SettingsCheckboxesDescription => "settings-checkboxes-description",
            Self::SettingsGridThumbnails => "settings-grid-thumbnails",
            Self::SettingsGridThumbnailsDescription => "settings-grid-thumbnails-description",
            Self::SettingsAbout => "settings-about",
            Self::SettingsVersion => "settings-version",
            Self::SettingsDescription => "settings-description",
            Self::SettingsSaveError => "settings-save-error",
            Self::SettingsLoadError => "settings-load-error",
            Self::StatusFolders => "status-folders",
            Self::StatusFiles => "status-files",
            Self::ViewDetails => "view-details",
            Self::ViewGrid => "view-grid",
            Self::SelectedFiles => "selected-files",
            Self::SelectedFolders => "selected-folders",
            Self::SelectedMixed => "selected-mixed",
            Self::TooltipType => "tooltip-type",
            Self::TooltipSize => "tooltip-size",
            Self::TooltipModified => "tooltip-modified",
            Self::TooltipLocation => "tooltip-location",
            Self::Open => "menu-open",
            Self::OpenNewTab => "menu-open-new-tab",
            Self::PinQuickAccess => "menu-pin-quick-access",
            Self::UnpinQuickAccess => "menu-unpin-quick-access",
            Self::Cut => "menu-cut",
            Self::Copy => "menu-copy",
            Self::Paste => "menu-paste",
            Self::Rename => "menu-rename",
            Self::Delete => "menu-delete",
            Self::Properties => "menu-properties",
            Self::MoreOptions => "menu-more-options",
            Self::NewFolder => "menu-new-folder",
            Self::DuplicateTab => "menu-duplicate-tab",
            Self::CloseOtherTabs => "menu-close-other-tabs",
            Self::CloseTab => "menu-close-tab",
            Self::SelectAll => "menu-select-all",
            Self::ExpandAll => "menu-expand-all",
            Self::CollapseAll => "menu-collapse-all",
            Self::CopyPath => "menu-copy-path",
            Self::SelectAllChecks => "menu-select-all-checks",
            Self::DeselectAllChecks => "menu-deselect-all-checks",
            Self::SortBy => "menu-sort-by",
            Self::SortAscending => "menu-sort-ascending",
            Self::SortDescending => "menu-sort-descending",
            Self::SortExtension => "menu-sort-extension",
            Self::ColumnName => "column-name",
            Self::ColumnType => "column-type",
            Self::ColumnSize => "column-size",
            Self::ColumnModified => "column-modified",
            Self::FileTypeFolder => "file-type-folder",
            Self::FileTypeFile => "file-type-file",
            Self::FileTypeSymlink => "file-type-symlink",
            Self::FileTypeJunction => "file-type-junction",
            Self::EmptyFiltered => "empty-filtered",
            Self::FolderLoading => "folder-loading",
            Self::FolderEmpty => "folder-empty",
            Self::PreviewLoading => "preview-loading",
            Self::OperationFailed => "operation-failed",
            Self::PathPlaceholder => "path-placeholder",
            Self::Back => "navigation-back",
            Self::Forward => "navigation-forward",
            Self::Refresh => "navigation-refresh",
            Self::NewPanel => "title-new-panel",
            Self::NewTab => "tabs-new-tab",
            Self::Tools => "title-tools",
            Self::ToolSizes => "tool-sizes",
            Self::ToolSearch => "tool-search",
            Self::QuickSearch => "quick-search",
            Self::QuickSearchTitle => "quick-search-title",
            Self::QuickSearchResults => "quick-search-results",
            Self::QuickSearchRunning => "quick-search-running",
            Self::QuickSearchLimited => "quick-search-limited",
            Self::QuickSearchEmpty => "quick-search-empty",
            Self::QuickSearchFailed => "quick-search-failed",
            Self::QuickSearchClear => "quick-search-clear",
            Self::QuickSearchStop => "quick-search-stop",
            Self::OpenFileLocation => "open-file-location",
            Self::ColumnLocation => "column-location",
            Self::ToolDuplicates => "tool-duplicates",
            Self::Undo => "navigation-undo",
            Self::Redo => "navigation-redo",
            Self::OpCopy => "operation-copy",
            Self::OpMove => "operation-move",
            Self::OpRename => "operation-rename",
            Self::OpDelete => "operation-delete",
            Self::OpCompress => "operation-compress",
            Self::OpExtract => "operation-extract",
            Self::Compress => "compress",
            Self::ArchiveFormat => "archive-format",
            Self::Compression => "compression",
            Self::CompressionFormat => "compression-format",
            Self::CompressionFast => "compression-fast",
            Self::CompressionNormal => "compression-normal",
            Self::CompressionBest => "compression-best",
            Self::OpExtractHere => "operation-extract-here",
            Self::OpNewFolder => "operation-new-folder",
            Self::OpRestore => "operation-restore",
            Self::OpsTitle => "operations-title",
            Self::OpsShowPanel => "operations-show-panel",
            Self::OpsHidePanel => "operations-hide-panel",
            Self::OpsQueued => "operations-queued",
            Self::OpsCalculating => "operations-calculating",
            Self::OpsRunning => "operations-running",
            Self::OpsPaused => "operations-paused",
            Self::OpsCancelled => "operations-cancelled",
            Self::OpsCompleted => "operations-completed",
            Self::OpsError => "operations-error",
            Self::OpsFrom => "operations-from",
            Self::OpsTo => "operations-to",
            Self::OpsItems => "operations-items",
            Self::OpsRemaining => "operations-remaining",
            Self::OpsDuration => "operations-duration",
            Self::OpsAverage => "operations-average",
            Self::OpsSpeedWaiting => "operations-speed-waiting",
            Self::OpsPause => "operations-pause",
            Self::OpsResume => "operations-resume",
            Self::OpsStop => "operations-stop",
            Self::OpsFast => "operations-fast",
            Self::OpsNormal => "operations-normal",
            Self::OpsDismiss => "operations-dismiss",
            Self::OpsDismissAll => "operations-dismiss-all",
            Self::OpsEmpty => "operations-empty",
            Self::NameFilterPlaceholder => "name-filter-placeholder",
            Self::UndoOperation => "undo-operation",
            Self::UndoOverwriteUnavailable => "undo-overwrite-unavailable",
            Self::RedoOperation => "redo-operation",
            Self::ShortcutBack => "shortcut-back",
            Self::ShortcutForward => "shortcut-forward",
            Self::SidebarCollapseAll => "sidebar-collapse-all",
            Self::SidebarScrollTop => "sidebar-scroll-top",
            Self::SidebarCollapse => "sidebar-collapse",
            Self::SidebarExpand => "sidebar-expand",
            Self::SidebarQuickAccess => "sidebar-quick-access",
            Self::SidebarDrives => "sidebar-drives",
            Self::SidebarLocations => "sidebar-locations",
            Self::SidebarNetwork => "sidebar-network",
            Self::MapNetworkDrive => "map-network-drive",
            Self::DisconnectNetworkDrive => "disconnect-network-drive",
            Self::DisconnectThisNetworkDrive => "disconnect-this-network-drive",
            Self::NetworkForcePrompt => "network-force-prompt",
            Self::NetworkCategory => "network-category",
            Self::TooltipNetworkCategory => "tooltip-network-category",
            Self::TooltipNetworkWorkgroup => "tooltip-network-workgroup",
            Self::TooltipNetworkLocation => "tooltip-network-location",
            Self::NetworkWorkgroup => "network-workgroup",
            Self::NetworkLocation => "network-location",
            Self::NetworkComputer => "network-computer",
            Self::NetworkOtherDevices => "network-other-devices",
            Self::NetworkForceAction => "network-force-action",
            Self::NetworkDriveFailed => "network-drive-failed",
            Self::OpenItemFailed => "open-item-failed",
            Self::NetworkMapDescription => "network-map-description",
            Self::NetworkMapHint => "network-map-hint",
            Self::NetworkDisconnectDescription => "network-disconnect-description",
            Self::NetworkLetter => "network-letter",
            Self::NetworkFolder => "network-folder",
            Self::NetworkUsername => "network-username",
            Self::NetworkPassword => "network-password",
            Self::NetworkReconnect => "network-reconnect",
            Self::NetworkFinish => "network-finish",
            Self::NetworkDisconnect => "network-disconnect",
            Self::NetworkNoDrives => "network-no-drives",
            Self::NetworkNoLetter => "network-no-letter",
            Self::NetworkInvalidPath => "network-invalid-path",
            Self::NetworkConnected => "network-connected",
            Self::NetworkDisconnected => "network-disconnected",
            Self::NetworkWorking => "network-working",
            Self::NetworkLoading => "network-loading",
            Self::SidebarTrash => "sidebar-trash",
            Self::SizeTiny => "size-tiny",
            Self::SizeSmall => "size-small",
            Self::SizeMedium => "size-medium",
            Self::SizeLarge => "size-large",
            Self::SizeHuge => "size-huge",
            Self::DateThisMonth => "date-this-month",
            Self::SettingsGroupByDate => "settings-group-by-date",
            Self::SettingsGroupByDateDescription => "settings-group-by-date-description",
            Self::DateGroupCount => "date-group-count",
            Self::DateToday => "date-today",
            Self::DateYesterday => "date-yesterday",
            Self::DateThisWeek => "date-this-week",
            Self::DateLastWeek => "date-last-week",
            Self::DateOlder => "date-older",
            Self::PathNotFound => "path-error-not-found",
            Self::PathAccessDenied => "path-error-access-denied",
            Self::PathLoadFailed => "path-error-failed",
            Self::UndoFailed => "undo-failed",
            Self::RedoFailed => "redo-failed",
            Self::PrepareFailed => "prepare-failed",
            Self::TransferFailed => "transfer-failed",
            Self::DragCopy => "drag-copy",
            Self::DragMove => "drag-move",
            Self::NewFolderName => "new-folder-name",
            Self::NewFolderNumbered => "new-folder-numbered",
            Self::NativeMenuEmpty => "native-menu-empty",
            Self::NativeMenuFailed => "native-menu-failed",
            Self::DragItems => "drag-items",
            Self::NativeMenuLoading => "native-menu-loading",
            Self::LocalDrive => "local-drive",
            Self::UnknownItem => "unknown-item",
        }
    }
}

pub struct I18n {
    language: Language,
    active: FluentBundle<FluentResource>,
    fallback: FluentBundle<FluentResource>,
}

impl Global for I18n {}

impl Default for I18n {
    fn default() -> Self {
        Self::new("fr", FRENCH).expect("the embedded French catalog must be valid")
    }
}

impl I18n {
    pub fn for_language(language: Language) -> Self {
        let catalog = match language {
            Language::French => FRENCH,
            Language::English => ENGLISH,
        };
        Self::new(language.as_str(), catalog).expect("embedded catalogs must be valid")
    }

    pub fn language(&self) -> Language {
        self.language
    }

    /// Locale identifiers use BCP 47 (e.g. `fr`, `en-GB`). The caller loads the
    /// catalog before constructing this global; no disk access occurs in render.
    pub fn new(locale: &str, catalog: &str) -> Result<Self, String> {
        Ok(Self {
            language: Language::from_id(locale),
            active: Self::bundle(locale, catalog)?,
            fallback: Self::bundle("fr", FRENCH)?,
        })
    }

    fn bundle(locale: &str, catalog: &str) -> Result<FluentBundle<FluentResource>, String> {
        let locale: LanguageIdentifier = locale
            .parse()
            .map_err(|error| format!("Invalid locale: {error}"))?;
        let resource = FluentResource::try_new(catalog.to_owned())
            .map_err(|(_, errors)| format!("Invalid catalog: {errors:?}"))?;
        let mut bundle = FluentBundle::new(vec![locale]);
        bundle
            .add_resource(resource)
            .map_err(|errors| format!("Duplicate catalog messages: {errors:?}"))?;
        Ok(bundle)
    }

    pub fn text(&self, message: Message) -> SharedString {
        self.format(message.key(), None)
    }

    pub fn text_with_args(&self, message: Message, args: &FluentArgs<'_>) -> SharedString {
        self.format(message.key(), Some(args))
    }

    fn format(&self, key: &str, args: Option<&FluentArgs<'_>>) -> SharedString {
        for bundle in [&self.active, &self.fallback] {
            let Some(pattern) = bundle.get_message(key).and_then(|message| message.value()) else {
                continue;
            };
            let mut errors = Vec::new();
            let text = bundle.format_pattern(pattern, args, &mut errors);
            if errors.is_empty() {
                return text.into_owned().into();
            }
            log::warn!("Cannot format UI message {key}: {errors:?}");
        }
        log::warn!("No usable translation for UI message {key}");
        format!("[{key}]").into()
    }
}

pub trait I18nContext {
    fn i18n(&self) -> &I18n;
}

impl I18nContext for App {
    fn i18n(&self) -> &I18n {
        I18n::global(self)
    }
}

impl<T> I18nContext for Context<'_, T> {
    fn i18n(&self) -> &I18n {
        I18n::global(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dialog_catalog_has_all_messages_and_valid_parameters() {
        for language in [Language::French, Language::English] {
            let i18n = I18n::for_language(language);
            let messages = [
                Message::EjectDrive,
                Message::EjectDriveFailed,
                Message::MountDiskImage,
                Message::UnmountDiskImage,
                Message::DiskImageMountFailed,
                Message::DiskImageUnmountFailed,
                Message::DiskImageOpenFailed,
                Message::DiskImageBusy,
                Message::DiskImageNoVolume,
                Message::Cancel,
                Message::RenameTitle,
                Message::RenameNamePlaceholder,
                Message::RenameEmptyName,
                Message::RenameAlreadyExists,
                Message::RenameNotFound,
                Message::RenameAccessDenied,
                Message::RenameInvalidName,
                Message::RenameUnavailable,
                Message::RenameTaskFailed,
                Message::ConflictTitle,
                Message::ConflictDescription,
                Message::ConflictSkipAll,
                Message::ConflictReplaceAll,
                Message::Close,
                Message::Retry,
                Message::SettingsTitle,
                Message::SettingsAppearance,
                Message::SettingsTheme,
                Message::SettingsScale,
                Message::SettingsScaleReset,
                Message::SettingsScaleValue,
                Message::SettingsLanguage,
                Message::SettingsRegionLanguage,
                Message::SettingsPerformance,
                Message::SettingsTurbo,
                Message::SettingsSearchLimit,
                Message::SettingsData,
                Message::SettingsClearCache,
                Message::SettingsResetPreferences,
                Message::SettingsComingSoon,

                Message::SettingsDateFormat,
                Message::DateFormatUs,
                Message::DateFormatEuropean,
                Message::DateFormatIso,
                Message::SettingsNetwork,
                Message::SettingsNetworkDescription,

                Message::ThemeDark,
                Message::ThemeLight,
                Message::SettingsFiles,
                Message::SettingsHidden,
                Message::SettingsHiddenDescription,
                Message::SettingsSystem,
                Message::SettingsSystemDescription,
                Message::SettingsPreviews,
                Message::SettingsPreviewsDescription,
                Message::SettingsWindowsIcons,
                Message::SettingsWindowsIconsDescription,
                Message::SettingsPathTracking,
                Message::SettingsPathTrackingDescription,
                Message::SettingsCheckboxes,
                Message::SettingsCheckboxesDescription,
                Message::SettingsGridThumbnails,
                Message::SettingsGridThumbnailsDescription,
                Message::SettingsAbout,
                Message::SettingsVersion,
                Message::SettingsDescription,
                Message::SettingsSaveError,
                Message::SettingsLoadError,
                Message::StatusFolders,
                Message::StatusFiles,
                Message::ViewDetails,
                Message::ViewGrid,
                Message::SelectedFiles,
                Message::SelectedFolders,
                Message::SelectedMixed,
                Message::TooltipType,
                Message::TooltipSize,
                Message::TooltipModified,
                Message::TooltipLocation,
                Message::Open,
                Message::OpenNewTab,
                Message::PinQuickAccess,
                Message::UnpinQuickAccess,
                Message::Cut,
                Message::Copy,
                Message::Paste,
                Message::Rename,
                Message::Delete,
                Message::Properties,
                Message::MoreOptions,
                Message::NewFolder,
                Message::DuplicateTab,
                Message::CloseOtherTabs,
                Message::CloseTab,
                Message::SelectAll,
                Message::ExpandAll,
                Message::CollapseAll,
                Message::CopyPath,
                Message::SelectAllChecks,
                Message::DeselectAllChecks,
                Message::SortBy,
                Message::SortAscending,
                Message::SortDescending,
                Message::SortExtension,
                Message::ColumnName,
                Message::ColumnType,
                Message::ColumnSize,
                Message::ColumnModified,
                Message::FileTypeFolder,
                Message::FileTypeFile,
                Message::FileTypeSymlink,
                Message::FileTypeJunction,
                Message::EmptyFiltered,
                Message::FolderLoading,
                Message::FolderEmpty,
                Message::PreviewLoading,
                Message::OperationFailed,
                Message::PathPlaceholder,
                Message::Back,
                Message::Forward,
                Message::Refresh,
                Message::NewPanel,
                Message::NewTab,
                Message::Tools,
                Message::ToolSizes,
                Message::ToolSearch,
                Message::QuickSearch,
                Message::QuickSearchTitle,
                Message::QuickSearchResults,
                Message::QuickSearchRunning,
                Message::QuickSearchLimited,
                Message::QuickSearchEmpty,
                Message::QuickSearchFailed,
                Message::QuickSearchClear,
                Message::QuickSearchStop,
                Message::OpenFileLocation,
                Message::ColumnLocation,
                Message::ToolDuplicates,
                Message::Undo,
                Message::Redo,
                Message::OpCopy,
                Message::OpMove,
                Message::OpRename,
                Message::OpDelete,
                Message::OpCompress,
                Message::OpExtract,
                Message::Compress,
                Message::ArchiveFormat,
                Message::Compression,
                Message::CompressionFormat,
                Message::CompressionFast,
                Message::CompressionNormal,
                Message::CompressionBest,
                Message::OpExtractHere,
                Message::OpNewFolder,
                Message::OpRestore,
                Message::OpsTitle,
                Message::OpsShowPanel,
                Message::OpsHidePanel,
                Message::OpsQueued,
                Message::OpsCalculating,
                Message::OpsRunning,
                Message::OpsPaused,
                Message::OpsCancelled,
                Message::OpsCompleted,
                Message::OpsError,
                Message::OpsFrom,
                Message::OpsTo,
                Message::OpsItems,
                Message::OpsRemaining,
                Message::OpsDuration,
                Message::OpsAverage,
                Message::OpsSpeedWaiting,
                Message::OpsPause,
                Message::OpsResume,
                Message::OpsStop,
                Message::OpsFast,
                Message::OpsNormal,
                Message::OpsDismiss,
                Message::OpsDismissAll,
                Message::OpsEmpty,
                Message::NameFilterPlaceholder,
                Message::UndoOperation,
                Message::UndoOverwriteUnavailable,
                Message::RedoOperation,
                Message::ShortcutBack,
                Message::ShortcutForward,
                Message::SidebarCollapseAll,
                Message::SidebarScrollTop,
                Message::SidebarCollapse,
                Message::SidebarExpand,
                Message::SidebarQuickAccess,
                Message::SidebarDrives,
                Message::SidebarLocations,
                Message::SidebarNetwork,
                Message::MapNetworkDrive,
                Message::DisconnectNetworkDrive,
                Message::DisconnectThisNetworkDrive,
                Message::NetworkForcePrompt,
                Message::NetworkCategory,
                Message::TooltipNetworkCategory,
                Message::TooltipNetworkWorkgroup,
                Message::TooltipNetworkLocation,
                Message::NetworkWorkgroup,
                Message::NetworkLocation,
                Message::NetworkComputer,
                Message::NetworkOtherDevices,
                Message::NetworkForceAction,
                Message::NetworkDriveFailed,
                Message::OpenItemFailed,
                Message::NetworkMapDescription,
                Message::NetworkMapHint,
                Message::NetworkDisconnectDescription,
                Message::NetworkLetter,
                Message::NetworkFolder,
                Message::NetworkUsername,
                Message::NetworkPassword,
                Message::NetworkReconnect,
                Message::NetworkFinish,
                Message::NetworkDisconnect,
                Message::NetworkNoDrives,
                Message::NetworkNoLetter,
                Message::NetworkInvalidPath,
                Message::NetworkConnected,
                Message::NetworkDisconnected,
                Message::NetworkWorking,
                Message::NetworkLoading,
                Message::SidebarTrash,
                Message::SizeTiny,
                Message::SizeSmall,
                Message::SizeMedium,
                Message::SizeLarge,
                Message::SizeHuge,
                Message::DateThisMonth,
                Message::SettingsGroupByDate,
                Message::SettingsGroupByDateDescription,
                Message::DateGroupCount,
                Message::DateToday,
                Message::DateYesterday,
                Message::DateThisWeek,
                Message::DateLastWeek,
                Message::DateOlder,
                Message::PathNotFound,
                Message::PathAccessDenied,
                Message::PathLoadFailed,
                Message::UndoFailed,
                Message::RedoFailed,
                Message::PrepareFailed,
                Message::TransferFailed,
                Message::DragCopy,
                Message::DragMove,
                Message::NewFolderName,
                Message::NewFolderNumbered,
                Message::NativeMenuEmpty,
                Message::NativeMenuFailed,
                Message::DragItems,
                Message::NativeMenuLoading,
                Message::LocalDrive,
                Message::UnknownItem,
            ];
            let mut args = FluentArgs::new();
            args.set("count", 2);
            args.set("folder_count", 1);
            args.set("query", "us");
            args.set("folder", "Downloads");
            args.set("file_count", 1);
            args.set("number", "2");
            args.set("version", "1.0");
            args.set("format", "zip");
            args.set("reference", 16);
            args.set("size", "1,5 Ko");
            args.set("folders", "1");
            args.set("files", "1");
            args.set("value", "Données");
            args.set("operation", "copie");
            args.set("detail", "Erreur système");
            args.set("drive", "C:");
            args.set("path", "C:/example");
            args.set("done", "1");
            args.set("total", "2");
            args.set("seconds", "10");
            args.set("duration", "1.2 s");
            args.set("speed", "42 Mo/s");
            for message in messages {
                let pattern = i18n
                    .active
                    .get_message(message.key())
                    .unwrap()
                    .value()
                    .unwrap();
                let mut errors = Vec::new();
                let text = i18n
                    .active
                    .format_pattern(pattern, Some(&args), &mut errors);
                assert!(!text.is_empty(), "{} is empty", message.key());
                assert!(errors.is_empty(), "{}: {errors:?}", message.key());
            }
        }
    }

    #[test]
    fn operations_title_uses_singular_only_for_one_operation() {
        for (language, singular, plural) in [
            (Language::French, "Opération", "Opérations"),
            (Language::English, "Operation", "Operations"),
        ] {
            let i18n = I18n::for_language(language);
            for count in [0, 1, 2, 1000] {
                let mut args = FluentArgs::new();
                args.set("count", count);
                assert_eq!(
                    i18n.text_with_args(Message::OpsTitle, &args).as_ref(),
                    if count == 1 { singular } else { plural },
                );
            }
        }
    }

    #[test]
    fn conflict_count_uses_french_plural_rules() {
        let i18n = I18n::default();
        for (count, expected) in [
            (0, "conflit détecté"),
            (1, "conflit détecté"),
            (2, "conflits détectés"),
        ] {
            let mut args = FluentArgs::new();
            args.set("count", count);
            args.set("number", count.to_string());
            let text = i18n.text_with_args(Message::ConflictDescription, &args);
            assert!(text.contains(expected), "{text}");
            assert!(text.contains(&count.to_string()), "{text}");
        }
    }

    #[test]
    fn missing_or_broken_translation_falls_back_to_french() {
        let i18n = I18n::new("en", "common-cancel = Cancel\nrename-title = { $missing }").unwrap();
        assert_eq!(i18n.text(Message::Cancel).as_ref(), "Cancel");
        assert_eq!(i18n.text(Message::RenameTitle).as_ref(), "Renommer");
        assert_eq!(
            i18n.text(Message::ConflictTitle).as_ref(),
            "Fichiers déjà présents"
        );
        assert_eq!(i18n.format("unknown-key", None).as_ref(), "[unknown-key]");
    }

    #[test]
    fn fallback_uses_its_own_plural_rules_and_named_arguments() {
        let i18n = I18n::new("en", "common-cancel = Cancel").unwrap();
        let mut args = FluentArgs::new();
        args.set("count", 0);
        args.set("number", "0");
        assert!(i18n
            .text_with_args(Message::ConflictDescription, &args)
            .contains("conflit détecté"));
    }

    #[test]
    fn missing_arguments_produce_a_visible_marker_without_panicking() {
        assert_eq!(
            I18n::default().text(Message::ConflictDescription).as_ref(),
            "[conflict-description]"
        );
    }

    #[test]
    fn malformed_catalogs_are_rejected_at_load_time() {
        assert!(I18n::new("fr", "broken = { $unfinished").is_err());
        assert!(I18n::new("fr", "key = One\nkey = Two").is_err());
        assert!(I18n::new("!invalid!", "key = Value").is_err());
    }

    #[test]
    fn english_and_french_use_different_zero_plural_rules() {
        for (language, expected) in [
            (Language::French, "0 fichier"),
            (Language::English, "0 files"),
        ] {
            let i18n = I18n::for_language(language);
            let mut args = FluentArgs::new();
            args.set("count", 0);
            args.set("number", "0");
            let actual = i18n
                .text_with_args(Message::StatusFiles, &args)
                .replace(['\u{2068}', '\u{2069}'], "");
            assert_eq!(actual, expected);
        }
    }

    #[test]
    fn user_values_are_not_interpreted_as_translation_syntax() {
        let value = "Données { $count } 日本語.txt";
        let mut args = FluentArgs::new();
        args.set("value", value);
        for language in [Language::French, Language::English] {
            assert!(I18n::for_language(language)
                .text_with_args(Message::TooltipType, &args)
                .contains(value));
        }
    }
}
