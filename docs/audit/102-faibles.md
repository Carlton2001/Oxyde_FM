# Audit : suivi des faibles restants

Ouvert le 2026-11-20, à partir des catégories 4 (faibles Linux F45 à F69) et 5 (faibles non examinés) de `101-restes.md`. Le texte de chaque constat est dans l'annexe de `101-restes.md` ; ce fichier n'en garde que l'emplacement et le suivi.

Chaque constat est vérifié sur le code actuel avant d'être corrigé. Un commit par constat sur `audit-fixes`, sans push.

États : « à examiner », « corrigé », « obsolète » (déjà corrigé ou ne se produit plus), « écarté » (choix volontaire ou gain trop faible, raison en note), « reporté » (refonte ou décision, raison en note), « Linux » (attend une session Linux).

### Plateforme Linux et services communs

| ID | Emplacement | État | Commits | Note |
| --- | --- | --- | --- | --- |
| F45 | `platform/linux_file_clipboard.rs:82` | Linux | | |
| F46 | `platform/linux_file_clipboard/wayland.rs:42` | Linux | | |
| F47 | `platform/linux_download.rs:38` | Linux | | |
| F48 | `platform/linux_file_tree.rs:88 et :57` | Linux | | |
| F49 | `platform/linux_gio.rs:384, :391 et :486` | Linux | | |
| F50 | `platform/linux_gio/applications.rs:186` | Linux | | |
| F51 | `platform/linux_extended_attributes.rs:24` | Linux | | |
| F52 | `platform/hardware.rs:46, :80 et :146` | corrigé | e11d6088 | lettre de lecteur lue en ASCII (`drive_letter`) ; « é:\… » n'est plus un lecteur ; test |
| F53 | `platform/hardware.rs:79` | corrigé | a4654b0a | volume monté dans un dossier interrogé par son nom `Volume{…}` (numéro de disque et pénalité de recherche) ; repli sur un identifiant propre au volume ; à tester avec un disque monté dans un dossier |
| F54 | `platform/linux_permissions.rs:103` | Linux | | |
| F55 | `platform/linux_portal.rs:239 et :270` | Linux | | |
| F56 | `platform/linux_portal/chooser.rs:286` | Linux | | |
| F57 | `platform/linux_media/worker.rs:114` | Linux | | |
| F58 | `platform/linux_sandbox/seccomp.rs:38` | Linux | | |
| F59 | `platform/linux_volumes/holders.rs:35` | Linux | | |
| F60 | `platform/linux_trash.rs:72` | Linux | | |
| F61 | `platform/linux_trash.rs:33` | Linux | | |
| F62 | `platform/linux_quick_access.rs:35` | Linux | | |
| F63 | `platform/thumbnails.rs:116` | corrigé | 7f618735 | sans date de modification, miniature générée sans cache ; clé sans casse sous Windows (anciennes entrées orphelines, évincées par le quota) |
| F64 | `platform/thumbnails.rs:87` | écarté |  | choix de sécurité de c37f7b7b : le décodeur suit l'extension (test `cache_and_source_do_not_decode_a_different_format_than_the_extension`) ; un fichier sans extension d'image n'est pas demandé en miniature |
| F65 | `platform/thumbnails.rs:230` | Linux | | |
| F66 | `platform/thread_priority.rs:17` | Linux | | |
| F67 | `platform/viewer_media.rs:191` | à examiner | | |
| F68 | `platform/network.rs:112, :214-216, :236 et :181` | à examiner | | |
| F69 | `platform/network.rs:320` | à examiner | | |

### Archives, systèmes de fichiers, historique et opérations

| ID | Emplacement | État | Commits | Note |
| --- | --- | --- | --- | --- |
| F70 | `commands/archive_iso_open.rs:208` | à examiner | | |
| F71 | `commands/archive_iso_open.rs:181 et :195` | à examiner | | |
| F73 | `commands/archive.rs:239` | à examiner | | |
| F74 | `commands/archive.rs:143` | à examiner | | |
| F75 | `commands/archive_io_codec/parallel.rs:65` | à examiner | | |
| F77 | `systems/archive_open_cache.rs:516` | écarté | | choix du lot 10 : une copie modifiée par l'utilisateur n'est jamais évincée (catégorie 3) |
| F78 | `systems/archive_open_cache.rs:235` | à examiner | | |
| F79 | `commands/archive_rar.rs:145` | à examiner | | |
| F80 | `commands/archive_verify.rs:30, :51 et :65` | à examiner | | |
| F81 | `commands/archive_rar.rs:39` | à examiner | | |
| F82 | `systems/archive_io.rs:171` | à examiner | | |
| F83 | `systems/archive_owned_tree.rs:13` | à examiner | | |
| F84 | `utils/archive_codec.rs:85 et :87` | à examiner | | |
| F85 | `utils/archive.rs:112` | à examiner | | |
| F86 | `systems/archive_volumes.rs:27` | à examiner | | |
| F87 | `systems/archive_workspace.rs:315, :333 et :352` | à examiner | | |
| F88 | `utils/archive_listing_protocol.rs:242` | à examiner | | |
| F89 | `utils/archive_volumes.rs:58 et :44` | à examiner | | |
| F93 | `commands/duplicates.rs:341, :396 et :446` | à examiner | | |
| F95 | `commands/ops/conflicts.rs:54` | à examiner | | |
| F96 | `commands/ops/trash_listing.rs:53` | à examiner | | |
| F98 | `commands/ops.rs:482` | à examiner | | |
| F99 | `commands/image_handle_worker.rs:76-79` | à examiner | | |
| F104 | `systems/file_ops/copy.rs:273` | à examiner | | |
| F105 | `systems/file_ops/archive_transfer.rs:171` | à examiner | | |
| F107 | `systems/file_ops/copy.rs:561` | corrigé | c2132fc8 | Windows : type de stockage demandé par dossier source (avec F53) ; Linux inchangé (un stat par fichier, sans requête de stockage) |
| F109 | `systems/history_delete.rs:83-90 et :216-226` | à examiner | | |
| F110 | `systems/history_links.rs:33-45` | à examiner | | |
| F112 | `systems/file_ops/delete.rs:148-169 et :297-307` | à examiner | | |
| F113 | `systems/file_ops/scheduler.rs:236-273` | à examiner | | |
| F114 | `systems/folder_sizes/pools.rs:59-60` | à examiner | | |
| F118 | `systems/history_move.rs:57` | à examiner | | |

### Modèles, utilitaires et noyau

| ID | Emplacement | État | Commits | Note |
| --- | --- | --- | --- | --- |
| F124 | `commands/sidebar.rs:25-31, :45 et :120` | à examiner | | |
| F125 | `commands/sidebar.rs:63-79` | à examiner | | |
| F126 | `commands/system.rs:9-19` | à examiner | | |
| F127 | `commands/system.rs:33-53` | à examiner | | |
| F130 | `models/error.rs:131-143 et autres` | à examiner | | |
| F131 | `models/history.rs:142-148 et :170-183` | à examiner | | |
| F136 | `utils/office_document.rs:109-139` | à examiner | | |
| F138 | `utils/office_document.rs:252` | à examiner | | |
| F140 | `utils/office_document.rs:248-274` | à examiner | | |
| F141 | `utils/office_document.rs:50-57 et office_document/sheets.rs:76-81` | à examiner | | |
| F142 | `utils/office_preview.rs:21-24 et :41-47` | à examiner | | |
| F143 | `utils/office_document/search.rs:67-71` | à examiner | | |
| F144 | `utils/mod.rs:109-110` | à examiner | | |
| F146 | `utils/image_preview_decode.rs:76-81` | à examiner | | |
| F148 | `chemins non UTF-8 : utils/path_text.rs:29, commands/sidebar.rs:35-142, ui/app/file_chooser/navbar.rs:150 et browse.rs:140, commands/ops.rs:332-335 et :515-517, platform/linux_impl.rs:588 et :825, platform/linux_preview.rs:84, platform/windows_pdf_session.rs:42` | à examiner | | |

### Interface : application, fenêtres et composants

| ID | Emplacement | État | Commits | Note |
| --- | --- | --- | --- | --- |
| F149 | `ui/app/archive_password.rs:184` | à examiner | | |
| F150 | `ui/app/archive_password.rs:104` | à examiner | | |
| F151 | `ui/app/archive_password.rs:176 et :187` | à examiner | | |
| F152 | `ui/app/archive_password.rs:143` | à examiner | | |
| F153 | `ui/app/advanced_search/validation.rs:75` | à examiner | | |
| F154 | `ui/app/advanced_search/validation.rs:26` | à examiner | | |
| F155 | `ui/app/advanced_search/calendar.rs:36, :19 et :29-32` | à examiner | | |
| F156 | `ui/app/advanced_search/help.rs:171` | à examiner | | |
| F157 | `ui/app/clipboard.rs:18` | à examiner | | |
| F158 | `ui/app/context_menus.rs:305 et :331` | à examiner | | |
| F159 | `ui/app/context_menus.rs:206` | à examiner | | |
| F160 | `ui/app/context_menu_render/native.rs:229` | à examiner | | |
| F162 | `ui/app/drag.rs:548-559` | à examiner | | |
| F163 | `ui/app/drag.rs:51-55` | à examiner | | |
| F165 | `ui/app/duplicates/lifecycle.rs:126 et :129-130` | à examiner | | |
| F168 | `ui/app/drive_watchers.rs:468-517` | à examiner | | |
| F169 | `ui/app/drive_watchers.rs:170-176` | à examiner | | |
| F170 | `ui/app/disk_images.rs:53-80 et :108-111` | à examiner | | |
| F173 | `ui/app/file_chooser/browse.rs:68 et :70` | à examiner | | |
| F174 | `ui/app/file_chooser/location.rs:39` | à examiner | | |
| F175 | `ui/app/file_chooser.rs:368 et :387` | à examiner | | |
| F176 | `ui/app/file_chooser/create.rs:34, :53 et :63` | à examiner | | |
| F177 | `ui/app/file_chooser/band.rs:49` | à examiner | | |
| F178 | `ui/app/file_chooser/overwrite.rs:36` | à examiner | | |
| F179 | `ui/app/file_chooser/save.rs:222-229 et :92-94` | à examiner | | |
| F180 | `ui/app/file_operations.rs:268-278, :176-183 et :357` | à examiner | | |
| F181 | `ui/app/file_chooser/setting.rs:13-18 et :46-49, ui/app/folder_handler.rs:13-18 et :54-57` | à examiner | | |
| F182 | `ui/app/file_chooser/sidebar.rs:52-63` | à examiner | | |
| F183 | `ui/app/file_chooser/tree.rs:93-103` | à examiner | | |
| F184 | `ui/app/instance.rs:13-15 et :25-31` | à examiner | | |
| F188 | `ui/app/navigation.rs:54-56` | à examiner | | |
| F189 | `ui/app/navigation/breadcrumb.rs:129-154, :25-31 et :200-211` | à examiner | | |
| F190 | `ui/app/navigation.rs:393-416` | à examiner | | |
| F191 | `ui/app/layout/tabs.rs:645-651` | à examiner | | |
| F192 | `ui/app/open_with.rs:229-235` | à examiner | | |
| F193 | `ui/app/pinned_order.rs:76` | à examiner | | |
| F194 | `ui/app/network.rs:466-468` | à examiner | | |
| F195 | `ui/app/navigation.rs:700-716` | à examiner | | |
| F196 | `ui/app/properties/interactions.rs:148-173` | à examiner | | |
| F197 | `ui/app/properties/selection.rs:22-30` | à examiner | | |
| F198 | `ui/app/properties/window.rs:30-38` | à examiner | | |
| F200 | `ui/app/properties/interactions.rs:111` | à examiner | | |
| F201 | `ui/app/rename.rs:252-255` | à examiner | | |
| F202 | `ui/app/properties/permissions/save.rs:97-98 et properties/dialog.rs:52` | à examiner | | |
| F203 | `ui/app/selection.rs:55 et :354` | à examiner | | |
| F207 | `ui/app/theme_window/draft.rs:251 et :233, theme_window.rs:17` | à examiner | | |
| F209 | `ui/app/trash_settings/linux.rs:157 et :171` | Linux | | |
| F210 | `ui/app/trash_settings/settings.rs:82, :168, :229 et :281` | à examiner | | |
| F214 | `ui/app/viewer.rs:196 et viewer/markdown/loading.rs:13` | à examiner | | |
| F215 | `ui/app/viewer/fullscreen_controls.rs:154 et :207` | à examiner | | |
| F216 | `ui/app/viewer/markdown.rs:95` | à examiner | | |
| F217 | `ui/app/viewer/markdown/parse.rs:112 et :51-66` | à examiner | | |
| F218 | `ui/app/viewer/markdown/selection.rs:134-137 et viewer/text/selection.rs:141-143` | à examiner | | |
| F219 | `ui/app/viewer/media.rs:238-244` | à examiner | | |
| F220 | `ui/app/viewer/media_overlay.rs:409-479` | à examiner | | |
| F221 | `ui/app/viewer/tree.rs:230-243, :54, tree/loading.rs:75 et :155-157` | à examiner | | |
| F223 | `ui/components/conflict_dialog/advanced.rs:135-137 et frame.rs:187-189` | à examiner | | |
| F224 | `ui/components/file_list/icons.rs:281` | à examiner | | |
| F226 | `ui/components/file_list/presentation.rs:72 et :174` | à examiner | | |
| F227 | `ui/components/file_list/details.rs:329, grid.rs:433` | à examiner | | |
| F228 | `ui/components/file_list/icons.rs:411` | à examiner | | |
| F233 | `ui/components/input.rs:129-132` | à examiner | | |
| F235 | `ui/components/operations/items.rs:244 et :267` | à examiner | | |
| F236 | `ui/components/operations/view.rs:163` | à examiner | | |
| F237 | `ui/components/operations/items/icons.rs:14` | à examiner | | |
| F238 | `ui/components/popup/hover.rs:38` | à examiner | | |
| F239 | `ui/components/operations/list.rs:255` | à examiner | | |
| F241 | `ui/components/sidebar.rs:42` | à examiner | | |
| F242 | `ui/components/sidebar/rail.rs:168 et :226-232` | à examiner | | |
| F243 | `ui/components/sidebar/pinned.rs:62 et :143` | à examiner | | |
| F244 | `ui/components/tabs/scroll.rs:44` | à examiner | | |
| F245 | `ui/components/settings/fallback.rs:18` | à examiner | | |
| F246 | `ui/components/top_bar/breadcrumb_menu.rs:57` | à examiner | | |
| F248 | `ui/components/tooltip/hover.rs:176-186` | à examiner | | |
| F249 | `ui/components/tooltip.rs:28-32` | à examiner | | |
| F250 | `ui/components/tooltip/hover/native_window.rs:534-538` | à examiner | | |
| F251 | `ui/audio_covers.rs:25-29` | à examiner | | |
| F252 | `ui/date_groups/grouping.rs:178` | à examiner | | |
| F253 | `ui/date_groups/grouping.rs:121-123 et :141-147` | à examiner | | |
| F254 | `ui/format.rs:69-85, :155-160 et :12-25` | à examiner | | |
| F255 | `ui/errors.rs:134-142 et :48, :91` | à examiner | | |
| F257 | `ui/elements/button.rs:75-93` | à examiner | | |
| F258 | `ui/file_types.rs:6-273` | à examiner | | |
| F259 | `ui/i18n.rs:1797` | à examiner | | |
| F260 | `ui/paths.rs:11` | à examiner | | |
| F261 | `ui/style.rs:98` | à examiner | | |
| F262 | `ui/workspace/filters.rs:335 et :151` | à examiner | | |
| F263 | `ui/workspace/snapshots.rs:22, :82 et :52` | à examiner | | |
| F264 | `ui/workspace/operations.rs:6 et :64` | à examiner | | |
