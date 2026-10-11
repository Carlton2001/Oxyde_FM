# REFACTOR 8 — compte rendu

Branche : `codex/refactor-8-extract-tests` (départ : `master`).

Les sept fichiers ont été traités successivement. Les 17 blocs sont maintenant des modules enfants externes déclarés par `#[path]`, sans niveau logique supplémentaire. Corps conservés byte pour byte, indentation et fins de ligne comprises. Une reconstruction finale à partir des fichiers extraits correspond aux sources Git initiales après normalisation CRLF/LF. Les attributs `cfg`, les helpers, les accès privés et le code de production sont inchangés. Aucune macro dépendante du fichier source n'a été trouvée dans les blocs déplacés. Aucun bloc laissé en place.

Les modifications préexistantes de `AGENTS.md`, `UI_ARCHITECTURE.md`, `.codex/`, des documents REFACTOR et du script de comptage ont été préservées. Aucun commit créé.

## Arborescence

```text
src/commands/ops.rs
  src/commands/ops/tests/copy_redo_tests.rs
  src/commands/ops/tests/history_concurrency_tests.rs
  src/commands/ops/tests/move_undo_rollback_tests.rs
  src/commands/ops/tests/new_folder_history_tests.rs
  src/commands/ops/tests/restoration_selection_tests.rs
  src/commands/ops/tests/trash_restore_tests.rs
src/systems/history_file_undo.rs
  src/systems/history_file_undo/tests/tests.rs
src/commands/search.rs
  src/commands/search/tests/archive_search_tests.rs
  src/commands/search/tests/content_tests.rs
  src/commands/search/tests/quick_search_tests.rs
src/platform/thumbnails.rs
  src/platform/thumbnails/tests/office_thumbnail_security_tests.rs
  src/platform/thumbnails/tests/thumbnail_cache_write_tests.rs
  src/platform/thumbnails/tests/thumbnail_decode_security_tests.rs
  src/platform/thumbnails/tests/video_stream_tests.rs
src/systems/file_copy.rs
  src/systems/file_copy/tests/tests.rs
src/systems/history_move.rs
  src/systems/history_move/tests/tests.rs
src/systems/history_move_redo.rs
  src/systems/history_move_redo/tests/tests.rs
```

## Lignes et découverte

Comptage physique, commentaires et lignes vides compris. « Production » exclut les blocs de tests et leurs attributs ainsi que les helpers `#[cfg(test)]` hors blocs. « Tests » inclut ces helpers et les enveloppes/déclarations de modules. Le total des lignes de tests reste identique : les deux lignes `mod … {` / `}` sont remplacées par `#[path]` / `mod …;`. « Corps extraits » compte seulement les lignes présentes dans les nouveaux fichiers.

| Module | Fichier principal avant → après | Production avant → après | Tests avant → après | Corps extraits | Tests découverts avant → après |
| --- | ---: | ---: | ---: | ---: | ---: |
| src/commands/ops.rs | 2392 → 781 | 762 → 762 | 1630 → 1630 | 1611 | 33 → 33 |
| src/systems/history_file_undo.rs | 1977 → 730 | 727 → 727 | 1250 → 1250 | 1247 | 35 → 35 |
| src/commands/search.rs | 1575 → 777 | 760 → 760 | 815 → 815 | 798 | 22 → 22 |
| src/platform/thumbnails.rs | 1465 → 833 | 821 → 821 | 644 → 644 | 632 | 34 → 34 |
| src/systems/file_copy.rs | 1081 → 376 | 368 → 368 | 713 → 713 | 705 | 22 → 22 |
| src/systems/history_move.rs | 1003 → 294 | 263 → 263 | 740 → 740 | 709 | 16 → 16 |
| src/systems/history_move_redo.rs | 989 → 439 | 436 → 436 | 553 → 553 | 550 | 13 → 13 |

Les helpers `search_in_archive`, `remove_verified_output`, `move_file` et `move_with` sont restés à leur emplacement initial.

## Configuration et résultats

Validation native Windows : `x86_64-pc-windows-msvc`, fonctionnalités par défaut, profils dev/test du projet. Les sept modules appartiennent à `oxyde_lib` ; le binaire `oxyde` a aussi été vérifié.

- État initial : `cargo check` réussi ; découverte bibliothèque 656 tests, binaire 164 tests.
- Après chaque fichier : `cargo test --lib <chemin_module>:: -- --list`, comparaison exacte des noms complets et compilation ; `cargo test --lib <chemin_module>::`, examen mécanique et `git diff --check` ciblé.
- Résultats ciblés : ops 30 réussis / 3 ignorés ; history_file_undo 35 réussis ; search 22 réussis ; thumbnails 34 réussis ; file_copy 22 réussis ; history_move 15 réussis / 1 ignoré ; history_move_redo 13 réussis. Total : 171 réussis, 4 déjà ignorés, 175 découverts avant et après.
- Découverte globale finale : 656 noms de bibliothèque et 164 noms du binaire, listes strictement identiques aux listes initiales.
- `cargo check` final : réussi, aucun avertissement du projet.
- `cargo test` : 639 tests bibliothèque réussis, 16 ignorés, 1 échec préexistant : `commands::archive::archive_tests::archive_outputs_and_external_copies_preserve_internet_provenance`, erreur `ArchiveCache(Full)` à `src/commands/archive.rs:882`. Même résultat lors de la relance complète. Échec reproduit isolément puis avec les sept fichiers dans leur état Git initial ; les fichiers extraits ont ensuite été rétablis exactement. Aucun changement hors périmètre pour corriger cet échec.
- `cargo test --bin oxyde` : 163 réussis, 1 déjà ignoré.
- `cargo test --doc` : réussi, aucun doctest découvert.
- `cargo clippy` : réussi, 30 avertissements bibliothèque et 5 binaire. Comparaison avec les sources initiales : mêmes messages, tous dans le code de production préexistant.
- `git diff --check` : réussi.

Le sandbox a empêché certaines opérations de fichiers Windows et la lecture des ressources embarquées du binaire. Les validations concernées ont été relancées avec accès normal ; les tests ciblés passent. Les avertissements initiaux de canonisation/cache incrémental sont propres à cet environnement. Aucun test ignoré n'a été forcé. La cible Linux est installée mais n'a pas été compilée ni exécutée ; aucune validation Linux n'est revendiquée.

Les journaux de validation et les scripts mécaniques sont conservés hors du dépôt dans `C:/Users/Biluf/.codex/visualizations/2026/10/11/01a12b32-1521-74b3-a8fa-82750cf673be/refactor8-validation/`.

## Noms complets des tests concernés

Les listes ci-dessous sont identiques avant et après pour la configuration Windows ci-dessus, y compris les tests déjà ignorés.

### src/commands/ops.rs — 33 noms identiques avant/après

```text
commands::ops::copy_redo_tests::redo_copy_directory_cleanup_preserves_a_late_empty_replacement
commands::ops::copy_redo_tests::redo_copy_rejects_incomplete_or_changed_journals_and_can_retry
commands::ops::copy_redo_tests::redo_copy_rolls_back_earlier_outputs_when_a_later_source_changed
commands::ops::copy_redo_tests::redo_copy_rolls_back_new_files_when_a_later_destination_conflicts
commands::ops::copy_redo_tests::undo_copy_directory_error_preserves_outputs_and_allows_retry
commands::ops::copy_redo_tests::undo_copy_preserves_a_replacement_with_identical_contents
commands::ops::copy_redo_tests::undo_copy_removes_its_verified_output_without_shell_trash
commands::ops::history_concurrency_tests::concurrent_commands_do_not_touch_files_and_can_retry_after_release
commands::ops::history_concurrency_tests::io_error_keeps_history_and_allows_another_attempt
commands::ops::history_concurrency_tests::rename_history_keeps_missing_sources_retryable
commands::ops::history_concurrency_tests::rename_history_preserves_destinations_created_after_the_original_action
commands::ops::history_concurrency_tests::rename_history_preserves_replacements_before_undo_and_redo_and_can_retry
commands::ops::history_concurrency_tests::rename_history_requires_complete_paths_and_identity_before_stack_changes
commands::ops::move_undo_rollback_tests::directory_failure_keeps_files_and_history_for_retry
commands::ops::move_undo_rollback_tests::missing_destination_is_refused_before_history_changes
commands::ops::move_undo_rollback_tests::move_history_preserves_identical_replacements_before_undo_and_redo
commands::ops::move_undo_rollback_tests::move_history_without_identity_preserves_data_and_stack
commands::ops::move_undo_rollback_tests::redo_move_destination_conflict_keeps_history_and_can_retry
commands::ops::move_undo_rollback_tests::redo_move_roundtrips_nested_directory_history_twice
commands::ops::move_undo_rollback_tests::source_directory_cannot_be_replaced_by_a_file
commands::ops::new_folder_history_tests::new_folder_history_preserves_missing_sources_and_does_not_recreate_parents
commands::ops::new_folder_history_tests::new_folder_history_requires_a_proof_and_a_single_created_path
commands::ops::new_folder_history_tests::new_folder_history_roundtrips_and_tracks_the_immediate_rename
commands::ops::new_folder_history_tests::new_folder_redo_preserves_occupied_destinations_and_keeps_the_action_retryable
commands::ops::new_folder_history_tests::new_folder_rename_tracking_never_adopts_a_replacement_or_changes_a_reservation
commands::ops::new_folder_history_tests::new_folder_undo_preserves_later_children_and_can_retry
commands::ops::new_folder_history_tests::new_folder_undo_refuses_a_nonempty_replacement_without_recycling_it
commands::ops::restoration_selection_tests::restoration_refuses_ambiguous_or_missing_original_paths_before_effects
commands::ops::restoration_selection_tests::restoration_requires_the_complete_corrected_name_without_stem_guessing
commands::ops::restoration_selection_tests::restoration_selects_exact_ids_first_and_deduplicates_requests
commands::ops::trash_restore_tests::native_purge_only_removes_selected_trash_payload
commands::ops::trash_restore_tests::native_restore_conflict_preserves_existing_file_and_trash_item
commands::ops::trash_restore_tests::native_restore_preserves_original_names
```

### src/systems/history_file_undo.rs — 35 noms identiques avant/après

```text
systems::history_file_undo::tests::changed_partial_output_is_refused_before_any_file_moves
systems::history_file_undo::tests::copy_directory_error_and_unwind_restore_all_staged_outputs
systems::history_file_undo::tests::copy_directory_identity_is_checked_after_planning_before_staging
systems::history_file_undo::tests::copy_directory_without_identity_is_refused_before_any_effect
systems::history_file_undo::tests::copy_identity_is_rechecked_after_planning_before_the_native_move
systems::history_file_undo::tests::copy_missing_output_is_refused_before_staging_other_files
systems::history_file_undo::tests::copy_undo_preserves_a_directory_replaced_before_removal
systems::history_file_undo::tests::copy_undo_retains_recovery_without_writing_through_a_new_junction
systems::history_file_undo::tests::copy_undo_rollback_preserves_a_replacement_after_directory_restoration_fails
systems::history_file_undo::tests::copy_without_identity_is_refused_without_guessing_from_contents
systems::history_file_undo::tests::copy_writer_on_second_output_rolls_back_first_staged_file
systems::history_file_undo::tests::late_directory_error_restores_staged_output_removed_directory_and_files
systems::history_file_undo::tests::move_directory_independent_source_arrival_is_preserved_before_undo
systems::history_file_undo::tests::move_directory_missing_proof_is_refused_before_any_effect
systems::history_file_undo::tests::move_directory_replacement_after_planning_is_preserved_before_undo
systems::history_file_undo::tests::move_directory_replacement_before_undo_is_preserved_and_original_can_retry
systems::history_file_undo::tests::move_directory_restored_source_replacement_is_preserved_before_redo
systems::history_file_undo::tests::move_directory_undo_rollback_refreshes_proofs_for_retry
systems::history_file_undo::tests::move_partial_output_identity_is_required_before_any_undo_effect
systems::history_file_undo::tests::move_partial_output_is_rechecked_after_planning_and_completed_moves_roll_back
systems::history_file_undo::tests::move_undo_rechecks_identity_after_plan_before_publication
systems::history_file_undo::tests::move_undo_rollback_preserves_a_replacement_after_directory_restoration_fails
systems::history_file_undo::tests::partial_output_without_journal_hash_is_refused
systems::history_file_undo::tests::rollback_conflict_retains_staged_content_and_preserves_foreign_file
systems::history_file_undo::tests::rollback_preserves_a_replacement_for_a_directory_created_by_undo
systems::history_file_undo::tests::source_directory_error_removes_only_parents_created_by_undo
systems::history_file_undo::tests::successful_undo_keeps_foreign_directory_contents
systems::history_file_undo::tests::trash_sidecars_require_durable_identity_before_undo_and_redo
systems::history_file_undo::tests::undo_directory_creation_stays_in_the_initial_junction_parent
systems::history_file_undo::tests::undo_directory_removal_stays_in_the_initial_junction_parent
systems::history_file_undo::tests::undo_keeps_a_directory_absent_from_the_initial_capture
systems::history_file_undo::tests::undo_rollback_blocks_descendants_of_an_unrestored_parent
systems::history_file_undo::tests::undo_rollback_restores_files_outside_a_blocked_parent
systems::history_file_undo::tests::undo_unwind_preserves_replacements_and_staged_recovery
systems::history_file_undo::tests::unwind_rolls_back_all_completed_phases
```

### src/commands/search.rs — 22 noms identiques avant/après

```text
commands::search::archive_search_tests::archive_search_cancellation_is_not_reported_as_a_skipped_archive
commands::search::archive_search_tests::archive_search_warning_preserves_the_limit_kind_and_threshold
commands::search::archive_search_tests::archive_search_warns_and_keeps_results_after_multiple_skipped_archives
commands::search::archive_search_tests::iso_search_keeps_nested_paths_size_filters_and_cancellation
commands::search::archive_search_tests::iso_search_rejects_malformed_identifiers_incomplete_records_and_cycles
commands::search::archive_search_tests::seven_search_lists_header_names_without_constructing_a_content_decoder
commands::search::archive_search_tests::seven_search_preserves_size_filters_and_cancellation
commands::search::archive_search_tests::zip_tar_metadata_rejects_overflowing_tar_dates_without_panicking
commands::search::archive_search_tests::zip_tar_metadata_rejects_special_entries_in_search_and_navigation
commands::search::archive_search_tests::zip_tar_metadata_search_preserves_name_size_date_filters_and_cancellation
commands::search::archive_search_tests::zip_tar_metadata_search_rejects_unsafe_names_before_returning_results
commands::search::archive_search_tests::zip_tar_metadata_zip_names_do_not_require_a_supported_content_codec
commands::search::content_tests::office_cancellation_stops_during_decompression_and_keeps_normal_matching
commands::search::content_tests::text_cancellation_stops_inside_a_large_line_before_publishing_a_match
commands::search::content_tests::waiting_for_disk_lock_can_be_cancelled_without_releasing_the_lock
commands::search::quick_search_tests::advanced_filters_content_accents_size_and_recursion
commands::search::quick_search_tests::advanced_regex_case_dates_and_invalid_bounds
commands::search::quick_search_tests::advanced_searches_zip_entry_names
commands::search::quick_search_tests::advanced_stream_respects_limit_and_content_regex
commands::search::quick_search_tests::invalid_root_reports_failure
commands::search::quick_search_tests::recursive_names_are_case_insensitive_and_keep_actual_paths
commands::search::quick_search_tests::wildcard_limit_and_cancellation
```

### src/platform/thumbnails.rs — 34 noms identiques avant/après

```text
platform::thumbnails::office_thumbnail_security_tests::embedded_office_thumbnail_keeps_bgra_channels_for_native_ui
platform::thumbnails::office_thumbnail_security_tests::normal_images_decode_and_excessive_dimensions_are_rejected
platform::thumbnails::office_thumbnail_security_tests::oversized_compressed_payload_is_rejected_with_bounded_reads
platform::thumbnails::thumbnail_cache_write_tests::cache_junction_does_not_publish_into_its_target
platform::thumbnails::thumbnail_cache_write_tests::cache_junction_does_not_supply_a_thumbnail
platform::thumbnails::thumbnail_cache_write_tests::exclusive_creation_collision_preserves_the_existing_file
platform::thumbnails::thumbnail_cache_write_tests::failed_publication_cleans_only_the_created_staging_file
platform::thumbnails::thumbnail_cache_write_tests::failed_publication_still_returns_the_generated_thumbnail
platform::thumbnails::thumbnail_cache_write_tests::generation_failure_preserves_invalid_cache_until_replacement
platform::thumbnails::thumbnail_cache_write_tests::missing_source_metadata_still_reports_an_error
platform::thumbnails::thumbnail_cache_write_tests::normal_cache_hit_avoids_generation_and_preserves_pixels
platform::thumbnails::thumbnail_cache_write_tests::occupied_disk_quota_lease_keeps_the_generated_thumbnail_usable
platform::thumbnails::thumbnail_cache_write_tests::persistence_failure_keeps_the_generated_image
platform::thumbnails::thumbnail_cache_write_tests::unavailable_cache_directory_does_not_prevent_generation
platform::thumbnails::thumbnail_cache_write_tests::unavailable_cache_does_not_hide_a_generation_error
platform::thumbnails::thumbnail_decode_security_tests::cache_and_source_do_not_decode_a_different_format_than_the_extension
platform::thumbnails::thumbnail_decode_security_tests::cached_thumbnail_dimension_limit_accepts_boundary_and_rejects_next_pixel
platform::thumbnails::thumbnail_decode_security_tests::cached_thumbnail_encoded_limit_accepts_boundary_and_rejects_next_byte
platform::thumbnails::thumbnail_decode_security_tests::normal_source_and_cache_images_preserve_dimensions_and_pixels
platform::thumbnails::thumbnail_decode_security_tests::source_thumbnail_dimension_limit_accepts_boundary_and_rejects_next_pixel
platform::thumbnails::thumbnail_decode_security_tests::source_thumbnail_rejects_oversized_open_file_before_format_detection
platform::thumbnails::thumbnail_decode_security_tests::thumbnail_decode_rejects_output_over_allocation_budget
platform::thumbnails::thumbnail_decode_security_tests::thumbnail_reader_stops_after_encoded_limit_plus_one
platform::thumbnails::thumbnail_decode_security_tests::valid_thumbnail_reader_accepts_exact_budget_and_rejects_appended_byte
platform::thumbnails::video_stream_tests::an_empty_stream_ends_with_error_instead_of_reopening_forever
platform::thumbnails::video_stream_tests::cancellation_during_restart_pause_prevents_a_new_reader
platform::thumbnails::video_stream_tests::cancellation_stops_gaps_and_suppresses_a_frame_returned_after_it
platform::thumbnails::video_stream_tests::native_scaled_frames_keep_colors_alpha_padding_and_signed_stride
platform::thumbnails::video_stream_tests::native_video_layout_and_truncated_buffer_are_validated
platform::thumbnails::video_stream_tests::native_video_open_failure_is_reported
platform::thumbnails::video_stream_tests::only_natural_end_restarts_and_callback_refusal_stops_both_loops
platform::thumbnails::video_stream_tests::opening_and_read_errors_are_returned_without_restart
platform::thumbnails::video_stream_tests::video_pixels_are_bounded_and_signed_stride_is_respected
platform::thumbnails::video_stream_tests::video_timestamps_preserve_units_without_overflow
```

### src/systems/file_copy.rs — 22 noms identiques avant/après

```text
systems::file_copy::tests::active_copy_staging_resists_retargeting_and_is_cleaned_after_cancellation
systems::file_copy::tests::callback_panic_stops_native_copy_without_crossing_ffi
systems::file_copy::tests::cancellation_before_replacement_preserves_old_contents_without_publication
systems::file_copy::tests::cancellation_during_native_copy_removes_only_private_partial_output
systems::file_copy::tests::copy_keeps_source_and_publication_parents_fixed_during_progress
systems::file_copy::tests::copy_stays_in_the_selected_parent_after_junction_retargeting
systems::file_copy::tests::empty_readonly_copy_is_published_with_attributes
systems::file_copy::tests::hidden_overwrite_copies_streams_and_preserves_monotonic_main_progress
systems::file_copy::tests::identity_checked_cleanup_preserves_a_matching_replacement
systems::file_copy::tests::identity_cleanup_without_a_snapshot_preserves_the_output
systems::file_copy::tests::native_checkpoint_can_pause_and_source_writer_is_refused_until_resume
systems::file_copy::tests::publication_conflict_keeps_foreign_file_and_cleans_readonly_copy
systems::file_copy::tests::redo_copy_keeps_the_verified_source_when_replacement_is_attempted
systems::file_copy::tests::redo_copy_never_publishes_a_staged_hash_mismatch
systems::file_copy::tests::redo_copy_rejects_changed_sources_and_cancels_verification_before_staging
systems::file_copy::tests::redo_copy_stays_in_its_resolved_source_after_junction_retargeting
systems::file_copy::tests::staging_cleans_only_owned_output_and_preserves_unknown_children
systems::file_copy::tests::staging_is_never_created_in_a_new_reparse_target
systems::file_copy::tests::staging_keeps_unverified_ownership_without_adopting_a_file
systems::file_copy::tests::staging_preserves_unowned_outputs_after_a_native_creation_collision
systems::file_copy::tests::staging_refuses_same_content_replacements_for_publication_and_cleanup
systems::file_copy::tests::verified_copy_cleanup_preserves_readonly_foreign_hardlink_attributes
```

### src/systems/history_move.rs — 16 noms identiques avant/après

```text
systems::history_move::tests::changed_source_is_rejected_before_rename_or_copy
systems::history_move::tests::copy_fallback_publishes_verified_content_then_removes_source
systems::history_move::tests::failed_or_corrupted_copy_keeps_source_and_removes_staging
systems::history_move::tests::forced_history_copy_preserves_streams_hidden_attribute_and_modified_time
systems::history_move::tests::history_copy_keeps_initial_source_and_destination_after_retargeting
systems::history_move::tests::history_copy_refuses_a_new_destination_reparse_before_staging
systems::history_move::tests::history_move_keeps_the_initial_destination_after_junction_retargeting
systems::history_move::tests::history_readonly_cleanup_uses_the_published_identity_after_retargeting
systems::history_move::tests::identity_bound_copy_undo_refuses_a_cross_volume_fallback
systems::history_move::tests::move_journal_refreshes_identity_on_verified_cross_volume_publication
systems::history_move::tests::native_cross_volume_transfer_preserves_streams_and_modified_time
systems::history_move::tests::native_move_handle_blocks_writers_and_path_replacement
systems::history_move::tests::native_same_volume_move_preserves_identity_and_refuses_overwrite
systems::history_move::tests::publication_conflict_preserves_external_file_and_source
systems::history_move::tests::readonly_copy_cleanup_preserves_original_attributes
systems::history_move::tests::refused_source_deletion_removes_own_published_copy
```

### src/systems/history_move_redo.rs — 13 noms identiques avant/après

```text
systems::history_move_redo::tests::redo_move_cleanup_preserves_a_substituted_empty_directory
systems::history_move_redo::tests::redo_move_keeps_a_child_arriving_before_source_cleanup
systems::history_move_redo::tests::redo_move_readonly_source_is_preserved_without_clearing_permissions
systems::history_move_redo::tests::redo_move_rechecks_identity_after_plan_before_publication
systems::history_move_redo::tests::redo_move_recreates_nested_directories_below_a_canonical_root
systems::history_move_redo::tests::redo_move_refuses_a_directory_arriving_before_exclusive_creation
systems::history_move_redo::tests::redo_move_reports_incomplete_rollback_and_keeps_new_content
systems::history_move_redo::tests::redo_move_rollback_never_recovers_files_into_a_replacement_source_directory
systems::history_move_redo::tests::redo_move_rollback_preserves_an_output_changed_after_publication
systems::history_move_redo::tests::redo_move_rolls_back_removed_directories_before_files_and_preserves_hidden_attribute
systems::history_move_redo::tests::redo_move_source_cleanup_refuses_substitution_and_rolls_files_back_to_bound_source
systems::history_move_redo::tests::redo_move_unwind_cleans_only_its_empty_created_directories
systems::history_move_redo::tests::redo_move_uses_bound_destination_after_parent_junction_retarget
```
