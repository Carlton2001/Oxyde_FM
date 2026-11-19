# Plan d'audit complet d'Oxyde

Audit en lecture seule : aucun code n'est modifié. 692 fichiers et 155 116 lignes de production, répartis en 39 lots (17 en Opus pour 64 043 lignes, 22 en Sonnet pour 91 073 lignes).

Exclus : `vendor/`, `reference_zed/`, `target/`, `examples/`, les dossiers `tests/`, les catalogues `.ftl` et les ressources.

## Rôles

| Rôle | Type d'agent | Modèle | Effort | Tâches |
| --- | --- | --- | --- | --- |
| Orchestrateur | session principale | Sonnet 5.5 (`/model`) | high | Lancer les lots, suivre la couverture, déléguer la vérification, rédiger la synthèse. Ne lit jamais le code. |
| Audit Opus | `general-purpose` | opus | high | Lots WIN-*, DIALOG, ARCHIVES, FILEOPS, PLATFORM-01 à 03 (Linux) |
| Audit Sonnet | `general-purpose` | sonnet | high | Lots PLATFORM-04 (commun), CORE, UI-* |
| Vérification | `general-purpose` | opus | high | Constats critiques et élevés uniquement |

Ne jamais utiliser l'agent `Explore`, qui ne lit que des extraits.

## Prompt de lancement (à coller dans une nouvelle session)

```
Exécute l'audit décrit dans docs/audit/00-plan.md, en lecture seule, sans modifier aucun code.
- Tu es l'orchestrateur : ne lis jamais le code toi-même, seulement ce plan et les rapports.
- Pour chaque lot, saute-le si docs/audit/<LOT>.md existe et se termine par <!-- LOT TERMINÉ -->. Sinon, lance un agent general-purpose avec le modèle et l'effort indiqués, et le « Prompt d'audit » ci-dessous en remplaçant <LOT>, la liste de fichiers et le focus de sa famille.
- Lots Opus d'abord, dans l'ordre du plan. 3 agents en parallèle au maximum.
- Quand tous les lots sont terminés : phase de vérification, puis synthèse.
- Ne commite rien et ne pousse rien.
```

## Prompt d'audit (un agent par lot)

```
Audit en lecture seule du lot <LOT> du projet Oxyde (gestionnaire de fichiers natif en Rust, GPUI et Tokio, cibles Windows 10/11 et Linux). Ne modifie aucun fichier source.

Lis d'abord AGENTS.md, puis INTÉGRALEMENT chaque fichier de la liste, par grandes lectures (fichiers entiers). Hors de ta liste, ne fais que des recherches ciblées (Grep) pour suivre un appel ou une invariante ; n'ouvre pas d'autres fichiers en entier.

Fichiers :
<LISTE>

Cherche, dans cet ordre :
1. Bugs de logique : cas limites, erreurs avalées, unwrap/expect/indexation atteignables, débordements, conversions de chemins ou d'encodage (UTF-16, chemins longs, \?\ et UNC).
2. Concurrence : courses entre Tokio et GPUI, verrous tenus pendant un await, tâches ou spawn sans annulation, résultats périmés appliqués à un état qui a changé.
3. Travail bloquant ou I/O sur le thread UI.
4. Ressources : handles, processus, threads, COM ou tâches qui fuient, ordre d'arrêt.
5. Sécurité et intégrité des données : traversée de chemins, liens symboliques et points de reparse, écrasement ou suppression involontaire, opérations non atomiques.
6. Optimisations mesurables : allocations ou clones dans le rendu, I/O redondantes, tris ou parcours répétés, complexité quadratique.
<FOCUS>

Ignore le style et ce que Clippy détecte déjà. Ne signale que ce que tu peux justifier par le code.

Écris docs/audit/<LOT>.md, en français et concis, sans citer de code :
- En tête : liste des fichiers lus en entier, chacun coché.
- Puis un constat par entrée : `fichier:ligne`, gravité (critique, élevé, moyen, faible, optimisation), confiance (confirmé ou plausible), une phrase sur le défaut, le scénario concret de déclenchement.
- Dernière ligne, uniquement si tout est lu : <!-- LOT TERMINÉ -->

Réponds seulement avec le nombre de constats par gravité.
```

### Focus par famille (remplace `<FOCUS>`)

**WIN-\*, DIALOG**
```
7. Compatibilité selon la version de Windows (priorité, classe du bug Win10 LTSC erreur 4395) : API absentes ou différentes sur Windows 10 LTSC/IoT et anciennes builds, codes d'erreur Win32/HRESULT non prévus, GetLastError ignoré ou lu trop tard, flags de reparse et FILE_FLAG_OPEN_REPARSE_POINT, OneDrive et fichiers cloud, partages réseau, WSL, FAT/exFAT/ReFS, chemins longs. Ownership COM et durée de vie des pointeurs, appartement COM, fermeture des HANDLE, sécurité des appels unsafe.
DIALOG uniquement : la bibliothèque est chargée dans des processus tiers ; tout panic, blocage ou fuite y est critique.
```

**ARCHIVES**
```
7. Confinement : traversée de chemin (zip slip), liens dans les archives, noms invalides ou réservés sous Windows, bombes de décompression et limites, mots de passe, volumes multiples, archives corrompues ou tronquées, nettoyage des fichiers temporaires après annulation ou erreur. Comparer avec docs/architecture/archives.md et docs/security/status.md.
```

**FILEOPS**
```
7. Intégrité des données : perte possible lors d'une copie, d'un déplacement, d'une suppression, d'un renommage ou d'une annulation/rétablissement ; conflits de noms, différences de casse, destination incluse dans la source, interruption à mi-chemin, journal de staging, comportement entre volumes différents.
```

**PLATFORM (Linux et commun)**
```
7. Portabilité : différences entre environnements de bureau, portails et sandbox (Flatpak), GIO/GStreamer absents, permissions, attributs étendus, liens, systèmes de fichiers montés ; code commun qui suppose Windows ou Linux.
```

**CORE**
```
7. Protocoles des workers : validation des messages reçus, versions, tailles maximales, comportement si un worker plante ou se bloque ; persistance de la configuration (fichiers corrompus, migrations, écriture atomique).
```

**UI-\***
```
7. Cohérence d'état : sélection, onglets et historique après une modification du disque, résultats asynchrones arrivant pour un dossier quitté, fenêtres fermées pendant une tâche, saisie clavier et focus, index invalides après filtrage ou tri.
```

## Phase de vérification

Une fois tous les lots terminés, l'orchestrateur regroupe les constats critiques et élevés par lots de 10 environ et lance pour chaque groupe un agent `general-purpose`, modèle opus, effort high :

```
Vérifie en lecture seule chacun des constats suivants dans le code d'Oxyde. Pour chacun, lis uniquement les lignes concernées et ce qu'il faut pour suivre l'appel. Verdict : confirmé, faux positif ou incertain, avec une phrase de justification et la gravité corrigée si besoin. N'ouvre pas les fichiers en entier sans nécessité.
<CONSTATS>
Écris docs/audit/verification-<N>.md, terminé par <!-- LOT TERMINÉ -->.
```

## Synthèse

L'orchestrateur écrit `docs/audit/99-synthese.md` :
- en tête, nombre de constats confirmés par gravité et les 10 plus urgents ;
- puis les constats dédoublonnés et classés par gravité, faux positifs retirés ;
- les constats qui se recoupent d'un lot à l'autre, regroupés ;
- la liste des lots incomplets ou des fichiers non lus, qui doit être vide.

## Reprise après la limite de quota

Ouvre une nouvelle session et colle le prompt de lancement. Les lots déjà terminés sont sautés ; un rapport sans `<!-- LOT TERMINÉ -->` est refait.

## Lots

### WIN-SHELL-01 (Opus, 4363 lignes)

- `src/platform/windows_com.rs` (250)
- `src/platform/windows_executables.rs` (109)
- `src/platform/windows_explorer_browse.rs` (171)
- `src/platform/windows_file_dialog.rs` (279)
- `src/platform/windows_file_dialog/registration.rs` (116)
- `src/platform/windows_file_dialog/relay.rs` (141)
- `src/platform/windows_fixed_dialog.rs` (148)
- `src/platform/windows_folder_handler.rs` (286)
- `src/platform/windows_folder_handler/direct_launch.rs` (97)
- `src/platform/windows_folder_handler/menu.rs` (80)
- `src/platform/windows_folder_launcher.rs` (65)
- `src/platform/windows_hover_window.rs` (609)
- `src/platform/windows_hover_window/keyboard.rs` (103)
- `src/platform/windows_hover_window/pointer.rs` (115)
- `src/platform/windows_instance.rs` (345)
- `src/platform/windows_jump_list.rs` (146)
- `src/platform/windows_peek.rs` (150)
- `src/platform/windows_pointer.rs` (103)
- `src/platform/windows_process_launch.rs` (77)
- `src/platform/windows_shell.rs` (832)
- `src/platform/windows_shell_delete.rs` (141)

### WIN-SHELL-02 (Opus, 2793 lignes)

- `src/platform/windows_shell_delete_tracked.rs` (617)
- `src/platform/windows_shell_delete_tracked/batch.rs` (124)
- `src/platform/windows_shell_delete_tracked/effects.rs` (171)
- `src/platform/windows_shell_properties.rs` (80)
- `src/platform/windows_shell_selection.rs` (164)
- `src/platform/windows_shell_selection/controls.rs` (87)
- `src/platform/windows_shell_selection/thread.rs` (63)
- `src/platform/windows_shell_window.rs` (343)
- `src/platform/windows_shortcuts.rs` (224)
- `src/platform/windows_startup.rs` (62)
- `src/platform/windows_system_menu.rs` (20)
- `src/platform/windows_viewer_controls.rs` (185)
- `src/platform/windows_viewer_media.rs` (562)
- `src/platform/windows_viewer_media/info.rs` (67)
- `src/platform/windows_window_owner.rs` (24)

### WIN-FS-01 (Opus, 3793 lignes)

- `src/platform/windows_clipboard_write.rs` (537)
- `src/platform/windows_copy_staging.rs` (314)
- `src/platform/windows_disk_images.rs` (197)
- `src/platform/windows_disk_images/native.rs` (396)
- `src/platform/windows_drives.rs` (571)
- `src/platform/windows_eject_holders.rs` (141)
- `src/platform/windows_file_clipboard.rs` (25)
- `src/platform/windows_file_copy.rs` (160)
- `src/platform/windows_file_drag.rs` (391)
- `src/platform/windows_file_drag/data.rs` (186)
- `src/platform/windows_file_drag/target.rs` (198)
- `src/platform/windows_file_kind.rs` (29)
- `src/platform/windows_file_moves.rs` (280)
- `src/platform/windows_file_names.rs` (85)
- `src/platform/windows_file_tree.rs` (283)

### WIN-FS-02 (Opus, 3675 lignes)

- `src/platform/windows_impl.rs` (1364)
- `src/platform/windows_no_reparse.rs` (359)
- `src/platform/windows_onedrive.rs` (94)
- `src/platform/windows_quick_access.rs` (32)
- `src/platform/windows_reparse_tests.rs` (98)
- `src/platform/windows_resource_pressure.rs` (111)
- `src/platform/windows_source_selection.rs` (30)
- `src/platform/windows_system_paste.rs` (97)
- `src/platform/windows_trash.rs` (399)
- `src/platform/windows_trash/listing.rs` (95)
- `src/platform/windows_trash_clipboard.rs` (342)
- `src/platform/windows_trash_restore.rs` (547)
- `src/platform/windows_volumes.rs` (44)
- `src/platform/windows_wsl.rs` (63)

### WIN-WORKERS-01 (Opus, 4336 lignes)

- `src/platform/windows_archive_io_worker.rs` (495)
- `src/platform/windows_archive_io_worker/timing.rs` (118)
- `src/platform/windows_archive_io_worker/transport.rs` (124)
- `src/platform/windows_archive_security.rs` (359)
- `src/platform/windows_archive_source.rs` (88)
- `src/platform/windows_archive_worker.rs` (220)
- `src/platform/windows_broker_cancellation.rs` (192)
- `src/platform/windows_cache.rs` (52)
- `src/platform/windows_cache_proof.rs` (93)
- `src/platform/windows_cache_proof/key.rs` (158)
- `src/platform/windows_download.rs` (53)
- `src/platform/windows_icon_bitmap.rs` (365)
- `src/platform/windows_icon_cache.rs` (175)
- `src/platform/windows_icon_persistence.rs` (280)
- `src/platform/windows_icon_pixels.rs` (170)
- `src/platform/windows_icon_source.rs` (87)
- `src/platform/windows_office_search.rs` (83)
- `src/platform/windows_pdf_session.rs` (248)
- `src/platform/windows_pdf_session/previews.rs` (56)
- `src/platform/windows_pdf_thumbnail.rs` (341)
- `src/platform/windows_preview_cache.rs` (579)

### WIN-WORKERS-02 (Opus, 3159 lignes)

- `src/platform/windows_preview_quota.rs` (429)
- `src/platform/windows_preview_worker.rs` (666)
- `src/platform/windows_preview_worker/icon_session.rs` (364)
- `src/platform/windows_preview_worker/image_session.rs` (444)
- `src/platform/windows_thumbnail_bitmap.rs` (264)
- `src/platform/windows_worker_budget.rs` (222)
- `src/platform/windows_worker_sandbox.rs` (770)

### DIALOG-01 (Opus, 2144 lignes)

- `shell/oxyde-file-dialog/examples/fake_launcher.rs` (23)
- `shell/oxyde-file-dialog/examples/show_dialog.rs` (375)
- `shell/oxyde-file-dialog/src/ask.rs` (270)
- `shell/oxyde-file-dialog/src/controls.rs` (226)
- `shell/oxyde-file-dialog/src/dialog.rs` (710)
- `shell/oxyde-file-dialog/src/items.rs` (96)
- `shell/oxyde-file-dialog/src/lib.rs` (211)
- `shell/oxyde-file-dialog/src/request.rs` (233)

### ARCHIVES-01 (Opus, 4179 lignes)

- `src/bin/oxyde-archive-io-worker.rs` (7)
- `src/bin/oxyde-archive-list-worker.rs` (8)
- `src/commands/archive.rs` (672)
- `src/commands/archive_handle_worker.rs` (139)
- `src/commands/archive_io_codec.rs` (1168)
- `src/commands/archive_io_codec/encode.rs` (274)
- `src/commands/archive_io_codec/parallel.rs` (171)
- `src/commands/archive_io_worker.rs` (218)
- `src/commands/archive_iso_open.rs` (1043)
- `src/commands/archive_metadata.rs` (479)

### ARCHIVES-02 (Opus, 4461 lignes)

- `src/commands/archive_rar.rs` (361)
- `src/commands/archive_seven_open.rs` (977)
- `src/commands/archive_tar_open.rs` (833)
- `src/commands/archive_verify.rs` (79)
- `src/commands/archive_worker.rs` (73)
- `src/systems/archive_encode.rs` (290)
- `src/systems/archive_io.rs` (329)
- `src/systems/archive_move_sources.rs` (304)
- `src/systems/archive_open_cache.rs` (963)
- `src/systems/archive_open_cache/recovery.rs` (252)

### ARCHIVES-03 (Opus, 4254 lignes)

- `src/systems/archive_open_cache/recovery/reclaim.rs` (151)
- `src/systems/archive_owned_tree.rs` (747)
- `src/systems/archive_recovery.rs` (200)
- `src/systems/archive_volumes.rs` (127)
- `src/systems/archive_workspace.rs` (936)
- `src/systems/archive_workspace/native_commit.rs` (306)
- `src/utils/archive.rs` (144)
- `src/utils/archive_codec.rs` (219)
- `src/utils/archive_io_failure.rs` (94)
- `src/utils/archive_io_protocol.rs` (657)
- `src/utils/archive_io_state.rs` (303)
- `src/utils/archive_io_stream.rs` (370)

### ARCHIVES-04 (Opus, 1917 lignes)

- `src/utils/archive_limits.rs` (491)
- `src/utils/archive_listing_protocol.rs` (545)
- `src/utils/archive_mutation_test.rs` (35)
- `src/utils/archive_password.rs` (163)
- `src/utils/archive_security.rs` (162)
- `src/utils/archive_volumes.rs` (71)
- `src/utils/disk_image.rs` (64)
- `src/utils/tar_fidelity.rs` (386)

### FILEOPS-01 (Opus, 4413 lignes)

- `src/systems/compression_output.rs` (109)
- `src/systems/compression_sources.rs` (316)
- `src/systems/copied_source.rs` (200)
- `src/systems/copy_benchmark.rs` (235)
- `src/systems/copy_diagnostics.rs` (175)
- `src/systems/destination_tree.rs` (422)
- `src/systems/file_copy.rs` (426)
- `src/systems/file_ops.rs` (528)
- `src/systems/file_ops/archive_transfer.rs` (212)
- `src/systems/file_ops/compress.rs` (342)
- `src/systems/file_ops/copy.rs` (783)
- `src/systems/file_ops/copy/destinations.rs` (196)
- `src/systems/file_ops/copy/directories.rs` (40)
- `src/systems/file_ops/copy/links.rs` (71)
- `src/systems/file_ops/copy/whole_moves.rs` (55)
- `src/systems/file_ops/copy/worker.rs` (303)

### FILEOPS-02 (Opus, 4268 lignes)

- `src/systems/file_ops/delete.rs` (351)
- `src/systems/file_ops/delete/trash_batch.rs` (137)
- `src/systems/file_ops/execution.rs` (284)
- `src/systems/file_ops/extract.rs` (241)
- `src/systems/file_ops/item_outcomes.rs` (224)
- `src/systems/file_ops/overview.rs` (76)
- `src/systems/file_ops/scheduler.rs` (332)
- `src/systems/file_ops/summaries.rs` (117)
- `src/systems/folder_sizes.rs` (376)
- `src/systems/folder_sizes/pools.rs` (72)
- `src/systems/history_delete.rs` (650)
- `src/systems/history_directories.rs` (399)
- `src/systems/history_directory_attributes.rs` (52)
- `src/systems/history_file_undo.rs` (880)
- `src/systems/history_links.rs` (77)

### FILEOPS-03 (Opus, 3876 lignes)

- `src/systems/history_move.rs` (318)
- `src/systems/history_move_redo.rs` (510)
- `src/systems/history_renamed_dirs.rs` (64)
- `src/systems/item_rename.rs` (407)
- `src/systems/link_moves.rs` (61)
- `src/systems/mod.rs` (45)
- `src/systems/new_folder_history.rs` (201)
- `src/systems/path_parents.rs` (651)
- `src/systems/quick_access_watcher.rs` (63)
- `src/systems/source_directories.rs` (411)
- `src/systems/source_path_set.rs` (36)
- `src/systems/source_selection.rs` (677)
- `src/systems/staging_journal.rs` (173)
- `src/systems/transfer_naming.rs` (120)
- `src/systems/updates.rs` (139)

### PLATFORM-01 (Opus, 4270 lignes)

- `src/platform/built_binary.rs` (14)
- `src/platform/clipboard.rs` (354)
- `src/platform/file_clipboard.rs` (20)
- `src/platform/file_drag.rs` (38)
- `src/platform/file_names.rs` (40)
- `src/platform/hardware.rs` (202)
- `src/platform/icons.rs` (354)
- `src/platform/linux_access.rs` (26)
- `src/platform/linux_archive_io.rs` (239)
- `src/platform/linux_archive_list.rs` (112)
- `src/platform/linux_cache.rs` (93)
- `src/platform/linux_download.rs` (67)
- `src/platform/linux_extended_attributes.rs` (88)
- `src/platform/linux_file_clipboard.rs` (88)
- `src/platform/linux_file_clipboard/tests.rs` (29)
- `src/platform/linux_file_clipboard/wayland.rs` (73)
- `src/platform/linux_file_copy.rs` (121)
- `src/platform/linux_file_names.rs` (74)
- `src/platform/linux_file_tree.rs` (183)
- `src/platform/linux_gio.rs` (512)
- `src/platform/linux_gio/applications.rs` (338)
- `src/platform/linux_gio/library.rs` (89)
- `src/platform/linux_gio/servers.rs` (119)
- `src/platform/linux_gio/tests.rs` (162)
- `src/platform/linux_gpu.rs` (129)
- `src/platform/linux_gpu/compositor.rs` (54)
- `src/platform/linux_gstreamer.rs` (520)
- `src/platform/linux_gstreamer/library.rs` (132)

### PLATFORM-02 (Opus, 4459 lignes)

- `src/platform/linux_held.rs` (298)
- `src/platform/linux_hidden_list.rs` (30)
- `src/platform/linux_impl.rs` (957)
- `src/platform/linux_instance.rs` (261)
- `src/platform/linux_library.rs` (42)
- `src/platform/linux_links.rs` (101)
- `src/platform/linux_media.rs` (146)
- `src/platform/linux_media/worker.rs` (153)
- `src/platform/linux_mime.rs` (307)
- `src/platform/linux_mime/magic.rs` (150)
- `src/platform/linux_pdf.rs` (210)
- `src/platform/linux_pdf/library.rs` (31)
- `src/platform/linux_pdf/session.rs` (132)
- `src/platform/linux_permissions.rs` (196)
- `src/platform/linux_permissions/enclosed.rs` (130)
- `src/platform/linux_portal.rs` (325)
- `src/platform/linux_portal/chooser.rs` (480)
- `src/platform/linux_portal/conf.rs` (143)
- `src/platform/linux_portal/forward.rs` (60)
- `src/platform/linux_portal/guard.rs` (57)
- `src/platform/linux_portal/pending.rs` (116)
- `src/platform/linux_preview.rs` (134)

### PLATFORM-03 (Opus, 3683 lignes)

- `src/platform/linux_quick_access.rs` (44)
- `src/platform/linux_quick_access/bookmarks.rs` (85)
- `src/platform/linux_quick_access/tests.rs` (45)
- `src/platform/linux_rename.rs` (43)
- `src/platform/linux_sandbox.rs` (312)
- `src/platform/linux_sandbox/landlock.rs` (163)
- `src/platform/linux_sandbox/seccomp.rs` (133)
- `src/platform/linux_settings.rs` (50)
- `src/platform/linux_settings/tests.rs` (24)
- `src/platform/linux_signature.rs` (75)
- `src/platform/linux_source_selection.rs` (40)
- `src/platform/linux_startup.rs` (84)
- `src/platform/linux_terminal.rs` (67)
- `src/platform/linux_terminal/tests.rs` (27)
- `src/platform/linux_trash.rs` (405)
- `src/platform/linux_trash/transfer.rs` (85)
- `src/platform/linux_update.rs` (142)
- `src/platform/linux_video.rs` (113)
- `src/platform/linux_video/stream.rs` (85)
- `src/platform/linux_volumes.rs` (154)
- `src/platform/linux_volumes/actions.rs` (118)
- `src/platform/linux_volumes/disks.rs` (107)
- `src/platform/linux_volumes/fallback.rs` (116)
- `src/platform/linux_volumes/holders.rs` (54)
- `src/platform/linux_volumes/images.rs` (166)
- `src/platform/linux_volumes/inventory.rs` (177)
- `src/platform/linux_volumes/mounts.rs` (70)
- `src/platform/linux_volumes/signals.rs` (86)
- `src/platform/linux_volumes/udisks.rs` (212)
- `src/platform/linux_volumes/watch.rs` (61)
- `src/platform/linux_window_controls.rs` (21)
- `src/platform/linux_window_controls/tests.rs` (10)
- `src/platform/linux_worker.rs` (252)
- `src/platform/memory.rs` (57)

### PLATFORM-04 (Sonnet, 3243 lignes)

- `src/platform/mod.rs` (1371)
- `src/platform/network.rs` (420)
- `src/platform/quick_access.rs` (14)
- `src/platform/remote.rs` (66)
- `src/platform/thread_priority.rs` (50)
- `src/platform/thumbnails.rs` (814)
- `src/platform/video_thumbnail.rs` (78)
- `src/platform/viewer_media.rs` (198)
- `src/platform/volumes.rs` (81)
- `src/platform/worker_test_support.rs` (151)

### CORE-01 (Sonnet, 4408 lignes)

- `build.rs` (20)
- `src/app_paths.rs` (70)
- `src/bin/copy_benchmark.rs` (159)
- `src/bin/oxyde-folder-launcher.rs` (8)
- `src/bin/oxyde-image-worker.rs` (19)
- `src/commands/audio_handle_worker.rs` (45)
- `src/commands/duplicates.rs` (628)
- `src/commands/icon_session_worker.rs` (61)
- `src/commands/image_handle_worker.rs` (81)
- `src/commands/image_handle_worker/session.rs` (73)
- `src/commands/io.rs` (964)
- `src/commands/mod.rs` (35)
- `src/commands/office_handle_worker.rs` (74)
- `src/commands/ops.rs` (641)
- `src/commands/ops/conflicts.rs` (114)
- `src/commands/ops/conflicts/summaries.rs` (99)
- `src/commands/ops/linux_trash.rs` (191)
- `src/commands/ops/trash_listing.rs` (89)
- `src/commands/ops/trash_restore.rs` (78)
- `src/commands/search.rs` (565)
- `src/commands/search/content.rs` (265)
- `src/commands/search/content/normalize.rs` (129)

### CORE-02 (Sonnet, 4267 lignes)

- `src/commands/search/content/vm.rs` (178)
- `src/commands/search/criteria.rs` (120)
- `src/commands/sidebar.rs` (154)
- `src/commands/system.rs` (87)
- `src/lib.rs` (6)
- `src/main.rs` (321)
- `src/models/application.rs` (25)
- `src/models/config.rs` (491)
- `src/models/custom_theme.rs` (106)
- `src/models/error.rs` (672)
- `src/models/file_entry.rs` (325)
- `src/models/history.rs` (673)
- `src/models/mod.rs` (22)
- `src/models/permissions.rs` (234)
- `src/models/preferences.rs` (293)
- `src/models/transaction.rs` (195)
- `src/utils/audio_metadata.rs` (263)
- `src/utils/audio_metadata/tests.rs` (102)

### CORE-03 (Sonnet, 1879 lignes)

- `src/utils/image_preview_decode.rs` (288)
- `src/utils/mod.rs` (155)
- `src/utils/office_document.rs` (331)
- `src/utils/office_document/search.rs` (307)
- `src/utils/office_document/sheets.rs` (118)
- `src/utils/office_document/slides.rs` (84)
- `src/utils/office_preview.rs` (66)
- `src/utils/path_security.rs` (76)
- `src/utils/path_text.rs` (84)
- `src/utils/preview.rs` (55)
- `src/utils/preview_protocol.rs` (269)
- `src/utils/visibility.rs` (46)

### UI-APP-01 (Sonnet, 4116 lignes)

- `src/ui/app/advanced_search.rs` (63)
- `src/ui/app/advanced_search/calendar.rs` (139)
- `src/ui/app/advanced_search/help.rs` (271)
- `src/ui/app/advanced_search/render.rs` (398)
- `src/ui/app/advanced_search/settings.rs` (303)
- `src/ui/app/advanced_search/validation.rs` (105)
- `src/ui/app/advanced_search/window.rs` (303)
- `src/ui/app/archive_password.rs` (313)
- `src/ui/app/archive_protection.rs` (151)
- `src/ui/app/archives.rs` (223)
- `src/ui/app/calendar.rs` (49)
- `src/ui/app/cards.rs` (127)
- `src/ui/app/clipboard.rs` (101)
- `src/ui/app/context_menu_render/column_filter.rs` (378)
- `src/ui/app/context_menu_render/items.rs` (299)
- `src/ui/app/context_menu_render/mod.rs` (118)
- `src/ui/app/context_menu_render/native.rs` (335)
- `src/ui/app/context_menus.rs` (440)

### UI-APP-02 (Sonnet, 4494 lignes)

- `src/ui/app/dialog_windows.rs` (547)
- `src/ui/app/dialog_windows/fixed.rs` (137)
- `src/ui/app/dialog_windows/fixed/notice.rs` (195)
- `src/ui/app/disk_images.rs` (252)
- `src/ui/app/drag.rs` (568)
- `src/ui/app/drag/external.rs` (218)
- `src/ui/app/drag/native.rs` (289)
- `src/ui/app/drag/native/ghost.rs` (267)
- `src/ui/app/drag/outgoing.rs` (41)
- `src/ui/app/drag/popup.rs` (30)
- `src/ui/app/drag/volumes.rs` (42)
- `src/ui/app/drive_watchers.rs` (726)
- `src/ui/app/drive_watchers/changes.rs` (200)
- `src/ui/app/duplicates.rs` (57)
- `src/ui/app/duplicates/lifecycle.rs` (246)
- `src/ui/app/duplicates/presentation.rs` (89)
- `src/ui/app/duplicates/results.rs` (391)
- `src/ui/app/duplicates/scroll.rs` (64)
- `src/ui/app/duplicates/search.rs` (135)

### UI-APP-03 (Sonnet, 4296 lignes)

- `src/ui/app/duplicates/sources.rs` (229)
- `src/ui/app/duplicates/title_actions.rs` (57)
- `src/ui/app/duplicates/window.rs` (198)
- `src/ui/app/file_chooser.rs` (529)
- `src/ui/app/file_chooser/additions.rs` (158)
- `src/ui/app/file_chooser/arrange.rs` (143)
- `src/ui/app/file_chooser/band.rs` (176)
- `src/ui/app/file_chooser/browse.rs` (370)
- `src/ui/app/file_chooser/choices.rs` (108)
- `src/ui/app/file_chooser/create.rs` (222)
- `src/ui/app/file_chooser/crumbs.rs` (23)
- `src/ui/app/file_chooser/details.rs` (341)
- `src/ui/app/file_chooser/filter.rs` (152)
- `src/ui/app/file_chooser/footer.rs` (214)
- `src/ui/app/file_chooser/grid.rs` (218)
- `src/ui/app/file_chooser/keys.rs` (92)
- `src/ui/app/file_chooser/location.rs` (153)
- `src/ui/app/file_chooser/menus.rs` (154)
- `src/ui/app/file_chooser/navbar.rs` (476)
- `src/ui/app/file_chooser/overwrite.rs` (183)
- `src/ui/app/file_chooser/render.rs` (100)

### UI-APP-04 (Sonnet, 3770 lignes)

- `src/ui/app/file_chooser/save.rs` (305)
- `src/ui/app/file_chooser/scroll.rs` (111)
- `src/ui/app/file_chooser/selection.rs` (139)
- `src/ui/app/file_chooser/setting.rs` (56)
- `src/ui/app/file_chooser/sidebar.rs` (351)
- `src/ui/app/file_chooser/status.rs` (126)
- `src/ui/app/file_chooser/tree.rs` (217)
- `src/ui/app/file_chooser/types.rs` (30)
- `src/ui/app/file_operations.rs` (373)
- `src/ui/app/file_preparation.rs` (193)
- `src/ui/app/filters.rs` (109)
- `src/ui/app/filters/grid_menu.rs` (97)
- `src/ui/app/filters/panel.rs` (343)
- `src/ui/app/folder_handler.rs` (64)
- `src/ui/app/group_selection.rs` (94)
- `src/ui/app/grouping.rs` (81)
- `src/ui/app/instance.rs` (147)
- `src/ui/app/item_pointer.rs` (111)
- `src/ui/app/keyboard.rs` (121)
- `src/ui/app/keyboard_menu.rs` (88)
- `src/ui/app/layout.rs` (14)
- `src/ui/app/layout/geometry.rs` (61)
- `src/ui/app/layout/measurements.rs` (66)
- `src/ui/app/layout/pane.rs` (52)
- `src/ui/app/layout/scroll.rs` (127)
- `src/ui/app/layout/split_drop.rs` (294)

### UI-APP-05 (Sonnet, 4483 lignes)

- `src/ui/app/layout/tabs.rs` (848)
- `src/ui/app/layout/tree.rs` (130)
- `src/ui/app/main_window.rs` (170)
- `src/ui/app/mod.rs` (661)
- `src/ui/app/navigation.rs` (815)
- `src/ui/app/navigation/breadcrumb.rs` (215)
- `src/ui/app/navigation/trash_loading.rs` (105)
- `src/ui/app/network.rs` (689)
- `src/ui/app/open_with.rs` (266)
- `src/ui/app/pinned_order.rs` (86)
- `src/ui/app/preferences.rs` (498)

### UI-APP-06 (Sonnet, 4009 lignes)

- `src/ui/app/properties.rs` (230)
- `src/ui/app/properties/dialog.rs` (253)
- `src/ui/app/properties/general.rs` (304)
- `src/ui/app/properties/interactions.rs` (175)
- `src/ui/app/properties/loading.rs` (123)
- `src/ui/app/properties/permissions.rs` (475)
- `src/ui/app/properties/permissions/enclosed.rs` (50)
- `src/ui/app/properties/permissions/item.rs` (114)
- `src/ui/app/properties/permissions/save.rs` (118)
- `src/ui/app/properties/section.rs` (99)
- `src/ui/app/properties/selection.rs` (219)
- `src/ui/app/properties/shortcut.rs` (171)
- `src/ui/app/properties/tabs.rs` (50)
- `src/ui/app/properties/window.rs` (311)
- `src/ui/app/quit.rs` (94)
- `src/ui/app/remote.rs` (289)
- `src/ui/app/rename.rs` (636)
- `src/ui/app/render.rs` (298)

### UI-APP-07 (Sonnet, 4161 lignes)

- `src/ui/app/search.rs` (968)
- `src/ui/app/search/content_warnings.rs` (37)
- `src/ui/app/selection.rs` (387)
- `src/ui/app/shell_features.rs` (319)
- `src/ui/app/shutdown.rs` (90)
- `src/ui/app/sizes.rs` (300)
- `src/ui/app/system_menus.rs` (34)
- `src/ui/app/theme_window.rs` (153)
- `src/ui/app/theme_window/draft.rs` (274)
- `src/ui/app/theme_window/editor.rs` (144)
- `src/ui/app/theme_window/list.rs` (93)
- `src/ui/app/theme_window/render.rs` (177)
- `src/ui/app/toasts.rs` (400)
- `src/ui/app/tool_geometry.rs` (72)
- `src/ui/app/trash_settings.rs` (85)
- `src/ui/app/trash_settings/actions.rs` (137)
- `src/ui/app/trash_settings/confirmation.rs` (104)
- `src/ui/app/trash_settings/deletion_policy.rs` (91)
- `src/ui/app/trash_settings/linux.rs` (296)

### UI-APP-08 (Sonnet, 4372 lignes)

- `src/ui/app/trash_settings/linux/render.rs` (465)
- `src/ui/app/trash_settings/render.rs` (471)
- `src/ui/app/trash_settings/settings.rs` (292)
- `src/ui/app/type_select.rs` (89)
- `src/ui/app/updates.rs` (128)
- `src/ui/app/viewer.rs` (496)
- `src/ui/app/viewer/audio.rs` (270)
- `src/ui/app/viewer/autoscroll.rs` (117)
- `src/ui/app/viewer/content.rs` (219)
- `src/ui/app/viewer/content_render.rs` (291)
- `src/ui/app/viewer/deletion.rs` (140)
- `src/ui/app/viewer/display_options.rs` (113)
- `src/ui/app/viewer/fullscreen_controls.rs` (315)
- `src/ui/app/viewer/image_interaction.rs` (222)
- `src/ui/app/viewer/markdown.rs` (500)
- `src/ui/app/viewer/markdown/autolink.rs` (108)
- `src/ui/app/viewer/markdown/loading.rs` (136)

### UI-APP-09 (Sonnet, 4530 lignes)

- `src/ui/app/viewer/markdown/parse.rs` (512)
- `src/ui/app/viewer/markdown/selection.rs` (154)
- `src/ui/app/viewer/media.rs` (269)
- `src/ui/app/viewer/media_bar.rs` (128)
- `src/ui/app/viewer/media_labels.rs` (163)
- `src/ui/app/viewer/media_overlay.rs` (550)
- `src/ui/app/viewer/pdf.rs` (212)
- `src/ui/app/viewer/render.rs` (302)
- `src/ui/app/viewer/scrollbar.rs` (202)
- `src/ui/app/viewer/text.rs` (349)
- `src/ui/app/viewer/text/selection.rs` (153)
- `src/ui/app/viewer/text/syntax.rs` (175)
- `src/ui/app/viewer/tree.rs` (356)
- `src/ui/app/viewer/tree/loading.rs` (284)
- `src/ui/app/viewer/tree/render.rs` (329)
- `src/ui/app/viewer/video_layout.rs` (201)
- `src/ui/app/viewer/window.rs` (191)

### UI-COMP-01 (Sonnet, 4274 lignes)

- `src/ui/components/archive_password_dialog.rs` (105)
- `src/ui/components/backdrop.rs` (123)
- `src/ui/components/calendar.rs` (89)
- `src/ui/components/client_decorations.rs` (299)
- `src/ui/components/confirmation.rs` (94)
- `src/ui/components/conflict_dialog.rs` (155)
- `src/ui/components/conflict_dialog/advanced.rs` (324)
- `src/ui/components/conflict_dialog/comparison.rs` (284)
- `src/ui/components/conflict_dialog/controls.rs` (154)
- `src/ui/components/conflict_dialog/frame.rs` (209)
- `src/ui/components/conflict_dialog/rules.rs` (313)
- `src/ui/components/context_menu.rs` (401)
- `src/ui/components/context_menu/grid.rs` (141)
- `src/ui/components/context_menu/item.rs` (411)
- `src/ui/components/dialog.rs` (109)
- `src/ui/components/dropdown.rs` (716)
- `src/ui/components/file_list.rs` (67)
- `src/ui/components/file_list/card_body.rs` (280)

### UI-COMP-02 (Sonnet, 3888 lignes)

- `src/ui/components/file_list/details.rs` (627)
- `src/ui/components/file_list/details_row.rs` (416)
- `src/ui/components/file_list/filter_status.rs` (73)
- `src/ui/components/file_list/filters.rs` (188)
- `src/ui/components/file_list/grid.rs` (553)
- `src/ui/components/file_list/grid_card.rs` (320)
- `src/ui/components/file_list/grid_scroll.rs` (66)
- `src/ui/components/file_list/group_header.rs` (127)
- `src/ui/components/file_list/headers.rs` (210)
- `src/ui/components/file_list/presentation.rs` (321)
- `src/ui/components/file_list/scroll_top.rs` (43)
- `src/ui/components/file_list/selection_button.rs` (45)
- `src/ui/components/file_list/thumbnails.rs` (90)
- `src/ui/components/file_list/thumbnails/lifecycle.rs` (170)
- `src/ui/components/history_actions.rs` (51)
- `src/ui/components/icons.rs` (544)
- `src/ui/components/icons/colors.rs` (44)

### UI-COMP-03 (Sonnet, 4466 lignes)

- `src/ui/components/input.rs` (1190)
- `src/ui/components/input/completion_menu.rs` (136)
- `src/ui/components/input/completion_menu/scroll.rs` (121)
- `src/ui/components/input/field.rs` (197)
- `src/ui/components/input/options.rs` (21)
- `src/ui/components/input/render.rs` (107)
- `src/ui/components/input/selection.rs` (163)
- `src/ui/components/mod.rs` (38)
- `src/ui/components/modal.rs` (186)
- `src/ui/components/network.rs` (141)
- `src/ui/components/network_dialog.rs` (584)
- `src/ui/components/network_dialog/letters.rs` (175)
- `src/ui/components/open_with_dialog.rs` (220)
- `src/ui/components/operations.rs` (45)
- `src/ui/components/operations/compact.rs` (96)
- `src/ui/components/operations/detail.rs` (199)
- `src/ui/components/operations/detail/controls.rs` (107)
- `src/ui/components/operations/detail/graph.rs` (127)
- `src/ui/components/operations/detail/metrics.rs` (191)
- `src/ui/components/operations/footer.rs` (81)
- `src/ui/components/operations/items.rs` (341)

### UI-COMP-04 (Sonnet, 4292 lignes)

- `src/ui/components/operations/items/icons.rs` (39)
- `src/ui/components/operations/items/outcome.rs` (56)
- `src/ui/components/operations/items/table.rs` (358)
- `src/ui/components/operations/lifecycle.rs` (94)
- `src/ui/components/operations/list.rs` (261)
- `src/ui/components/operations/view.rs` (239)
- `src/ui/components/popup/dropdown.rs` (67)
- `src/ui/components/popup/hover.rs` (61)
- `src/ui/components/popup/keyboard.rs` (160)
- `src/ui/components/popup/mod.rs` (224)
- `src/ui/components/popup/wayland.rs` (106)
- `src/ui/components/popup/window.rs` (863)
- `src/ui/components/popup/window/frame.rs` (99)
- `src/ui/components/quit_confirmation.rs` (79)
- `src/ui/components/remote_login_dialog.rs` (235)
- `src/ui/components/rename_dialog.rs` (284)
- `src/ui/components/scroll_area.rs` (110)
- `src/ui/components/scrollbar.rs` (370)
- `src/ui/components/scrollbar/scroll_top.rs` (60)
- `src/ui/components/select_menu.rs` (117)
- `src/ui/components/settings.rs` (74)
- `src/ui/components/settings/about.rs` (107)
- `src/ui/components/settings/columns.rs` (179)
- `src/ui/components/settings/compression.rs` (50)

### UI-COMP-05 (Sonnet, 4322 lignes)

- `src/ui/components/settings/controls.rs` (267)
- `src/ui/components/settings/data_actions.rs` (87)
- `src/ui/components/settings/default_format.rs` (145)
- `src/ui/components/settings/fallback.rs` (48)
- `src/ui/components/settings/integration.rs` (93)
- `src/ui/components/settings/measure.rs` (114)
- `src/ui/components/settings/panel.rs` (80)
- `src/ui/components/settings/submenu.rs` (82)
- `src/ui/components/sidebar.rs` (75)
- `src/ui/components/sidebar/folder_row.rs` (79)
- `src/ui/components/sidebar/linux_row.rs` (108)
- `src/ui/components/sidebar/measure.rs` (54)
- `src/ui/components/sidebar/menus.rs` (398)
- `src/ui/components/sidebar/pinned.rs` (163)
- `src/ui/components/sidebar/rail.rs` (320)
- `src/ui/components/sidebar/reorder.rs` (116)
- `src/ui/components/sidebar/rows.rs` (443)
- `src/ui/components/sidebar/toolbar.rs` (118)
- `src/ui/components/sidebar/tree.rs` (293)
- `src/ui/components/sidebar/volume_row.rs` (67)
- `src/ui/components/tabs/mod.rs` (6)
- `src/ui/components/tabs/scroll.rs` (119)
- `src/ui/components/tabs/tab.rs` (335)
- `src/ui/components/tabs/tab_bar.rs` (217)
- `src/ui/components/title_bar.rs` (217)
- `src/ui/components/tool_window.rs` (278)

### UI-COMP-06 (Sonnet, 4957 lignes)

- `src/ui/components/tooltip.rs` (287)
- `src/ui/components/tooltip/assets.rs` (226)
- `src/ui/components/tooltip/hover.rs` (451)
- `src/ui/components/tooltip/hover/anchored.rs` (80)
- `src/ui/components/tooltip/hover/handover.rs` (119)
- `src/ui/components/tooltip/hover/native_window.rs` (660)
- `src/ui/components/tooltip/hover/scope.rs` (63)
- `src/ui/components/tooltip/image_cache.rs` (172)
- `src/ui/components/tooltip/native.rs` (23)
- `src/ui/components/tooltip/pdf_cache.rs` (158)
- `src/ui/components/tooltip/pixels.rs` (53)
- `src/ui/components/tooltip/placement.rs` (169)
- `src/ui/components/tooltip/position.rs` (173)
- `src/ui/components/tooltip/presentation.rs` (493)
- `src/ui/components/tooltip/preview.rs` (500)
- `src/ui/components/tooltip/video.rs` (114)
- `src/ui/components/tooltip/viewer.rs` (64)
- `src/ui/components/top_bar.rs` (660)
- `src/ui/components/top_bar/breadcrumb_fades.rs` (67)
- `src/ui/components/top_bar/breadcrumb_glide.rs` (65)
- `src/ui/components/top_bar/breadcrumb_menu.rs` (83)
- `src/ui/components/window_controls.rs` (240)
- `src/ui/components/window_controls/tests.rs` (37)

### UI-REST-01 (Sonnet, 3561 lignes)

- `src/ui/assets.rs` (89)
- `src/ui/audio_covers.rs` (170)
- `src/ui/audio_covers/tests.rs` (81)
- `src/ui/date_groups.rs` (280)
- `src/ui/date_groups/families.rs` (85)
- `src/ui/date_groups/grouping.rs` (202)
- `src/ui/elements/bar_button.rs` (65)
- `src/ui/elements/button.rs` (112)
- `src/ui/elements/button/style.rs` (69)
- `src/ui/elements/checkbox.rs` (21)
- `src/ui/elements/color_picker.rs` (249)
- `src/ui/elements/focus_ring.rs` (34)
- `src/ui/elements/mod.rs` (15)
- `src/ui/elements/motion.rs` (39)
- `src/ui/elements/overlay.rs` (48)
- `src/ui/elements/progress_bar.rs` (74)
- `src/ui/elements/radio.rs` (17)
- `src/ui/elements/resize_separator.rs` (119)
- `src/ui/elements/rounded_fill.rs` (39)
- `src/ui/elements/segmented.rs` (90)
- `src/ui/elements/separator.rs` (22)
- `src/ui/elements/stepper.rs` (156)
- `src/ui/elements/toggle.rs` (76)
- `src/ui/errors.rs` (443)
- `src/ui/file_types.rs` (293)
- `src/ui/format.rs` (638)
- `src/ui/format/permissions.rs` (35)

### UI-REST-02 (Sonnet, 4499 lignes)

- `src/ui/i18n.rs` (2943)
- `src/ui/image_preview.rs` (201)
- `src/ui/image_preview/svg.rs` (74)
- `src/ui/memory_diagnostics.rs` (146)
- `src/ui/mod.rs` (34)
- `src/ui/paths.rs` (38)
- `src/ui/preview_budget.rs` (70)
- `src/ui/preview_disk.rs` (271)
- `src/ui/preview_disk/audio_refs.rs` (30)
- `src/ui/preview_jobs.rs` (206)
- `src/ui/preview_worker.rs` (159)
- `src/ui/scale.rs` (67)
- `src/ui/style.rs` (260)

### UI-REST-03 (Sonnet, 4786 lignes)

- `src/ui/style/cards.rs` (137)
- `src/ui/theme.rs` (191)
- `src/ui/theme/file_colors.rs` (196)
- `src/ui/theme/legibility.rs` (51)
- `src/ui/theme/seed.rs` (140)
- `src/ui/theme/states.rs` (161)
- `src/ui/window_geometry.rs` (173)
- `src/ui/workspace.rs` (317)
- `src/ui/workspace/filters.rs` (398)
- `src/ui/workspace/filters/calendar_refresh_tests.rs` (214)
- `src/ui/workspace/filters/criteria.rs` (111)
- `src/ui/workspace/filters/tests.rs` (405)
- `src/ui/workspace/folder_sizes.rs` (371)
- `src/ui/workspace/operations.rs` (452)
- `src/ui/workspace/preparation.rs` (187)
- `src/ui/workspace/presentation.rs` (82)
- `src/ui/workspace/sidebar.rs` (111)
- `src/ui/workspace/snapshots.rs` (169)
- `src/ui/workspace/sorting.rs` (139)
- `src/ui/workspace/tab.rs` (195)
- `src/ui/workspace/tab/prepared_sort_tests.rs` (194)
- `src/ui/workspace/tab/scroll.rs` (77)
- `src/ui/workspace/tree.rs` (229)
- `src/ui/workspace/viewer.rs` (86)

