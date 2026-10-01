## Shared dialog actions
common-cancel = Annuler

## Rename (also used by inline rename)
rename-title = Renommer
rename-name-placeholder = Nom du fichier
rename-error-empty-name = Le nom ne peut pas être vide.
rename-error-already-exists = Un fichier ou un dossier portant déjà ce nom existe ici.
rename-error-not-found = L’élément à renommer n’existe plus ou n’est plus accessible.
rename-error-access-denied = Le renommage est refusé. Vérifiez les autorisations de cet élément.
rename-error-invalid-name = Le renommage a échoué. Vérifiez le nom et réessayez.
rename-error-unavailable = Impossible de renommer cet élément pour le moment.
rename-error-task-failed = Le renommage a échoué. Réessayez.

## Copy/move conflicts. $count is a number, never a preformatted string.
conflict-title = Fichiers déjà présents
conflict-description =
    { $count ->
        [one] { $number } conflit détecté. Que souhaitez-vous faire ?
       *[other] { $number } conflits détectés. Que souhaitez-vous faire ?
    }
conflict-skip-all = Ignorer tout
conflict-replace-all = Remplacer tout

common-close = Fermer
common-retry = Réessayer
settings-title = Préférences
settings-appearance = APPARENCE
settings-theme = Thème
settings-scale = Échelle de l’interface
settings-scale-value = { $reference } px
settings-scale-reset = Rétablir la référence à 16 px par rem
settings-language = Langue
theme-dark = Sombre
theme-light = Clair
settings-files = FICHIERS
settings-hidden = Fichiers cachés
settings-hidden-description = Afficher les fichiers et dossiers cachés ou commençant par un point
settings-system = Fichiers système
settings-system-description = Afficher les fichiers et dossiers marqués comme système par l’OS
settings-previews = Aperçus médias
settings-previews-description = Afficher les aperçus des fichiers dans les info-bulles de la liste
settings-grid-thumbnails = Miniatures dans la vue grille
settings-grid-thumbnails-description = Afficher les miniatures des images directement dans les cartes
settings-about = À PROPOS
settings-version = Version { $version }
settings-description = Explorateur de fichiers moderne
settings-save-error = Préférences appliquées, mais sauvegarde impossible. Réessayez.
settings-load-error = Impossible de lire les préférences enregistrées. Les changements ne seront pas sauvegardés pour protéger ce fichier.

status-folders = { $count ->
    [one] { $number } dossier
   *[other] { $number } dossiers
    }
status-files = { $count ->
    [one] { $number } fichier
   *[other] { $number } fichiers
    }
view-details = Vue détaillée
view-grid = Vue grille
selected-files = { $count ->
    [one] { $number } fichier sélectionné ({ $size })
   *[other] { $number } fichiers sélectionnés ({ $size })
    }
selected-folders = { $count ->
    [one] { $number } dossier sélectionné ({ $size })
   *[other] { $number } dossiers sélectionnés ({ $size })
    }
selected-mixed = { $folder_count ->
    [one] { $folders } dossier
   *[other] { $folders } dossiers
    } et { $file_count ->
    [one] { $files } fichier
   *[other] { $files } fichiers
    } sélectionnés ({ $size })
tooltip-type = Type : { $value }
tooltip-size = Taille : { $value }
tooltip-modified = Modifié le : { $value }

## Main interface and commands
menu-open = Ouvrir
menu-open-new-tab = Ouvrir dans un nouvel onglet
menu-pin-quick-access = Épingler à l’accès rapide
menu-unpin-quick-access = Désépingler de l’accès rapide
menu-cut = Couper
menu-copy = Copier
menu-paste = Coller
menu-rename = Renommer
menu-delete = Supprimer
menu-properties = Propriétés
menu-more-options = Plus d’options
menu-new-folder = Nouveau dossier
menu-duplicate-tab = Dupliquer l’onglet
menu-close-other-tabs = Fermer les autres onglets
menu-close-tab = Fermer l’onglet
menu-select-all = Tout sélectionner
menu-select-all-checks = Tout cocher
menu-deselect-all-checks = Tout décocher
column-name = Nom
column-type = Type
column-size = Taille
column-modified = Modifié le
file-type-folder = Dossier
file-type-file = Fichier
file-type-symlink = Lien symbolique
file-type-junction = Jonction
empty-filtered = Aucun élément ne correspond aux filtres.
folder-loading = Chargement du dossier…
folder-empty = Ce dossier est vide.
preview-loading = Chargement de l’aperçu…
operation-failed = Opération impossible
path-placeholder = Chemin d’accès
navigation-back = Précédent
navigation-forward = Suivant
navigation-refresh = Actualiser
navigation-undo = Annuler
navigation-redo = Rétablir
operation-copy = Copie
operation-move = Déplacement
operation-rename = Renommage
operation-delete = Suppression
operation-compress = Compression
operation-extract = Extraction
operation-compress-zip = Compresser en ZIP
operation-extract-here = Extraire l’archive
operation-new-folder = Nouveau dossier
operation-restore = Restauration
operations-title = { $count ->
    [1] Opération
   *[other] Opérations
    }
operations-show-panel = Afficher les opérations
operations-hide-panel = Masquer les opérations
operations-queued = En attente
operations-calculating = Calcul en cours…
operations-running = En cours
operations-paused = En pause
operations-cancelled = Annulée
operations-completed = Terminée
operations-error = Échec
operations-from = De : { $path }
operations-to = Vers : { $path }
operations-items = { $done } / { $total } éléments
operations-remaining = Temps restant : { $seconds } s
operations-duration = Durée : { $duration }
operations-average = Moyenne : { $speed }
operations-speed-waiting = En attente
operations-pause = Pause
operations-resume = Reprendre
operations-stop = Annuler l’opération
operations-fast = Mode rapide activé — cliquer pour passer en mode normal
operations-normal = Mode normal activé — cliquer pour passer en mode rapide
operations-dismiss = Retirer
operations-dismiss-all = Retirer toutes les opérations terminées
operations-empty = Aucune opération
name-filter-placeholder = Filtrer par nom…
undo-operation = Annuler : { $operation }
redo-operation = Rétablir : { $operation }
shortcut-back = Alt+Gauche
shortcut-forward = Alt+Droite

## Sidebar, filters and navigation errors
sidebar-quick-access = ACCÈS RAPIDE
sidebar-drives = LECTEURS
sidebar-locations = EMPLACEMENTS
sidebar-network = Réseau
sidebar-trash = Corbeille
size-tiny = Minuscule (< 10 Ko)
size-small = Petit (10 Ko - 1 Mo)
size-medium = Moyen (1 Mo - 100 Mo)
size-large = Grand (100 Mo - 1 Go)
size-huge = Très grand (> 1 Go)
date-today = Aujourd’hui
date-yesterday = Hier
date-this-week = Cette semaine
date-last-week = La semaine dernière
date-older = Plus ancien
path-error-not-found = Ce dossier est introuvable ou n’est plus disponible.
path-error-access-denied = L’accès à ce dossier est refusé. Vérifiez vos autorisations.
path-error-failed = Impossible d’ouvrir ce dossier. Vérifiez le chemin et réessayez.

## File operations
undo-failed = Annulation impossible : { $detail }
undo-overwrite-unavailable = Impossible d’annuler cette copie : elle a remplacé des fichiers existants.
redo-failed = Rétablissement impossible : { $detail }
prepare-failed = Préparation impossible : { $detail }
transfer-failed = Échec du transfert : { $detail }
drag-copy = Copier
drag-move = Déplacer
new-folder-name = Nouveau dossier
new-folder-numbered = Nouveau dossier ({ $number })
native-menu-empty = Aucune commande disponible
native-menu-failed = Chargement du menu natif échoué : { $detail }
drag-items = { $count ->
    [one] { $number } élément
   *[other] { $number } éléments
    }

native-menu-loading = Chargement du menu…
local-drive = Disque local ({ $drive })
unknown-item = Inconnu
menu-expand-all = Développer tout
menu-collapse-all = Réduire tout
menu-copy-path = Copier le chemin

sidebar-collapse-all = Réduire tout
sidebar-scroll-top = Retourner en haut
sidebar-collapse = Réduire le panneau
sidebar-expand = Agrandir le panneau
