# Vérification 1 : constats critiques et élevés

Rapports relus : WIN-SHELL-01, WIN-SHELL-02, WIN-FS-01, WIN-FS-02, WIN-WORKERS-01, WIN-WORKERS-02, DIALOG-01. Ils ne contiennent aucun constat critique. Huit constats sont classés élevés ; WIN-FS-01 et WIN-WORKERS-02 n'en ont aucun.

Bilan : 6 confirmés (dont 3 avec une gravité abaissée), 1 confirmé en partie, 1 incertain, aucun faux positif.

## WIN-SHELL-01

### `src/platform/windows_file_dialog/registration.rs:23` : DLL x64 inscrite sans tenir compte de l'architecture
- Verdict : confirmé. Gravité : élevé, maintenue.
- Justification : le code écrit `InprocServer32` sous `HKCU\Software\Classes\CLSID` sans aucun contrôle d'architecture. Le dépôt ne contient aucune détection ARM64, la publication ne produit qu'un setup x64, et l'installeur accepte `x64compatible`. Sous ARM64, les processus x64 émulés et ARM64 natifs partagent la même vue du registre, et COM ne revient pas à l'inscription machine quand la DLL de l'utilisateur ne se charge pas.
- Scénario : sur un PC Windows 11 ARM64, l'utilisateur active l'option. Le Bloc-notes, Paint ou Office ARM64 ne peuvent plus créer de boîte Ouvrir ni Enregistrer. Le constat ne concerne que les utilisateurs ARM64 qui activent cette option, et désactiver l'option suffit à rétablir les boîtes système.

## WIN-SHELL-02

### `src/platform/windows_shell_selection.rs:30` et `:71` : points et espaces finaux retirés avant `SHParseDisplayName`
- Verdict : confirmé. Gravité : abaissée à moyen.
- Justification : `validate_path` (`src/utils/path_security.rs`) retire les points et les espaces finaux du nom. Le menu contextuel Shell à élément unique (`windows_shell.rs:190` et `:355`), la sélection multiple et les propriétés passent tous par cette fonction, et rien ne refuse ensuite ces noms.
- Scénario : un dossier contient à la fois `notes` et `notes.`. Supprimer, renommer ou ouvrir les propriétés de `notes.` agit sur `notes`. La gravité est abaissée parce qu'il faut que les deux noms coexistent et que la suppression du Shell passe par défaut par la corbeille.

### `src/platform/windows_shell_delete_tracked.rs:298` : corbeille demandée sans `FOF_WANTNUKEWARNING`
- Verdict : confirmé. Gravité : élevé, maintenue.
- Justification : `operation_flags` (`windows_shell_delete.rs:6`) combine `FOF_ALLOWUNDO` et `FOF_NOCONFIRMATION`, sans `FOF_WANTNUKEWARNING`. `record_outcome` (`systems/history_delete.rs:118`) inscrit la racine dans l'historique dès que le reçu est complet, que la corbeille ait fourni une destination ou non. Si la configuration de corbeille du volume est illisible, `shell_delete_files_tracked` (`windows_impl.rs:604`) choisit le recyclage par défaut.
- Scénario : un fichier situé sur un partage réseau, sur une clé sans corbeille, ou plus gros que le quota de la corbeille est détruit définitivement sans aucune question. L'historique le présente comme recyclé, et l'annulation échoue.

## WIN-FS-02

### `src/platform/windows_no_reparse.rs:62` : sur Windows 11, pas de repli par composants après l'erreur 4395
- Verdict : incertain. Gravité si le défaut est avéré : élevé.
- Justification : le défaut de code est réel. La branche « liens de lecteur acceptés » ouvre le chemin en une seule recherche et n'a aucun repli, contrairement à la branche « inconnu ». En revanche, rien ne montre que Windows 11 refuse sous `OBJ_DONT_REPARSE` les liens d'un lecteur SUBST, d'un lecteur réseau mappé ou de `\??\UNC` alors qu'il accepte `\??\C:`. La sonde ne porte que sur le lecteur système, et aucun test ne couvre ces cas.
- À vérifier : sous Windows 11, faire une copie depuis un lecteur SUBST et depuis un lecteur réseau mappé.

### `src/platform/windows_no_reparse.rs:353` : tout objet en reparse refusé, quelle que soit son étiquette
- Verdict : confirmé en partie. Gravité : abaissée à moyen.
- Justification : le refus porte sur l'attribut 0x400, sans lire l'étiquette, et `OBJ_DONT_REPARSE` refuse aussi tout composant intermédiaire. `HeldParents::new` résout les jonctions par `canonicalize`, mais un volume monté dans un dossier reste un point de montage dans le chemin résolu, que la suite ouvre par `hold_parent`. Le cas OneDrive, ainsi que WOF et la déduplication, dépend de l'exposition de l'attribut aux processus ordinaires (mode de compatibilité des placeholders), ce qu'une lecture du code ne permet pas d'établir. Le dépôt ne traite les étiquettes nulle part.
- Scénario confirmé : un disque est monté dans `C:\Data`. Toute copie, tout déplacement, toute extraction ou toute opération de cache sous ce dossier échoue (erreur 4395 ou « Native lookup refuses a reparse object »).

## WIN-WORKERS-01

### `src/platform/windows_archive_security.rs:90` : `read_provenance` ne tolère que `NotFound`
- Verdict : confirmé. Gravité : élevé, maintenue.
- Justification : FAT32 et exFAT rejettent le nom de flux `fichier:Zone.Identifier` avec ERROR_INVALID_NAME (123), que Rust ne classe pas en `NotFound`. Tous les appelants propagent l'erreur par `?` : `extract.rs:137`, `commands/archive.rs:126` et `:391`, `archive_tar_open.rs:229`, `archive_seven_open.rs:271`, `archive_iso_open.rs:499`, `archive_workspace.rs` et `reclaim.rs`.
- Scénario : l'utilisateur extrait ou ouvre une archive posée sur une clé USB FAT32 ou exFAT. L'opération est refusée avant toute extraction.

### `src/platform/windows_archive_security.rs:191` : même défaut dans `read_output_provenance`
- Verdict : confirmé. Gravité : élevé, maintenue.
- Justification : `open_name` convertit STATUS_OBJECT_NAME_INVALID en erreur 123, et seul `NotFound` est toléré. Tous les appelants propagent l'erreur : `archive_io.rs:147` à chaque décodage, `archive_workspace.rs:169` et `:204`, `native_commit.rs:151` et `:158`, `recovery.rs:225`.
- Scénario : sur une clé FAT ou exFAT, toute extraction par le worker d'archives échoue, de même que l'ouverture d'un espace de travail d'archive.

## DIALOG-01

### `shell/oxyde-file-dialog/src/ask.rs:54-59`, `:68-69` et `:238` : pipes héritables pendant le lancement
- Verdict : confirmé. Gravité : abaissée à moyen.
- Justification : `CreatePipe` crée les deux bouts héritables. Le bout gardé ne perd l'héritage qu'à la ligne 59, et ceux du lanceur restent héritables jusqu'à leur fermeture à la fin de `launch`. La liste de handles ne protège que le `CreateProcessW` du lanceur, pas ceux qu'un autre thread de l'hôte lance au même moment. `reader.join()` n'a ensuite ni délai ni abandon.
- Scénario : l'application hôte lance un processus avec héritage pendant ces quelques millisecondes. Ce processus garde le bout écriture de la sortie, `read_all` n'atteint jamais EOF et le thread UI de l'hôte se fige tant que ce processus vit. La gravité est abaissée parce que la fenêtre de course est très courte.

<!-- LOT TERMINÉ -->
