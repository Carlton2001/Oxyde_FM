mod dialog;
mod general;
mod interactions;
mod loading;
mod shortcut;
mod window;
mod selection;

pub(crate) use window::close_properties_windows;

use crate::ui::components::input::TextInput;
use crate::ui::i18n::Message;
use gpui::{Bounds, Entity, FocusHandle, Pixels, Point};
use oxyde_lib::models::{DriveInfo, FileProperties};

pub(crate) struct PropertiesWindow {
    owner: gpui::WeakEntity<super::OxydeApp>,
    owner_window: gpui::AnyWindowHandle,
    properties_dialog: Option<PropertiesDialog>,
    context_menus: Vec<super::ContextMenuInstance>,
    scrollbar_origins: std::collections::HashMap<String, Point<f32>>,
    _subscriptions: Vec<gpui::Subscription>,
    _calculation: Option<gpui::Task<()>>,
    cancellation: std::sync::Arc<std::sync::atomic::AtomicBool>,
}

impl Drop for PropertiesWindow {
    fn drop(&mut self) {
        self.cancellation.store(true, std::sync::atomic::Ordering::Relaxed);
    }
}

impl crate::ui::components::scrollbar::ScrollbarState for PropertiesWindow {
    fn scrollbar_origins(&mut self) -> &mut std::collections::HashMap<String, Point<f32>> {
        &mut self.scrollbar_origins
    }
    fn set_grab_offset(&mut self, _: f32) {}
}

pub(crate) struct PropertiesDialog {
    path: String,
    paths: Vec<String>,
    selection: Option<selection::SelectionProperties>,
    revision: u64,
    properties: Option<FileProperties>,
    drive: Option<DriveInfo>,
    calculating: bool,
    error: Option<String>,
    focus: FocusHandle,
    ok_focus: FocusHandle,
    shortcut_tab: bool,
    shortcut_inputs: Vec<Entity<TextInput>>,
    run_window: i32,
    run_menu: Option<Point<Pixels>>,
    run_focus: FocusHandle,
    run_bounds: Option<Bounds<Pixels>>,
    saving: bool,
    validation_error: Option<Message>,
}

struct PropertyPresentation<'a> {
    name: &'a str,
    location: &'a str,
    location_label: Message,
    deleted_time: Option<u64>,
}

fn property_presentation(properties: &FileProperties) -> PropertyPresentation<'_> {
    let original_identity = properties
        .original_path
        .as_deref()
        .map(std::path::Path::new)
        .filter(|path| path.is_absolute())
        .and_then(|path| Some((path.file_name()?.to_str()?, path.parent()?.to_str()?)));
    let (name, location, location_label) = match original_identity {
        Some((name, location)) => (name, location, Message::TrashOriginalLocation),
        None => (
            properties.name.as_str(),
            properties.parent.as_str(),
            Message::ColumnLocation,
        ),
    };
    PropertyPresentation {
        name,
        location,
        location_label,
        deleted_time: properties
            .deleted_time
            .and_then(|time| u64::try_from(time).ok()),
    }
}

#[cfg(test)]
mod property_presentation_tests {
    use super::{property_presentation, FileProperties, Message};

    fn fixture() -> FileProperties {
        FileProperties {
            name: "$Rfixture".into(),
            path: std::env::temp_dir()
                .join("physical")
                .join("$Rfixture")
                .to_string_lossy()
                .into_owned(),
            parent: std::env::temp_dir()
                .join("physical")
                .to_string_lossy()
                .into_owned(),
            is_dir: false,
            size: 17,
            is_calculated: false,
            created: 0,
            modified: 0,
            accessed: 0,
            original_path: Some(
                std::env::temp_dir()
                    .join("Documents")
                    .join("Rapport école.pdf")
                    .to_string_lossy()
                    .into_owned(),
            ),
            deleted_time: Some(1_791_370_123_000),
            folders_count: None,
            files_count: None,
            shortcut: None,
            link_target: None,
            is_broken_link: false,
        }
    }

    #[test]
    fn trash_properties_present_original_identity_and_keep_physical_path() {
        let properties = fixture();
        let physical = properties.path.clone();
        let original = properties.original_path.clone().unwrap();
        let presentation = property_presentation(&properties);
        assert_eq!(presentation.name, "Rapport école.pdf");
        assert_eq!(
            presentation.location,
            std::path::Path::new(&original)
                .parent()
                .unwrap()
                .to_str()
                .unwrap()
        );
        assert!(matches!(
            presentation.location_label,
            Message::TrashOriginalLocation
        ));
        assert_eq!(presentation.deleted_time, Some(1_791_370_123_000));
        assert_eq!(properties.path, physical);
        assert_eq!(properties.name, "$Rfixture");
        assert_ne!(properties.path, original);
    }
    #[test]
    fn folder_and_shortcut_names_keep_their_original_extensions() {
        use crate::ui::components::icons::{get_icon_type, IconType};
        for (name, is_dir, icon) in [
            ("Dossier école", true, IconType::Folder),
            ("Raccourci école.lnk", false, IconType::ExternalLink),
            ("Lien école.url", false, IconType::Link),
            ("Rapport école.pdf", false, IconType::FileText),
        ] {
            let mut properties = fixture();
            properties.is_dir = is_dir;
            properties.original_path = Some(
                std::env::temp_dir()
                    .join("Documents")
                    .join(name)
                    .to_string_lossy()
                    .into_owned(),
            );
            let presentation = property_presentation(&properties);
            assert_eq!(presentation.name, name);
            assert_eq!(
                get_icon_type(
                    presentation.name,
                    is_dir,
                    Some(&properties.path),
                    false,
                    false
                ),
                icon
            );
        }
    }

    #[test]
    fn ordinary_or_incomplete_properties_fall_back_without_a_fake_deletion_date() {
        let mut properties = fixture();
        for original in [None, Some(String::new()), Some("relative.txt".into())] {
            properties.original_path = original;
            properties.deleted_time = None;
            let presentation = property_presentation(&properties);
            assert_eq!(presentation.name, properties.name);
            assert_eq!(presentation.location, properties.parent);
            assert!(matches!(
                presentation.location_label,
                Message::ColumnLocation
            ));
            assert!(presentation.deleted_time.is_none());
        }
        properties.deleted_time = Some(-1);
        assert!(property_presentation(&properties).deleted_time.is_none());
    }
}
