# Audit : suivi des optimisations (série O)

Ouvert le 2026-11-20, à partir de la catégorie 6 de `101-restes.md`. Le texte de chaque constat est dans l'annexe de `101-restes.md` ; ce fichier n'en garde que l'emplacement et le suivi.

Règle : pas de changement sans gain plausible et mesurable ; un changement simple et sans risque (complexité quadratique, travail répété à chaque rendu, E/S sur le thread UI) peut se faire sans mesure. Un commit par constat sur `audit-fixes`, sans push.

États : « à examiner », « corrigé », « obsolète », « écarté » (raison en note), « à mesurer », « Linux ».

### Archives, workers et IPC

| ID | Emplacement | État | Commits | Note |
| --- | --- | --- | --- | --- |
| O1 | `commands/archive_io_worker.rs:57 et archive_io_codec.rs:77` | à examiner | | |
| O2 | `commands/archive_io_codec.rs:257` | à examiner | | |
| O3 | `commands/archive_io_codec/parallel.rs:94` | à examiner | | |
| O4 | `commands/archive.rs:231 et archive_handle_worker.rs:108` | à examiner | | |
| O5 | `systems/archive_open_cache/recovery.rs:199` | à examiner | | |
| O6 | `systems/archive_owned_tree.rs:463` | à examiner | | |
| O7 | `systems/archive_workspace.rs:203 et :191` | à examiner | | |
| O8 | `systems/archive_workspace.rs:414 (via archive_transfer.rs:68) et :592` | à examiner | | |
| O9 | `platform/windows_worker_sandbox.rs:197` | à examiner | | |
| O10 | `platform/windows_preview_quota.rs:290` | à examiner | | |
| O11 | `platform/windows_icon_pixels.rs:84` | à examiner | | |

### Fichiers, opérations, historique

| ID | Emplacement | État | Commits | Note |
| --- | --- | --- | --- | --- |
| O13 | `systems/file_ops/file_copy.rs:282` | à examiner | | |
| O14 | `systems/file_ops/copy.rs:315` | à examiner | | |
| O15 | `systems/file_ops/destination_tree.rs:134` | à examiner | | |
| O17 | `systems/file_ops/file_copy.rs:52` | à examiner | | |
| O19 | `systems/history_file_undo.rs:400 et :686-691` | à examiner | | |
| O20 | `systems/history_directories.rs:30-64` | à examiner | | |
| O21 | `systems/folder_sizes.rs:150-176` | à examiner | | |
| O22 | `systems/history_move_redo.rs:419` | à examiner | | |
| O23 | `systems/updates.rs:62` | à examiner | | |
| O24 | `commands/duplicates.rs:335, :390 et :445` | à examiner | | |
| O25 | `commands/duplicates.rs:330, :385 et :440, commands/search.rs:520` | à examiner | | |
| O26 | `commands/duplicates.rs:409-424` | à examiner | | |
| O28 | `commands/ops/trash_restore.rs:69` | à examiner | | |
| O29 | `commands/ops/linux_trash.rs:84` | Linux | | |
| O30 | `commands/search.rs:153, :172, :460 et :539` | Linux | | |
| O32 | `commands/sidebar.rs:42-46 et :117-121, :58 et :133` | à examiner | | |
| O33 | `models/config.rs:299-303 et :313-331` | à examiner | | |
| O35 | `utils/audio_metadata.rs:38-44` | à examiner | | |
| O36 | `utils/office_document.rs:50-57` | à examiner | | |
| O37 | `utils/preview_protocol.rs:148-156` | à examiner | | |
| O39 | `utils/office_document/sheets.rs:104-109` | à examiner | | |
| O40 | `platform/linux_gio/applications.rs:153` | Linux | | |
| O41 | `platform/linux_permissions.rs:33` | Linux | | |
| O42 | `platform/linux_mime.rs:184 et :227` | Linux | | |
| O43 | `platform/thumbnails.rs:172 et :360` | à examiner | | |
| O44 | `platform/windows_peek.rs:22` | à examiner | | |
| O45 | `platform/windows_shell_delete_tracked/effects.rs:63` | à examiner | | |
| O46 | `platform/windows_shell_window.rs:197` | à examiner | | |
| O47 | `platform/windows_file_tree.rs:72 et :169` | à examiner | | |

### Interface

| ID | Emplacement | État | Commits | Note |
| --- | --- | --- | --- | --- |
| O50 | `ui/app/archive_password.rs:58` | à examiner | | |
| O52 | `ui/app/calendar.rs:6` | à examiner | | |
| O53 | `ui/app/archives.rs:122` | à examiner | | |
| O54 | `ui/app/advanced_search/validation.rs:58` | à examiner | | |
| O55 | `ui/app/drag/external.rs:43-55, :98, :115-120 et :151-154` | à examiner | | |
| O56 | `ui/app/duplicates/search.rs:104-114` | à examiner | | |
| O57 | `ui/app/drive_watchers.rs:107 et :453-467` | à examiner | | |
| O59 | `ui/app/file_chooser/details.rs:45` | à examiner | | |
| O60 | `ui/app/file_chooser.rs:377` | à examiner | | |
| O61 | `ui/app/file_chooser/browse.rs:102` | à examiner | | |
| O62 | `ui/app/file_chooser/navbar.rs:84` | à examiner | | |
| O63 | `ui/app/file_chooser/browse.rs:133` | à examiner | | |
| O64 | `ui/app/duplicates/title_actions.rs:33` | à examiner | | |
| O65 | `ui/app/file_preparation.rs:119-123` | à examiner | | |
| O66 | `ui/app/file_chooser/sidebar.rs:201-209 et :249` | à examiner | | |
| O67 | `ui/app/file_chooser/save.rs:215-219 et :111-119` | à examiner | | |
| O68 | `ui/app/item_pointer.rs:30-34` | à examiner | | |
| O69 | `ui/app/layout/tabs.rs:70-101 et :232-241` | à examiner | | |
| O71 | `ui/app/selection.rs:114 et :193-206` | à examiner | | |
| O72 | `ui/app/type_select.rs:72` | à examiner | | |
| O73 | `ui/app/viewer.rs:243` | à examiner | | |
| O74 | `ui/app/viewer/markdown.rs:162` | à examiner | | |
| O76 | `ui/app/viewer/content.rs:144` | à examiner | | |
| O77 | `ui/app/viewer/text.rs:135, :281-308 et :334` | à examiner | | |
| O78 | `ui/app/viewer/tree/loading.rs:47-55` | à examiner | | |
| O81 | `ui/components/conflict_dialog/advanced.rs:123-243` | à examiner | | |
| O82 | `ui/components/conflict_dialog/controls.rs:46-48` | à examiner | | |
| O83 | `ui/components/dropdown.rs:252-297, :326-336, :395 et :417` | à examiner | | |
| O84 | `ui/components/file_list/card_body.rs:54, :92 et :105` | à examiner | | |
| O85 | `ui/components/file_list/details.rs:101, grid.rs:514 et selection_button.rs:16` | à examiner | | |
| O86 | `ui/components/input.rs:134-140 et :626-647` | à examiner | | |
| O87 | `ui/components/open_with_dialog.rs:28, :108, :113 et :159` | à examiner | | |
| O89 | `ui/components/operations/footer.rs:36-40 et :65-71` | à examiner | | |
| O90 | `ui/components/popup/window/frame.rs:90` | à examiner | | |
| O91 | `ui/components/sidebar/tree.rs:35, :47, :72, :119-129 et :172, rows.rs:49` | à examiner | | |
| O92 | `ui/components/tooltip.rs:78-88` | à examiner | | |
| O93 | `ui/components/tooltip/presentation.rs:45-79 et :251-351, placement.rs:61-72` | à examiner | | |
| O94 | `ui/components/top_bar.rs:62-68 et :20-30` | à examiner | | |
| O95 | `ui/audio_covers.rs:14-32 et :54-59` | à examiner | | |
| O96 | `ui/date_groups.rs:24-29` | à examiner | | |
| O97 | `ui/preview_disk.rs:152 et :166` | à examiner | | |
| O98 | `ui/i18n.rs:1787` | à examiner | | |
| O100 | `ui/workspace/preparation.rs:41, :37 et :65` | à examiner | | |
| O101 | `ui/workspace/filters.rs:180 et :320` | à examiner | | |
| O102 | `ui/workspace/filters/criteria.rs:87` | à examiner | | |
| O103 | `ui/theme/states.rs:118` | à examiner | | |
