# Audit WIN-SHELL-02

Résumé : 0 critique, 2 élevés, 6 moyens, 9 faibles, 2 optimisations.

## Fichiers lus en entier

- [x] `src/platform/windows_shell_delete_tracked.rs`
- [x] `src/platform/windows_shell_delete_tracked/batch.rs`
- [x] `src/platform/windows_shell_delete_tracked/effects.rs`
- [x] `src/platform/windows_shell_properties.rs`
- [x] `src/platform/windows_shell_selection.rs`
- [x] `src/platform/windows_shell_selection/controls.rs`
- [x] `src/platform/windows_shell_selection/thread.rs`
- [x] `src/platform/windows_shell_window.rs`
- [x] `src/platform/windows_shortcuts.rs`
- [x] `src/platform/windows_startup.rs`
- [x] `src/platform/windows_system_menu.rs`
- [x] `src/platform/windows_viewer_controls.rs`
- [x] `src/platform/windows_viewer_media.rs`
- [x] `src/platform/windows_viewer_media/info.rs`
- [x] `src/platform/windows_window_owner.rs`

## Constats

### `src/platform/windows_shell_selection.rs:30` (et `:71`)
- Gravité : élevé. Confiance : confirmé.
- Défaut : la sélection passe par `validate_path`, qui retire les points et espaces finaux du nom, puis `SHParseDisplayName` normalise à son tour : le menu contextuel Shell et les propriétés visent alors un autre élément que celui qui est sélectionné.
- Scénario : un dossier contient `notes` et `notes.` (créé par WSL, Git ou un chemin `\\?\`). Clic droit sur `notes.` puis Supprimer, Renommer ou Propriétés : l'action Shell s'applique à `notes`. Aucun refus des noms que l'espace de noms Shell ne sait pas adresser.

### `src/platform/windows_shell_delete_tracked.rs:298` (drapeaux, avec `:198-204`)
- Gravité : élevé. Confiance : plausible.
- Défaut : la corbeille est demandée avec `FOF_ALLOWUNDO | FOF_NOCONFIRMATION` sans `FOF_WANTNUKEWARNING`. Quand Windows ne peut pas recycler l'élément, il le supprime donc définitivement sans rien demander. Le reçu racine est quand même accepté (S_OK), et l'absence de destination dans la corbeille n'est jamais remontée à l'appelant : elle sert seulement à filtrer les effets.
- Scénario : mise à la corbeille sur un partage réseau, sur une clé USB sans corbeille ou d'un fichier plus gros que le quota de la corbeille. L'élément est détruit et l'historique le présente comme recyclé : l'annulation échoue. Seul `nuke_on_delete` est vérifié en amont.

### `src/platform/windows_shell_delete_tracked/effects.rs:18`
- Gravité : moyen. Confiance : plausible.
- Défaut : `key` retire `\\?\`, mais une racine `\\?\UNC\srv\part\…` devient `UNC\srv\part\…`, alors que le Shell rapporte `\\srv\part\…`. Le test de préfixe de `pre` (ligne 49) échoue donc pour tous les descendants.
- Scénario : suppression d'un dossier désigné par un chemin verbatim UNC, puis échec partiel (fichier verrouillé, `FOFX_EARLYFAILURE`). Les enfants déjà supprimés ne laissent aucun reçu : l'historique annonce que rien n'a été supprimé, alors qu'une partie de l'arborescence a disparu.

### `src/platform/windows_shell_delete_tracked.rs:131` (avec `:159-179`)
- Gravité : moyen. Confiance : plausible.
- Défaut : la vérification d'identité de la racine ne s'exécute que si `Compare(SICHINT_CANONICAL)` reconnaît l'élément du rappel comme la racine. Sinon, l'élément part dans `Effects::pre`, qui l'ignore (pas de préfixe enfant), et la suppression continue sans aucune preuve d'identité : la vérification échoue en position ouverte.
- Scénario : le moteur de copie rapporte la racine sous une autre forme (lecteur `subst`, lecteur mappé, chemin résolu par une jonction). Un fichier remplacé entre-temps est supprimé sans contrôle. Le résultat annonce ensuite un échec (« no completion receipt »), alors que l'élément a bien disparu.

### `src/platform/windows_shell_delete_tracked/effects.rs:58`
- Gravité : moyen. Confiance : plausible.
- Défaut : au-delà de 65 536 descendants, `pre` renvoie une erreur, le puits répond `E_ABORT`, et l'opération (avec `FOFX_EARLYFAILURE`) s'arrête au milieu de l'arborescence.
- Scénario : suppression définitive (lecteur réglé sur « ne pas utiliser la corbeille ») d'un `node_modules` de plus de 65 536 entrées. Le dossier reste à moitié supprimé et l'opération est signalée en échec. La même chose arrive si un seul descendant ne peut pas être ouvert pour sa preuve (lignes 63-74), par exemple une jonction refusée par ACL.

### `src/platform/windows_viewer_controls.rs:93`
- Gravité : moyen. Confiance : plausible.
- Défaut : la fenêtre GPUI des contrôles appartient à la surface vidéo d'un autre processus, le worker. Windows détruit les fenêtres possédées avec leur propriétaire, et `release_overlays` n'est appelé que sur une fermeture prévue (`MediaSession::drop`, fermeture de la Visionneuse).
- Scénario : le superviseur tue le worker (délai de 15 s, pression mémoire via `windows_resource_pressure`), ou un codec plante. La surface disparaît avec le processus et emporte la fenêtre de contrôles d'Oxyde, que GPUI croit encore vivante.

### `src/platform/windows_viewer_media.rs:535` (avec `info.rs:48`)
- Gravité : moyen. Confiance : plausible.
- Défaut : `info::read` rouvre le fichier de façon synchrone (`MFCreateSourceReaderFromURL`) sur le thread qui possède la surface et pompe ses messages. Cette surface appartient à la fenêtre d'Oxyde (propriétaire inter-processus, files d'entrée attachées). Pendant ce temps, ni messages ni battement de cœur.
- Scénario : vidéo sur un partage lent ou OneDrive à la demande. La lecture des en-têtes bloque plusieurs secondes : l'entrée de la fenêtre d'Oxyde peut geler et, au-delà de 15 s sans rapport, le superviseur tue le worker. La seconde ouverture complète est en plus redondante avec celle du moteur.

### `src/platform/windows_shortcuts.rs:127`
- Gravité : moyen. Confiance : plausible.
- Défaut : l'enregistrement appelle toujours `SetPath` avec la cible lue par `GetPath(SLGP_RAWPATH)`, même quand seule la description ou les arguments changent. La cible d'origine (liste d'identifiants, descripteur MSI annoncé) est remplacée par un simple chemin.
- Scénario : modifier la description d'un raccourci Office annoncé du menu Démarrer. `GetPath` renvoie l'icône dans `C:\Windows\Installer\{GUID}\`, et le raccourci enregistré lance désormais ce fichier d'icône. Un raccourci sans chemin de fichier (cible Shell pure) ne peut pas être modifié du tout, car sa cible est vide.

### `src/platform/windows_shell_delete_tracked/batch.rs:63`
- Gravité : faible. Confiance : confirmé.
- Défaut : un échec de préparation sur un seul fichier (littéral refusé, `SHCreateItemFromParsingName` sur un fichier disparu entre-temps, ligne 66) abandonne le lot entier avant `PerformOperations`. Tous les fichiers reçoivent la même erreur, et le motif réel est perdu (`E_ABORT`).
- Scénario : mise à la corbeille de 500 fichiers dont un vient d'être supprimé par une autre application : aucun n'est traité, et tous sont signalés en erreur.

### `src/platform/windows_shell_delete_tracked/batch.rs:78`
- Gravité : faible. Confiance : plausible.
- Défaut : contrairement au commentaire de la ligne 51, un `E_ABORT` renvoyé par le puits d'un seul élément (preuve d'identité périmée, échec de comparaison) annule, selon le contrat de `IFileOperationProgressSink`, toutes les opérations restantes du lot.
- Scénario : un des fichiers du lot a été remplacé depuis la sélection. Les fichiers suivants ne sont pas supprimés et sont signalés « aborted ».

### `src/platform/windows_viewer_media.rs:457`
- Gravité : faible. Confiance : plausible.
- Défaut : faute de `LOADEDMETADATA` dans les 15 s, le worker déclare un échec de lecture, quelle que soit la source.
- Scénario : grosse vidéo OneDrive non hydratée ou partage lent : la lecture échoue avec « System codecs could not play the media » alors que le fichier est valide.

### `src/platform/windows_viewer_controls.rs:50`
- Gravité : faible. Confiance : confirmé.
- Défaut : l'initialisation est repérée par `WS_EX_NOACTIVATE`, qui est posé avant `SetWindowSubclass` et `windows_pointer::install`. Si l'une de ces étapes échoue, les appels suivants considèrent la fenêtre comme prête et ne réessaient jamais.
- Scénario : un échec transitoire de `SetWindowSubclass` laisse les contrôles sans `MA_NOACTIVATE` ni gestion du pointeur pour toute la session.

### `src/platform/windows_shortcuts.rs:150`
- Gravité : faible. Confiance : plausible.
- Défaut : `IPersistFile::Save` réécrit le `.lnk` sur place, sans fichier temporaire ni remplacement atomique.
- Scénario : coupure ou disque plein pendant l'enregistrement : le raccourci est tronqué et illisible.

### `src/platform/windows_shell_selection/thread.rs:40`
- Gravité : faible. Confiance : plausible.
- Défaut : l'attente boucle sans limite tant que le compteur de référence du thread reste au-dessus de la base.
- Scénario : un gestionnaire de propriétés tiers garde la référence du thread (fuite). Le thread `spawn_blocking` et la tâche qui l'attend restent bloqués jusqu'à la fermeture d'Oxyde.

### `src/platform/windows_shell_selection/controls.rs:23`
- Gravité : faible. Confiance : plausible.
- Défaut : chaque ouverture des propriétés multiples écrit un manifeste temporaire dans `%TEMP%` pour activer les contrôles communs v6. Si l'écriture échoue, la fonction échoue entièrement.
- Scénario : `%TEMP%` plein, redirigé ou interdit en écriture par une stratégie : les propriétés d'une sélection multiple deviennent indisponibles.

### `src/platform/windows_shell_window.rs:298`
- Gravité : faible. Confiance : confirmé.
- Défaut : quand `Listed::new` échoue pour la première liste, son `Drop` a déjà détruit la fenêtre (`clock`), et `Standing::new` la détruit une seconde fois.
- Scénario : échec de `RegisterPending` ou `OnNavigate`. Le second `DestroyWindow` vise un handle libéré, qui peut avoir été réattribué entre-temps à une autre fenêtre du thread.

### `src/platform/windows_shell_window.rs:49` (avec `:200`)
- Gravité : faible. Confiance : plausible.
- Défaut : la sélection est close 20 ms après le dernier `SelectItem`, et les éléments arrivés plus tard sont perdus.
- Scénario : « Afficher dans le dossier » de plusieurs fichiers d'un dossier réseau ou d'un processus chargé, avec des appels espacés de plus de 20 ms : Oxyde ne sélectionne qu'une partie des éléments.

### `src/platform/windows_shell_delete_tracked/effects.rs:63`
- Gravité : optimisation. Confiance : plausible.
- Défaut : pour chaque descendant supprimé, `HeldParents::new` rouvre tous les dossiers ancêtres, puis le fichier lui-même, sur le thread de rappel du Shell.
- Scénario : suppression d'une arborescence profonde de dizaines de milliers de fichiers : le nombre d'ouvertures est de l'ordre de fichiers × profondeur, et le coût est payé fichier par fichier dans le moteur de suppression.

### `src/platform/windows_shell_window.rs:197`
- Gravité : optimisation. Confiance : confirmé.
- Défaut : le dédoublonnage par `Vec::contains` est quadratique en nombre d'éléments sélectionnés.
- Scénario : un programme demande d'afficher plusieurs milliers de fichiers d'un même dossier : chaque `SelectItem` parcourt toute la liste sur le thread qui répond au Shell.

<!-- LOT TERMINÉ -->
