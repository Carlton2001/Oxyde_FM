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

## Copy/move conflicts.
conflict-skip-all = Tout ignorer
conflict-replace-all = Tout remplacer

common-close = Fermer
common-retry = Réessayer
settings-title = Préférences
settings-appearance = APPARENCE
settings-theme = Thème
settings-scale = Échelle de l’interface
settings-scale-value = { $percentage } %
settings-scale-reset = Rétablir l’échelle à 100 %
settings-language = Langue
theme-dark = { $name } Sombre
theme-light = { $name } Clair
settings-files = FICHIERS
settings-hidden = Fichiers cachés
settings-hidden-description = Afficher les fichiers et dossiers cachés ou commençant par un point
settings-system = Fichiers système
settings-system-description = Afficher les fichiers et dossiers marqués comme système par l’OS
settings-previews = Aperçus médias
settings-previews-description = Afficher les aperçus des fichiers dans les info-bulles de la liste
settings-grid-thumbnails = Miniatures dans la vue grille
settings-grid-thumbnails-description = Afficher les miniatures des images directement dans les cartes
settings-about = À propos
settings-version = Version { $version }
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
tooltip-location = Emplacement : { $value }
tooltip-original-location = Emplacement d’origine : { $value }
tooltip-deleted = Supprimé le : { $value }

## Main interface and commands
menu-open = Ouvrir
menu-open-new-tab = Ouvrir dans un nouvel onglet
menu-open-terminal = Ouvrir dans la console
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
menu-deselect-all = Tout désélectionner
menu-select-all-checks = Tout cocher
menu-deselect-all-checks = Tout décocher
column-name = Nom
column-type = Type
column-size = Taille
column-modified = Modifié le
file-type-folder = Dossier
file-type-file = Fichier
file-type-symlink = Lien symbolique
file-type-broken-link = Lien brisé
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
operation-extract-here = Extraire l’archive
operation-new-folder = Nouveau dossier
operations-title = { $count ->
    [1] Opération ({ $count })
   *[other] Opérations ({ $count })
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
operations-to = Vers : { $path }
operations-items = { $count ->
    [1] { $done } / { $total } élément
   *[other] { $done } / { $total } éléments
    }
operations-remaining = Temps restant : { $seconds } s
operations-duration = Durée : { $duration }
operations-average = Moyenne : { $speed }
operations-speed-waiting = En attente
operations-finalizing = Finalisation…
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

## Sidebar, filters and navigation errors
sidebar-quick-access = Accès rapide
sidebar-drives = LECTEURS
sidebar-locations = EMPLACEMENTS
sidebar-network = Réseau
sidebar-linux = Linux
sidebar-trash = Corbeille
sidebar-home = Dossier personnel
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
path-error-access-denied = Dossier protégé (Droits administrateur requis)
path-error-failed = Impossible d’ouvrir ce dossier. Vérifiez le chemin et réessayez.

## File operations
undo-failed = Annulation impossible : { $detail }
undo-copy-identity-changed = Cet élément ne correspond plus à la copie originale : { $detail }. Annuler est interrompu pour préserver vos fichiers.
undo-copy-unverified = L’historique ne permet pas de vérifier l’identité de cette copie : { $detail }. Les fichiers sont conservés.
undo-overwrite-unavailable = Impossible d’annuler cette copie : elle a remplacé des fichiers existants.
redo-failed = Rétablissement impossible : { $detail }
redo-copy-source-changed = La source a changé depuis la copie : { $detail }. Rétablir est interrompu pour préserver vos fichiers.
redo-copy-unverified = L’historique ne permet pas de vérifier cette copie : { $detail }. Relancez la copie depuis le dossier source.
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
unlabeled-volume = Volume de { $size }
unknown-item = Inconnu
menu-expand-all = Développer tout
menu-collapse-all = Réduire tout
menu-copy-path = Copier le chemin

sidebar-collapse-all = Réduire tout
sidebar-scroll-top = Retourner en haut
sidebar-collapse = Réduire le panneau
sidebar-expand = Agrandir le panneau

settings-windows-icons = Icônes Windows
settings-windows-icons-description = Utiliser les icônes Windows des fichiers et dossiers dans les vues liste et grille.

settings-checkboxes = Cases à cocher de sélection
settings-checkboxes-description = Sélectionner plusieurs éléments sans maintenir Ctrl.

settings-path-tracking = Suivi du chemin
settings-path-tracking-description = Déplier les dossiers parents et révéler le dossier actif dans la barre latérale.

settings-date-format = Format des dates
date-format-us = États-Unis (MM/JJ/AAAA)
date-format-european = Européen (JJ/MM/AAAA)
date-format-iso = ISO 8601 (AAAA-MM-JJ)
settings-network = Voisinage réseau
settings-network-description = Afficher le voisinage réseau dans le panneau gauche.

settings-region-language = Région & langue
settings-performance = Performances
settings-turbo = Mode Turbo
settings-search-limit = Limite de recherche
settings-data = Données de l’application
settings-clear-cache = Effacer le cache
settings-reset-preferences = Réinitialiser les préférences

title-new-panel = Nouveau panneau à droite
title-tools = Recherche
tool-sizes = Calculer les tailles
tool-search = Recherche avancée
quick-search = Recherche rapide
quick-search-title = Rechercher « { $query } » dans { $folder }
quick-search-results = Résultats de recherche
quick-search-running = Recherche en cours…
quick-search-limited = Limite de résultats atteinte
quick-search-empty = Aucun résultat
quick-search-failed = Impossible d’effectuer la recherche
quick-search-clear = Effacer
quick-search-stop = Arrêter la recherche
open-file-location = Ouvrir l’emplacement du fichier
column-location = Emplacement
tool-duplicates = Rechercher les doublons

tabs-new-tab = Nouvel onglet

date-this-month = Ce mois-ci
settings-group-by-date = Grouper par date
date-group-count = { $count ->
    [one] { $number } élément
   *[other] { $number } éléments
}

menu-sort-by = Trier par
menu-sort-ascending = Croissant
menu-sort-descending = Décroissant
menu-sort-extension = Extension
drive-eject = Éjecter
drive-unmount = Démonter
map-network-drive = Connecter un lecteur réseau
disconnect-network-drive = Déconnecter un lecteur réseau
disconnect-this-network-drive = Déconnecter le lecteur réseau
network-force-prompt = Des fichiers sont ouverts ou des opérations sont en cours sur { $drive }. Forcer la déconnexion peut interrompre une copie ou entraîner la perte de modifications non enregistrées. Voulez-vous continuer ?
network-force-action = Forcer la déconnexion
network-category = Catégorie
tooltip-network-category = Catégorie : { $value }
tooltip-network-workgroup = Groupe de travail : { $value }
tooltip-network-location = Emplacement réseau : { $value }
network-workgroup = Groupe de travail
network-location = Emplacement réseau
network-computer = Ordinateur
network-other-devices = Autres périphériques
network-drive-failed = Impossible de modifier la connexion réseau. { $detail }
open-item-failed = Impossible d’ouvrir cet élément. { $detail }
network-map-description = À quel dossier réseau voulez-vous vous connecter ?
network-map-hint = Spécifiez la lettre du lecteur et le dossier auquel vous souhaitez vous connecter.
network-disconnect-description = Sélectionnez les lecteurs réseau à déconnecter, puis cliquez sur Déconnecter.
network-letter = Lecteur
network-folder = Dossier
network-username = Utilisateur
network-password = Mot de passe
network-reconnect = Se reconnecter à l’ouverture de session
network-finish = Terminer
network-disconnect = Déconnecter
network-no-drives = Aucun lecteur réseau trouvé.
network-no-letter = Aucune lettre de lecteur disponible.
network-invalid-path = Saisissez un chemin réseau valide, par exemple \\serveur\partage.
network-connected = Lecteur réseau connecté avec succès.
network-disconnected = Lecteurs réseau déconnectés avec succès.
network-working = Opération en cours…
network-loading = Chargement des lecteurs…
remote-login-title = Authentification requise
remote-domain = Domaine
remote-anonymous = Se connecter anonymement
remote-remember-never = Oublier le mot de passe immédiatement
remote-remember-session = Retenir le mot de passe jusqu’à la déconnexion
remote-remember-always = Retenir indéfiniment
remote-retry = Identifiants refusés. Réessayez.
remote-connect = Se connecter
remote-connect-failed = Impossible de se connecter à cet emplacement.
remote-connect-failed-toast = Connexion impossible : { $detail }
drive-eject-failed = Impossible d’éjecter le lecteur. Il est peut-être utilisé par une autre application. { $detail }
drive-unmount-failed = Impossible de démonter le volume. Il est peut-être utilisé par une autre application. { $detail }
drive-holders = Ouvert dans : { $applications }.
drive-mount-failed = Impossible de monter le volume. { $detail }
drive-mount-hibernated = Windows n’a pas été complètement arrêté (veille prolongée ou démarrage rapide) : ce volume reste verrouillé. Arrêtez complètement Windows, puis réessayez.

archive-format = Archive .{ $format }
compression-fast = Rapide
compression-normal = Équilibrée
compression-best = Maximale
compress = Compresser
compression = Compression
compression-format = Format

disk-image-mount = Monter
disk-image-unmount = Démonter
disk-image-mount-failed = Impossible de monter l’image. { $detail }
disk-image-unmount-failed = Impossible de démonter l’image. { $detail }
disk-image-open-failed = Impossible d’ouvrir l’image. { $detail }
disk-image-busy = Une opération de fichiers utilise encore cette image.
disk-image-no-volume = Aucun volume accessible n’a été attribué à cette image.

advanced-settings = Paramètres
advanced-help = Aide
advanced-search-for = Nom du fichier
advanced-search-for-placeholder = *.* ou nomfichier.txt
advanced-search-in = Rechercher dans
advanced-search-subfolders = Inclure les sous-dossiers
advanced-search-in-archives = Chercher dans les archives
advanced-find-text = Texte dans le fichier
advanced-find-text-placeholder = Rechercher dans le contenu (non binaires)...
advanced-global-options = Options globales
advanced-case-sensitive = Respecter la casse
advanced-ignore-accents = Ignorer les accents
advanced-reset = Réinitialiser
advanced-search = Rechercher
advanced-regex-help-title = Modèles courants
advanced-use-regex = Expression régulière
advanced-regex-any-char = N'importe quel caractère
advanced-regex-any-char-desc = Remplace n'importe quelle lettre ou chiffre. Exemple : "t.t" trouve "tot", "tut", "tit".
advanced-regex-digit = Chiffres
advanced-regex-digit-desc = Trouve n'importe quel chiffre de 0 à 9. Exemple : "photo\d" trouve "photo1", "photo2".
advanced-regex-word-char = Caractères alphabétiques
advanced-regex-word-char-desc = Trouve lettres, chiffres et tirets bas. Exemple : "i\w" trouve "image", "item".
advanced-regex-start-line = Début du texte
advanced-regex-start-line-desc = Le nom doit commencer par ceci. Exemple : "^a" trouve les noms commençant par "a".
advanced-regex-end-line = Fin du texte
advanced-regex-end-line-desc = Le nom doit finir par ceci. Exemple : "txt$" trouve les fichiers finissant par ".txt".
advanced-regex-one-or-more = Un ou plusieurs
advanced-regex-one-or-more-desc = Le caractère précédent se répète 1 ou plusieurs fois. Exemple : "a+" trouve "a", "aa".
advanced-regex-optional = Facultatif
advanced-regex-optional-desc = Le caractère précédent peut être présent ou non. Exemple : "fichiers?" trouve "fichier" et "fichiers".
advanced-regex-either-or = Ou bien
advanced-regex-either-or-desc = Trouve soit la partie gauche, soit la droite. Exemple : "chat|chien" trouve "chat" ou "chien".
advanced-regex-range = Plage de caractères
advanced-regex-range-desc = N'importe quel caractère entre les crochets. Exemple : "[abc]" trouve "a", "b", ou "c".
advanced-regex-negated-set = Ensemble inversé
advanced-regex-negated-set-desc = Tout sauf ce qui est entre crochets. Exemple : "[^0-9]" trouve tout sauf les chiffres.
advanced-regex-group = Groupe
advanced-regex-group-desc = Regroupe plusieurs symboles. Exemple : "(ab)+" trouve "ab", "abab".
advanced-regex-count-exact = Nombre exact
advanced-regex-count-exact-desc = Se répète exactement n fois. Exemple : "a{ "{" }3{ "}" }" trouve exactement "aaa".
advanced-size = Taille
advanced-date = Modifié le
advanced-min = Min
advanced-max = Max
advanced-invalid = Vérifiez les expressions, les tailles et les dates (AAAA-MM-JJ).
advanced-date-placeholder = AAAA-MM-JJ
advanced-weekdays = Lun Mar Mer Jeu Ven Sam Dim

duplicate-locations = Emplacements de recherche
duplicate-settings = Paramètres de recherche
duplicate-name = Nom
duplicate-size = Taille
duplicate-content = Contenu (lent)
duplicate-current = Dossier actuel
duplicate-warning = Sélectionner plusieurs emplacements peut ralentir considérablement la recherche.
duplicate-filter = Filtrer les résultats…
duplicate-idle = Configurez la recherche puis cliquez sur Rechercher.
duplicate-empty = Aucun doublon trouvé.
duplicate-scanning = Parcours des fichiers : { $current }
duplicate-hashing = Comparaison du contenu : { $current } / { $total }
duplicate-groups = { $groups ->
    [one] { $group_count } groupe de doublons
   *[other] { $group_count } groupes de doublons
    }
duplicate-files = { $file_count ->
    [one] { $file_number } fichier
   *[other] { $file_number } fichiers
    }
duplicate-group = Groupe { $index }
duplicate-failed = La recherche a échoué : { $error }
duplicate-size-range = { $min } – { $max }
duplicate-disk = Disque #{ $disk }
duplicate-go-to-folder = Cliquez pour aller au dossier
folder-size-empty = Vide
folder-size-provisional-description =
    Taille à actualiser.
    Le recalcul automatique attend une accalmie.
    Actualisez pour forcer la mise à jour.
quick-search-current-folder = Dossier courant

settings-clear-cache-confirm = Supprimer les caches d’icônes et de miniatures ? Ils seront recréés au besoin.
settings-reset-confirm = Rétablir les préférences par défaut ? Les onglets ouverts seront conservés.
settings-cache-clearing = Nettoyage du cache…
settings-cache-cleared = Cache effacé.
settings-cache-error = Impossible d’effacer entièrement le cache. Réessayez.
settings-preferences-reset = Préférences réinitialisées.

settings-turbo-description = Mode Turbo par défaut des nouvelles copies, déplacements, suppressions, mises à la Corbeille, compressions et extractions. S’applique aussi aux calculs de tailles de dossiers, y compris en cours. Les opérations de fichiers en cours conservent leur réglage individuel.
settings-search-limit-description = Nombre maximal de résultats des nouvelles recherches rapides et avancées. Pas de 500, minimum 500.

navigation-up = Haut

about-repository = Dépôt GitHub
about-main-tools = Outils principaux
update-available-toast = Oxyde { $detail } est disponible. Installez-la depuis Paramètres › À propos.
update-failed-toast = Mise à jour impossible : { $detail }
update-check = Rechercher des mises à jour
update-checking = Recherche en cours…
update-current = Oxyde est à jour
update-check-failed = Échec de la recherche, réessayer
update-install = Installer la version { $version }
update-download-portable = Télécharger la version { $version }
update-downloading = Téléchargement de la mise à jour…

settings-search-limit-reset = Rétablir la limite par défaut de 3 000 résultats

menu-copy-name = Copier le nom

trash-restore = Restaurer
trash-restore-all = Tout restaurer
trash-delete-permanent = Supprimer définitivement
trash-delete-all = Vider la Corbeille
trash-original-location = Emplacement d’origine
trash-deleted-at = Date de suppression

trash-restore-selection = Restaurer la sélection

original-location-filter-placeholder = Filtrer par emplacement d’origine…

trash-properties = Propriétés de : Corbeille
trash-locations = Emplacement de la Corbeille
trash-available-space = Espace disponible
trash-selected-settings = Paramètres pour l’emplacement sélectionné
trash-custom-size = Taille personnalisée
trash-max-capacity = Taille maximale
trash-dont-move = Ne pas déplacer les fichiers vers la Corbeille
trash-delete-immediately = Supprimer immédiatement les fichiers lorsqu’ils sont effacés
trash-confirm-delete = Afficher la confirmation de suppression
trash-megabytes = Mo
trash-settings-error = Impossible d’appliquer les paramètres. Vérifiez la capacité saisie.
trash-auto-delete = Suppression automatique
trash-auto-delete-old = Supprimer automatiquement les éléments anciens
trash-retention-days = Durée de conservation (jours)
trash-gnome-retention-note = Réglage de GNOME, appliqué quelle que soit l’application qui a supprimé les fichiers.
trash-kde-time-limit = Supprimer les éléments plus anciens que (jours)
trash-kde-size-limit = Limiter la taille (% du disque)
trash-kde-limit-action = Lorsque la limite est atteinte
trash-kde-warn = Avertir
trash-kde-delete-oldest = Supprimer les plus anciens
trash-kde-delete-largest = Supprimer les plus gros
trash-kde-retention-note = Réglages de KDE, appliqués lors des suppressions faites depuis Dolphin et les applications KDE.
trash-retention-error = Impossible d’appliquer les paramètres. Vérifiez les valeurs saisies.

trash-settings-ok = OK
trash-delete-prompt = Supprimer les éléments sélectionnés ?

trash-item-count = { $count ->
    [one] ({ $formatted } élément)
   *[other] ({ $formatted } éléments)
    }

trash-permanent-delete-prompt = Supprimer définitivement les éléments sélectionnés ?

drive-tooltip-usage = { $used } / { $total }

properties-system = Propriétés Système
properties-created = Créé le
properties-accessed = Accédé le
properties-link-target = Cible du lien
properties-used = Espace utilisé
properties-capacity = Capacité
properties-contains = Contenu
properties-counts = { $files } fichiers, { $folders } dossiers
properties-drive = Lecteur
properties-file-system = Système de fichiers

properties-copy-name-double-click = Copier le nom sur double clic

toast-copied = Copié dans le presse-papier
toast-cut = Coupé dans le presse-papier
toast-names-copied = { $count ->
    [one] { $number } nom copié
   *[other] { $number } noms copiés
    }
toast-paths-copied = { $count ->
    [one] { $number } chemin copié
   *[other] { $number } chemins copiés
    }
toast-restored = { $count ->
    [one] { $number } élément restauré
   *[other] { $number } éléments restaurés
    }
toast-purged = { $count ->
    [one] { $number } élément supprimé définitivement
   *[other] { $number } éléments supprimés définitivement
    }
toast-mounted = Image disque montée
toast-unmounted = Image disque démontée

toast-undo = Opération annulée
toast-redo = Opération rétablie

toast-failed = Opération impossible : { $detail }

toast-copy-completed = Copie terminée
toast-move-completed = Déplacement terminé
toast-trash-completed = Mise à la corbeille terminée
toast-delete-completed = Suppression définitive terminée
toast-compress-completed = Archive créée
toast-extract-completed = Extraction terminée

conflict-source = Nouveau fichier
conflict-target = Fichier existant
conflict-newer = Plus récent
conflict-older = Plus ancien
conflict-larger = Plus volumineux
conflict-items = éléments
conflict-skip = Ignorer
conflict-replace = Remplacer
conflict-all = Tous
conflict-identical = Identiques
conflict-search = Filtrer les résultats…
conflict-empty = Aucun conflit correspondant
conflict-prompt = Un élément nommé « { $name } » existe déjà. Que voulez-vous faire ?
conflict-advanced-title = Résolution de conflits
conflict-advanced = Avancé
conflict-apply = Appliquer les décisions
conflict-step-title = { $current } / { $total } Conflit de fichier
conflict-inherited = Hérité de { $parent } : { $action }
conflict-back = Retour
conflict-recent-tab = Récents
conflict-older-tab = Anciens
conflict-automatic = Auto
conflict-apply-summary = Appliquer: { $replace_count ->
        [one] { $replace_number } remplacement
       *[other] { $replace_number } remplacements
    }, { $skip_count ->
        [one] { $skip_number } ignoré
       *[other] { $skip_number } ignorés
    }

peek-failed = Impossible d’ouvrir Peek
properties-general = Général
properties-shortcut = Raccourci
shortcut-target = Cible
shortcut-arguments = Arguments
shortcut-working-directory = Démarrer dans
shortcut-comment = Commentaire
shortcut-run = Exécuter
shortcut-normal = Fenêtre normale
shortcut-minimized = Réduite
shortcut-maximized = Agrandie
shortcut-saving = Enregistrement…
shortcut-invalid-target = La cible ne doit pas être vide et les champs doivent être valides.

# Archive cache recovery and resource limits
archive-cache-full = Impossible d’ouvrir { $detail }. Oxyde ne peut pas libérer assez de place : des copies sont encore utilisées, ont été modifiées ou viennent d’être ouvertes. Fermez les documents inutilisés puis réessayez, ou extrayez ce fichier vers un dossier de votre choix.
archive-cache-busy = Impossible d’ouvrir { $detail }. Une autre ouverture d’archive est encore en préparation. Réessayez dans un instant, ou extrayez ce fichier vers un dossier de votre choix.
archive-cache-unavailable = Impossible d’ouvrir { $detail }. Oxyde n’a pas pu préparer la copie temporaire. Vous pouvez extraire ce fichier vers un dossier de votre choix.
archive-cache-extract = Extraire et ouvrir…
archive-cache-exported = Fichier extrait : { $detail }
archive-limit-bytes = Le traitement est arrêté : la taille totale décompressée dépasse la limite de { $limit } pour cette action. La taille compressée de l’archive n’est pas la taille prise en compte.
archive-processing-failed = Le traitement de l’archive a échoué. Le fichier peut être endommagé ou nécessiter trop de ressources.
archive-recovery-notice = Fichiers conservés
archive-recovery-completed = L’action est terminée, mais des fichiers ou dossiers ont été conservés. Vérifiez ces emplacements avant de les supprimer : { $locations }
archive-recovery-stopped = L’action n’a pas été menée à son terme. Des fichiers ou dossiers ont été conservés, y compris les résultats déjà publiés. Vérifiez ces emplacements avant de réessayer : { $locations }
archive-recovery-more = { $count } autres emplacements sont consignés dans le journal de l’application.
archive-limit-source-bytes = La création de l’archive est arrêtée : la taille totale des fichiers sources dépasse la limite de { $limit } pour cette action.
archive-limit-encoded-bytes = La création de l’archive est arrêtée : la taille de sortie dépasse la limite de { $limit } pour cette action.
archive-limit-entries = Le traitement est arrêté : l’archive contient plus de { $limit } fichiers et dossiers.
archive-limit-depth = Le traitement est arrêté : un chemin de l’archive dépasse { $limit } niveaux, nom du fichier compris.
archive-limit-time = Le traitement est arrêté : la durée maximale de { $limit } minutes a été atteinte. Les pauses ne sont pas comptées.
archive-limit-decoded = Le traitement est arrêté : les données décompressées, en-têtes compris, dépassent { $limit }.
archive-source-changed = L’archive a changé pendant l’opération. Oxyde a arrêté l’opération et conservé la version actuelle de l’archive. Relancez l’opération depuis cette version.

quick-search-archives-skipped =
    { $count ->
        [one] Résultats incomplets : { $number } archive ignorée
       *[other] Résultats incomplets : { $number } archives ignorées
    }
quick-search-archive-skipped-detail = Première archive ignorée : { $path }. { $reason }
quick-search-archive-unreadable = Cette archive est invalide, non prise en charge ou illisible.
quick-search-archive-content-not-searched = Le contenu des archives n’est pas recherché. Les entrées de cette archive sont exclues lorsqu’un filtre de contenu est actif.
quick-search-archive-dates-unavailable = Les dates des entrées ne sont pas disponibles pour ce format d’archive. Ses entrées sont exclues lorsqu’un filtre de date est actif.

undo-new-folder-changed = Le dossier ne correspond plus à celui qui a été créé : { $detail }. Annuler est interrompu pour préserver vos fichiers.
undo-new-folder-unverified = L’historique ne permet pas de vérifier ce dossier : { $detail }. Le dossier et son contenu sont conservés.
undo-new-folder-not-empty = Le dossier contient maintenant des fichiers ou des sous-dossiers : { $detail }. Annuler ne supprime pas son contenu.
undo-new-folder-missing = Le dossier créé est introuvable : { $detail }. Annuler est interrompu sans supprimer d’autres éléments.
redo-new-folder-conflict = Un élément existe déjà à cet emplacement : { $detail }. Rétablir ne le remplace pas.
redo-new-folder-parent-missing = Le dossier parent est introuvable : { $detail }. Rétablir ne recrée pas les dossiers parents.
redo-new-folder-unverified = L’historique ne permet pas de rétablir ce dossier : { $detail }. Aucun élément n’a été créé.

redo-move-changed = Le fichier ou dossier a changé depuis l’opération : { $detail }. Rétablir le déplacement est interrompu pour préserver vos fichiers.
redo-move-conflict = Un élément occupe la destination : { $detail }. Rétablir le déplacement ne le remplace pas.
redo-move-unverified = Cet historique ne permet pas de rétablir le déplacement en toute sécurité : { $detail }. Relancez le déplacement depuis les fichiers actuels.
redo-move-incomplete = Le déplacement a été interrompu et certains éléments n’ont pas pu revenir à leur emplacement initial. Des fichiers ou dossiers sont conservés pour récupération : { $detail }. Vérifiez ces emplacements avant de réessayer.

undo-recovery-incomplete = L’annulation a été interrompue et certains éléments n’ont pas pu revenir à leur emplacement initial. Des fichiers ou dossiers ont été conservés pour récupération. Vérifiez ces emplacements avant de réessayer : { $detail }.
preview-unavailable = Impossible de charger cet aperçu. Essayez d’ouvrir le fichier directement.
preview-image-limit = Cette image dépasse les limites de l’aperçu. Le fichier reste accessible.
preview-animation-static = Aperçu fixe : l’animation dépasse les limites de l’aperçu.

undo-move-changed = Impossible d’annuler ce déplacement : le fichier ou dossier a été remplacé ou recréé. Il a été conservé à son emplacement actuel. { $detail }
undo-move-unverified = Impossible d’annuler ce déplacement en toute sécurité : l’identité du fichier ou dossier n’a pas pu être vérifiée. Il a été conservé. { $detail }

undo-rename-changed = Impossible d’annuler ce renommage : le fichier ou dossier a été remplacé. Il a été conservé à son emplacement actuel. { $detail }
undo-rename-unverified = Impossible d’annuler ce renommage en toute sécurité : l’identité du fichier ou dossier n’a pas pu être vérifiée. Il a été conservé. { $detail }
redo-rename-changed = Impossible de rétablir ce renommage : le fichier ou dossier a été remplacé. Il a été conservé à son emplacement actuel. { $detail }
redo-rename-unverified = Impossible de rétablir ce renommage en toute sécurité : l’identité du fichier ou dossier n’a pas pu être vérifiée. Il a été conservé. { $detail }

transfer-source-changed = La source a été remplacée ou n’est plus un fichier ou dossier ordinaire. L’opération a été arrêtée ; les éléments déjà transférés sont conservés. Vérifiez la source puis réessayez.
transfer-source-unverified = L’identité de la source ne peut pas être vérifiée sur cet emplacement. L’opération a été arrêtée ; les éléments déjà transférés sont conservés. Essayez depuis un dossier local compatible.

trash-restore-ambiguous = Plusieurs éléments de la corbeille proviennent du même emplacement. Ouvrez la corbeille et sélectionnez précisément l’élément à restaurer. Les éléments déjà restaurés sont conservés.
invalid-encoding-name = { $name } (encodage non valide)
unrepresentable-path = Le nom de « { $path } » n’est pas un texte UTF-8 valide. Renommez-le avant de le copier, déplacer, supprimer ou compresser ; aucun élément n’a été modifié.
properties-permissions = Droits d’accès
properties-contents = Contenu
permissions-owner = Propriétaire
permissions-group = Groupe
permissions-others = Autres
permissions-access = Accès
permissions-execution = Exécution
permissions-executable = Autoriser l’exécution du fichier comme un programme
permissions-none = Aucun
permissions-read-only = Lecture seule
permissions-read-write = Lecture et écriture
permissions-list-files = Lister les fichiers uniquement
permissions-access-files = Accéder aux fichiers
permissions-create-delete = Créer et supprimer des fichiers
permissions-not-owner = Vous n’êtes pas le propriétaire, vous ne pouvez donc pas modifier ces droits.
permissions-changed = L’élément a changé depuis l’ouverture de cette fenêtre ; rien n’a été modifié. Rouvrez ses propriétés.
permissions-not-kept = Ce système de fichiers ne conserve pas les droits Unix (NTFS ou FAT, par exemple) : ils sont restés tels quels.
permissions-unchanged = Inchangé
permissions-enclosed = Modifier les droits des éléments contenus
permissions-enclosed-hint = S’applique à tous les fichiers et dossiers contenus, à toute profondeur, au clic sur OK. Les liens symboliques et les éléments d’autres disques ne sont pas modifiés.
permissions-files = Fichiers
permissions-folders = Dossiers
permissions-incomplete = { $count } éléments contenus n’ont pas pu être modifiés ; les autres et le dossier l’ont été. Premier échec : { $detail }
permissions-multiple = Plusieurs
permissions-mixed = Mixte
permissions-folder-access = Accès aux dossiers
permissions-file-access = Accès aux fichiers
permissions-selection-incomplete = { $count } éléments n’ont pas pu être modifiés ; les autres l’ont été. Premier échec : { $detail }
trash-restore-changed = L’élément de la corbeille a changé depuis sa sélection. La restauration a été arrêtée. Actualisez la corbeille puis sélectionnez à nouveau l’élément. Les éléments déjà restaurés sont conservés.
trash-restore-conflict = Un fichier ou un dossier existe déjà à l’emplacement de restauration. Il a été conservé et l’élément concerné reste dans la corbeille. Renommez ou déplacez la destination puis réessayez.
trash-restore-unavailable = Cet élément n’a pas pu être restauré. Vérifiez qu’il est toujours dans la corbeille et que son dossier d’origine existe et est accessible, puis réessayez. Les éléments déjà restaurés sont conservés.

history-delete-changed = L’élément ne correspond plus à celui enregistré lors de la suppression, ou il n’est plus disponible. Annuler ou Rétablir a été arrêté pour préserver les fichiers actuels. Les éléments déjà traités sont conservés.
history-delete-unverified = Oxyde ne dispose pas des informations nécessaires pour annuler ou rétablir cette suppression en toute sécurité. L’action a été arrêtée. Vous pouvez vérifier la corbeille et restaurer manuellement l’élément voulu.

preview-timed-out = Cet aperçu a dépassé le délai de 15 secondes. Le fichier reste accessible.
copy-name-suffix = Copie
shell-delete-completed-error = Windows a terminé la suppression de cet élément, puis a signalé une erreur. L’historique conserve l’opération terminée. Vérifiez le dossier et la corbeille avant de réessayer.
shell-delete-unconfirmed = Windows n’a pas confirmé la suppression de cet élément. La suppression a pu être partielle. Vérifiez le dossier et la corbeille avant de réessayer.

archive-listing-metadata-limit = Impossible d’afficher le contenu : la liste des fichiers dépasse { $limit }. Cette limite concerne les noms et les informations des fichiers, pas leur taille décompressée. Vous pouvez essayer une extraction avec ses limites habituelles.
archive-listing-worker-failed = Impossible de lire le contenu de cette archive : le processus de lecture a échoué ou sa réponse est invalide. L’archive n’a pas été modifiée. Vous pouvez réessayer ou tenter une extraction avec ses limites habituelles.

toast-transfer-skipped = Tous les éléments ont été ignorés.

properties-calculating-size = Calcul de la taille…
drag-new-tab = Créer un nouvel onglet
drag-unavailable = Dépôt indisponible
properties-loading = Chargement des propriétés…

storage-unavailable = Impossible d’accéder aux données d’Oxyde. En mode portable, le dossier data doit être accessible en écriture.

properties-selection-items = { $number } éléments
properties-total-size = Taille totale
properties-multiple-locations = Plusieurs emplacements
operations-affected-items = { $count ->
    [1] Élément concerné ({ $number })
   *[other] Éléments concernés ({ $number })
    }
operations-view-items = Voir les { $count } éléments…
operations-items-filter = Rechercher un nom ou un chemin…
operations-no-matching-items = Aucun élément correspondant.
operations-items-loading = Chargement des éléments…
operations-item-done = Traité
operations-item-not-processed = Non traité
operations-item-skipped = Ignoré
operations-item-partial = Partiellement traité
operations-item-unknown = État inconnu
operations-item-interrupted = Interrompu

default-archive-format = Type d’archive par défaut

settings-onedrive = OneDrive
settings-onedrive-description = Afficher OneDrive dans la barre latérale lorsqu’il est configuré.

trash-restore-summary = Restauration terminée : { $restored } élément(s) restauré(s), { $failed } en échec. Les autres éléments ont été traités. Consultez le journal pour le détail des erreurs.
transfer-destination-collision = Plusieurs éléments sélectionnés aboutiraient à la même destination : { $path }. Renommez-les ou transférez-les séparément. Aucun fichier n’a été transféré.
archive-link-refused = Une archive ne peut pas contenir de lien symbolique : { $path }. Oxyde refuse les liens à l’ouverture et à l’extraction des archives, pour qu’aucune extraction ne puisse écrire hors de sa destination. Rien n’a été modifié.

grouping = Grouper par
group-none = Aucun
group-date = Date de modification
group-type = Type de fichier
group-folders = Dossiers
group-archives = Archives
group-applications = Applications et installateurs
group-documents = Documents
group-images = Images
group-video = Vidéos
group-audio = Audio
group-code = Code et scripts
group-other = Autres
group-disk-images = Images disque
mouse-right-click = Clic droit
grouping-current = Groupé par : { $mode }
group-size = Taille
group-extension = Extension
group-created = Date de création
group-deleted = Date de suppression
group-size-empty = Vide (0 octet)
group-no-extension = Sans extension
group-origin-unknown = Emplacement d’origine inconnu
group-date-unknown = Date inconnue
group-select-all = Sélectionner tous les éléments du groupe
group-deselect-all = Désélectionner tous les éléments du groupe

viewer-title = Visionneuse
viewer-open = Ouvrir la visionneuse
viewer-close = Fermer la visionneuse
viewer-visual-paused = Affichage vidéo en pause
viewer-file-unavailable = Ce fichier ne peut pas être affiché.
viewer-slide = Diapositive
viewer-text-truncated = Seuls les 512 premiers Mio du fichier sont affichés.
viewer-empty = Sélectionnez un fichier dans un panneau explorateur.
viewer-detach = Détacher la visionneuse
viewer-detached = La visionneuse est ouverte dans une fenêtre secondaire.
viewer-show-window = Afficher la fenêtre de la visionneuse
viewer-play = Lire
viewer-pause = Pause

viewer-rendered = Rendu
viewer-code = Code
viewer-highlighted = Coloration syntaxique

viewer-enable-highlighting = Activer la coloration syntaxique
viewer-disable-highlighting = Désactiver la coloration syntaxique

group-scripts = Scripts

viewer-loading = Chargement du fichier…

viewer-autoplay = Lecture automatique
viewer-enable-autoplay = Activer la lecture automatique des prochains médias
viewer-disable-autoplay = Désactiver la lecture automatique des prochains médias

audio-no-tags = Aucun tag disponible
audio-album = Album
audio-track = Piste
audio-year = Année


quick-search-content-skipped =
    { $count ->
        [one] Contenu non vérifié : { $number } fichier
       *[other] Contenu non vérifié : { $number } fichiers
    }
quick-search-content-skipped-detail = Recherche de contenu incomplète pour { $path } : document illisible ou limite de ressources atteinte.
