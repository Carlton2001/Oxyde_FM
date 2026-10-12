# Refactorisation 11 — résultats

Réalisée le 12 octobre 2026, par un agent unique, sans délégation ni automatisation de l'interface. L'effort de lancement du chat n'a pas été modifié.

Branche : `codex/refactor-11-operations`.

Commit de code : `5d6bc705801fe1f784f5e470b6e5291c3037e001` — `Refactor operations panel into focused modules`.

## Périmètre et arborescence

Le fichier initial comptait **1 030 lignes physiques**, sans tests locaux (correction du comptage annoncé dans la première mise à jour). Les comptes ci-dessous incluent imports et lignes vides ; aucun fichier ne contient de tests.

```text
src/ui/components/
├── operations.rs          33 lignes — façade, état privé, dimensions, états finaux
└── operations/
    ├── view.rs           128 lignes — construction, abonnement, drag/scroll, Render
    ├── panel.rs          321 lignes — assemblage, événements, géométrie, liste/scrollbar
    ├── footer.rs          68 lignes — indicateur récapitulatif et ouverture manuelle
    └── card.rs           517 lignes — cartes, actions, vitesse/graphique, durée
```

Total production : **1 067 lignes physiques** ; tests dans ce périmètre : **0**. Les petits modules façade, vue et pied correspondent à des responsabilités complètes. Le graphique et le formatage restent privés aux cartes, sans découpage artificiel supplémentaire.

## Interfaces et invariants

- `operations::OperationsPanelView`, `PANEL_WIDTH`, `panel_width`, `render_operations_footer` et `render_operations_panel` conservent leurs chemins, signatures et visibilités externes. Les deux fonctions de rendu sont réexportées par la façade ; aucun appelant n'est modifié.
- Tous les champs restent privés dans la façade. La référence faible à `OxydeApp`, l'abonnement possédé, position/bounds, offset et horodatage de déplacement restent identiques. Les blocs `impl` sont déplacés dans un enfant du module.
- Seuls `update_scrollbar_drag` et `render_operation_card` passent de privés à `pub(super)` pour les appels entre modules frères. Les sous-modules restent privés ; aucun accès public supplémentaire.
- `is_final` reste dans la façade car il est partagé par pied, panneau et cartes. Les cartes ne dépendent pas du panneau ; aucune dépendance circulaire entre sections. Le Render de la vue appelle le point d'entrée réexporté du panneau comme avant.
- Les corps déplacés, captures, paramètres et ordre d'exécution sont conservés. Aucun changement de tâches, notifications, actions, état backend, persistance, géométrie, scroll, identifiants GPUI, textes, couleurs, formats, dimensions, icônes ou curseurs. Les deux curseurs de déplacement déjà présents sont conservés.
- Aucun test existant déplacé ou modifié ; aucun test artificiel ajouté.

## Vérifications

1. État Git initial relevé : `AGENTS.md` modifié et documents/scripts non suivis préexistants. Ils sont préservés et exclus des commits. Le fichier d'opérations était propre et non encore extrait.
2. Référence initiale : `cargo check` réussi, avec seulement des messages environnementaux de canonicalisation de chemins. Aucun test local à `operations.rs` découvert par inspection ; pas de comparaison exécutée des listes de tests avant/après, puisqu'aucun test n'est déplacé.
3. Extraction des cartes puis extraction des trois autres responsabilités : `cargo check` réussi à chaque étape hors sandbox. La recompilation dans le sandbox échouait dans `RustEmbed` sur la résolution absolue des assets (accès refusé) ; la même vérification hors sandbox réussit. Aucun rejet d'approbation.
4. Comparaison mécanique avec le fichier original de Git : les quatre blocs complets sont identiques après normalisation des fins de ligne et retrait des deux adaptations `pub(super)`. Revue du diff de façade, des imports, des réexportations et du diff indexé effectuée.
5. `cargo check` final : réussi, sans avertissement du projet.
6. `cargo test` : bibliothèque **644 réussis, 1 échec, 16 ignorés**, sur 661 tests. Échec : `commands::archive::archive_tests::archive_outputs_and_external_copies_preserve_internet_provenance`, `src/commands/archive.rs:882`, `ArchiveCache(Full)`. Il se reproduit avec le filtre exact (1 test effectivement exécuté). Cette bibliothèque n'inclut pas le module UI modifié et ses sources sont inchangées : l'échec est indépendant de l'extraction. Aucun nettoyage du cache ni changement backend effectué.
7. `cargo test --bin oxyde` : **196 réussis, 1 ignoré**, sur 197 tests.
8. `cargo test -- --skip commands::archive::archive_tests::archive_outputs_and_external_copies_preserve_internet_provenance` : terminé avec code 0. Tous les autres tests passent, dont **49 tests d'intégration** (20 pipeline, 14 worker archive IO, 6 listing worker, 9 preview worker). Les doc-tests découvrent 0 test : cela n'est pas présenté comme une couverture du panneau.
9. `cargo clippy --all-targets` : terminé avec code 0, avec avertissements dans des fichiers inchangés hors du périmètre. Aucun diagnostic sur `operations.rs` ou ses enfants. Les avertissements extérieurs ne sont pas corrigés ; pas de référence Clippy exécutée avant extraction.
10. `git diff --check` et `git diff --cached --check` : réussis. Index limité aux cinq fichiers Rust de la tâche avant le commit de code.

Validation technique sur Windows (`x86_64-pc-windows-msvc`) uniquement. Linux n'a pas été compilé ni exécuté dans cette tâche. Le rendu et les interactions n'ont pas été validés visuellement. Aucun blocage structurel nécessitant un effort supérieur rencontré. Aucun push ni réécriture de l'historique.

## Checklist manuelle — non exécutée

- [ ] Panneau vide, puis une et plusieurs opérations ; ordre et compteurs.
- [ ] Progression, vitesse, graphique/historique et durée.
- [ ] Pause, reprise, annulation et turbo sur fixtures ; disponibilité des boutons.
- [ ] États terminé, annulé et erreur ; dismissal individuel et global.
- [ ] Scroll, déplacement, limites de fenêtre et position restaurée.
- [ ] Pied récapitulatif ; ouverture/fermeture automatique selon les règles actuelles.
- [ ] Thèmes clairs et sombres, langues et plusieurs niveaux de zoom.
- [ ] Noms et chemins longs.
- [ ] Tab, Entrée, Espace, Échap et restitution du focus selon le comportement actuel.
- [ ] Absence de changement de curseur par rapport à la version initiale.
