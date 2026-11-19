# Audit WIN-FS-02

Résumé : 22 constats. 0 critique, 2 élevés, 6 moyens, 11 faibles, 3 optimisations. Les deux élevés touchent la recherche native sans reparse (`windows_no_reparse.rs`), qui appartient à la classe du bug 4395.

## Fichiers lus en entier

- [x] `src/platform/windows_impl.rs`
- [x] `src/platform/windows_no_reparse.rs`
- [x] `src/platform/windows_onedrive.rs`
- [x] `src/platform/windows_quick_access.rs`
- [x] `src/platform/windows_reparse_tests.rs`
- [x] `src/platform/windows_resource_pressure.rs`
- [x] `src/platform/windows_source_selection.rs`
- [x] `src/platform/windows_system_paste.rs`
- [x] `src/platform/windows_trash.rs`
- [x] `src/platform/windows_trash/listing.rs`
- [x] `src/platform/windows_trash_clipboard.rs`
- [x] `src/platform/windows_trash_restore.rs`
- [x] `src/platform/windows_volumes.rs`
- [x] `src/platform/windows_wsl.rs`

## Constats

### `src/platform/windows_no_reparse.rs:62`
- Gravité : élevé. Confiance : plausible.
- Défaut : le comportement du noyau est sondé une seule fois, sur la racine du lecteur système. Quand il vaut « liens de lecteur acceptés », l'ouverture se fait en une seule recherche avec OBJ_DONT_REPARSE et, contrairement à la branche « inconnu », sans repli sur le parcours composant par composant si l'erreur 4395 apparaît.
- Scénario : sous Windows 11, un lecteur SUBST, un lecteur réseau mappé dans la session (`\??\Z:` vers le redirecteur), le lien `\??\UNC` ou un chemin `\\?\Volume{…}` dont le lien d'objet est refusé autrement que `\??\C:`. Toutes les opérations (copie, cache, archives) échouent avec l'erreur 4395, alors que le parcours par composants aurait réussi.

### `src/platform/windows_no_reparse.rs:353`
- Gravité : élevé. Confiance : plausible.
- Défaut : tout objet final qui porte FILE_ATTRIBUTE_REPARSE_POINT est refusé, quel que soit son tag. De même, OBJ_DONT_REPARSE (ligne 310) refuse tout composant intermédiaire en reparse. Les tags qui ne sont pas des liens (Cloud Files/OneDrive, WOF/CompactOS, déduplication) et les volumes montés dans un dossier NTFS sont donc traités comme des liens.
- Scénario : copier, déplacer ou restaurer un fichier OneDrive « à la demande », ou travailler sous `C:\Data` où un disque est monté dans un dossier. L'opération échoue avec « Native lookup refuses a reparse object » ou l'erreur 4395, alors que l'Explorateur réussit.

### `src/platform/windows_no_reparse.rs:310`
- Gravité : faible. Confiance : plausible.
- Défaut : rien ne vérifie que le noyau connaît OBJ_DONT_REPARSE. La sonde ne distingue que « 4395 » et « autre erreur ».
- Scénario : sur une build ancienne qui rejette l'attribut (paramètre invalide), la sonde renvoie « inconnu » et chaque ouverture échoue sans repli. Si le drapeau est ignoré en silence, les jonctions intermédiaires sont suivies sans détection.

### `src/platform/windows_impl.rs:1019`
- Gravité : moyen. Confiance : confirmé.
- Défaut : la taille du tampon de la première lecture du registre (MaxCapacity) est réutilisée pour la seconde (NukeOnDelete) sur un tampon de 4 octets, et le type de valeur n'est jamais vérifié.
- Scénario : un outil écrit MaxCapacity en QWORD ou en chaîne. La taille passe à 8 octets ou plus et la lecture suivante peut écrire au-delà du DWORD sur la pile. Avec une valeur NukeOnDelete de type chaîne, « 1 » est lu comme 0x31 et la suppression définitive n'est pas détectée.

### `src/platform/windows_impl.rs:959`
- Gravité : moyen. Confiance : plausible.
- Défaut : la politique de suppression n'est lue que dans `HKCU\…\BitBucket\Volume`. Les stratégies NoRecycleFiles et RecycleBinSize (HKLM ou HKCU Policies) sont ignorées.
- Scénario : sur un poste géré en entreprise avec « ne pas déplacer vers la Corbeille », Oxyde croit recycler et enregistre une suppression annulable, alors que le Shell supprime définitivement. L'annulation échoue ensuite.

### `src/platform/windows_impl.rs:546`
- Gravité : optimisation. Confiance : confirmé.
- Défaut : la suppression en lot relit la configuration de Corbeille du volume (GetVolumeNameForVolumeMountPointW, puis ouverture et deux lectures du registre) pour chaque chemin, sans cache par racine, alors que `shell_delete_files_tracked` en a un.
- Scénario : supprimer 10 000 fichiers d'un même dossier fait 10 000 résolutions de volume et 30 000 appels au registre avant l'opération Shell.

### `src/platform/windows_impl.rs:767`
- Gravité : moyen. Confiance : plausible.
- Défaut : un fichier est ouvert en passant son chemin en argument à explorer.exe. La bibliothèque standard ne met l'argument entre guillemets qu'en présence d'espaces, et Explorer découpe sa ligne de commande sur les virgules.
- Scénario : ouvrir `D:\Factures\2024,03.pdf` (une virgule, aucun espace). Explorer reçoit deux arguments, ouvre un autre dossier ou rien du tout, et Oxyde renvoie quand même un succès.

### `src/platform/windows_impl.rs:823`
- Gravité : faible. Confiance : plausible.
- Défaut : la feuille de propriétés est lancée par ShellExecuteExW (verbe properties, INVOKEIDLIST) sur un thread sans COM initialisé, et le résultat est ignoré. Pourtant, le même fichier initialise un STA pour les chemins CLSID dans `open_item`.
- Scénario : Propriétés de la Corbeille (`trash://`, transformé en CLSID) ou d'un élément dont le gestionnaire est une extension Shell. Le dialogue peut ne pas s'ouvrir, sans aucune erreur ni journal.

### `src/platform/windows_impl.rs:924`
- Gravité : faible. Confiance : confirmé.
- Défaut : chaque appel crée un HICON jamais détruit, et l'icône précédente renvoyée par WM_SETICON n'est pas libérée non plus.
- Scénario : chaque fenêtre principale ou fenêtre d'outil ouverte fuit un objet USER. Une longue session qui ouvre beaucoup de fenêtres secondaires les accumule.

### `src/platform/windows_impl.rs:478`
- Gravité : faible. Confiance : confirmé.
- Défaut : GetFileAttributesW et SetFileAttributesW reçoivent le chemin brut sans préfixe `\\?\`, et l'application n'a pas de manifeste longPathAware.
- Scénario : masquer ou afficher un fichier dont le chemin dépasse 260 caractères échoue avec « chemin introuvable », alors que les autres opérations de la bibliothèque standard réussissent.

### `src/platform/windows_impl.rs:345`
- Gravité : faible. Confiance : plausible.
- Défaut : tout échec de SHEmptyRecycleBinW est remonté tel quel. Or l'API répond par une erreur (E_UNEXPECTED) quand la Corbeille est déjà vide.
- Scénario : la Corbeille est vidée par l'Explorateur entre-temps, puis « Vider » dans Oxyde affiche une erreur de suppression.

### `src/platform/windows_impl.rs:403`
- Gravité : faible. Confiance : plausible.
- Défaut : l'occupation est demandée pour chaque lettre logique, lecteurs réseau, optiques et déconnectés compris, alors que la liste des réglages ne garde que les lecteurs fixes et amovibles.
- Scénario : avec un lecteur réseau mappé hors ligne, l'affichage de l'occupation attend le délai réseau, et le total inclut des volumes qui ne sont pas listés.

### `src/platform/windows_impl.rs:317`
- Gravité : moyen. Confiance : plausible.
- Défaut : `trash::os_limited::list` (aussi appelé dans `windows_trash.rs:48`) initialise un STA permanent, local au thread et jamais libéré, sur le thread appelant. Il panique si ce thread est déjà en MTA.
- Scénario : un thread Tokio du pool, une fois utilisé pour lister la Corbeille, reste un STA sans boucle de messages. Les travaux suivants qui demandent `mta_or_existing` sur ce thread (miniatures, PDF, propriétés Shell) tournent en STA sans pompe et peuvent bloquer. Un appel fait dans une portée MTA ferait paniquer le thread.

### `src/platform/windows_trash.rs:52`
- Gravité : optimisation. Confiance : confirmé.
- Défaut : chaque candidat est comparé à tous les éléments de la Corbeille en renormalisant (deux allocations et une conversion en minuscules) les deux côtés à chaque comparaison. `windows_trash_clipboard.rs:68` fait de même, en énumérant en plus les éléments un par un.
- Scénario : copier ou couper 1 000 éléments d'une Corbeille qui en contient 20 000 produit environ 40 millions d'allocations.

### `src/platform/windows_trash_restore.rs:163`
- Gravité : optimisation. Confiance : confirmé.
- Défaut : pour chaque entrée du journal, tous les éléments de la Corbeille sont filtrés en renormalisant les deux chemins, ce qui est quadratique en allocations.
- Scénario : annuler la suppression de 5 000 fichiers avec 20 000 éléments dans la Corbeille fait 100 millions de normalisations avant le premier déplacement.

### `src/platform/windows_trash_restore.rs:157`
- Gravité : faible. Confiance : plausible.
- Défaut : la préparation garde ouverte une sélection complète par élément avant tout déplacement : charge utile, métadonnées et deux chaînes de parents retenues. Le nombre de handles croît avec la taille du lot multipliée par la profondeur des chemins (même chose pour `restore`, ligne 140).
- Scénario : annuler une suppression de quelques milliers de fichiers fait dépasser le seuil de 8 192 handles de `windows_resource_pressure`. Les analyses d'archives et d'aperçus sont refusées pendant toute la restauration.

### `src/platform/windows_trash/listing.rs:43`
- Gravité : faible. Confiance : confirmé.
- Défaut : un seul élément dont le `$I` est absent, illisible ou mal formé fait échouer toute l'énumération native.
- Scénario : un élément est purgé par l'Explorateur pendant l'énumération, ou une charge utile reste orpheline. Le chargement progressif échoue, puis le repli refait une énumération native (qui échoue à nouveau) et une énumération par les propriétés Shell : la Corbeille est parcourue trois fois.

### `src/platform/windows_trash/listing.rs:73`
- Gravité : faible. Confiance : plausible.
- Défaut : la taille et la date d'une charge utile sont lues en suivant les liens. Un lien symbolique ou une jonction mis à la Corbeille est résolu vers sa cible.
- Scénario : une jonction recyclée qui pointe vers un partage hors ligne bloque l'énumération jusqu'au délai réseau. Un lien vers un gros fichier affiche la taille de la cible.

### `src/platform/windows_system_paste.rs:85`
- Gravité : moyen. Confiance : plausible.
- Défaut : le transfert du Shell peut continuer sur les messages du thread de collage. Ce thread est détaché, ne pompe que 30 s puis appelle OleUninitialize, et n'est ni annulé ni attendu à l'arrêt, qui se fait par TerminateProcess.
- Scénario : coller depuis VMware ou Outlook des fichiers virtuels volumineux. Après 30 s, ou si Oxyde est fermé pendant le transfert, la copie s'interrompt et des fichiers partiels restent dans la destination.

### `src/platform/windows_wsl.rs:12`
- Gravité : moyen. Confiance : plausible.
- Défaut : les distributions sont toujours exposées sous `\\wsl.localhost`, sans repli sur `\\wsl$`.
- Scénario : sous Windows 10 LTSC 2021 avec le WSL intégré au système (sans la version du Store), ce nom n'existe pas. Les entrées « Linux » de la barre latérale mènent à des chemins introuvables.

### `src/platform/windows_resource_pressure.rs:55`
- Gravité : faible. Confiance : plausible.
- Défaut : l'admission des analyseurs exige au moins 512 Mio de mémoire physique libre, un seuil fixe qui ne tient pas compte de la mémoire totale.
- Scénario : sur un poste de 4 Go (IoT, client léger) avec un navigateur ouvert, la mémoire disponible reste sous ce seuil. Les listes et aperçus d'archives échouent en permanence avec « System resource pressure stopped parser work ».

### `src/platform/windows_quick_access.rs:9`
- Gravité : faible. Confiance : confirmé.
- Défaut : un échec de la lecture de l'Accès rapide par le Shell est remplacé par une liste vide, sans journal.
- Scénario : sur une erreur COM ou un dossier Shell indisponible, la section Accès rapide apparaît vide, comme si l'utilisateur n'avait rien épinglé.

<!-- LOT TERMINÉ -->
