# Architecture et conventions de l'interface

Ce document décrit les conventions à suivre pour faire évoluer l'interface GPUI d'Oxyde. Les consignes générales des agents sont dans [AGENTS.md](AGENTS.md).

## Organisation

- `src/ui/app/` porte l'état de l'application et les parcours fonctionnels.
- `src/ui/components/` regroupe les composants d'interface composés et les vues réutilisables.
- `src/ui/elements/` regroupe les éléments simples de bas niveau, tels que les boutons.
- `src/ui/theme.rs` contient les palettes et les jetons de couleur sémantiques.
- `src/ui/i18n.rs` expose les messages typés et le contexte de traduction.
- `locales/fr.ftl` et `locales/en.ftl` contiennent les catalogues embarqués.
- `src/ui/format.rs` centralise les formats de nombres, dates et tailles.

## Thèmes

Le thème actif est accessible avec `cx.theme()`. Les thèmes pris en charge sont `ThemeId::Dark` et `ThemeId::Light`, définis dans `src/models/preferences.rs`.

- Utiliser les jetons de `OxydeTheme` pour les surfaces, textes, bordures, actions, sélection, survol, erreurs, menus, onglets, icônes et barres de défilement.
- Si un rôle visuel partagé manque, ajouter un jeton sémantique et lui donner une valeur dans chaque palette. Ne pas introduire une couleur littérale dans une vue pour contourner le thème.
- Garder une valeur locale uniquement si elle exprime un effet propre au composant, comme un fond entièrement transparent. Le rôle doit rester évident dans le code.
- Vérifier le contraste et les états normal, survol, focus, sélection, désactivé et erreur dans les deux palettes. Le texte sur une action remplie doit utiliser le jeton de texte correspondant à cette action.

## Typographie

- Le poids par défaut est Regular (400), défini par `DEFAULT_FONT_WEIGHT` dans `src/ui/style.rs` et appliqué à la racine de l'application. Les descendants héritent de ce poids sauf lorsqu'un ancêtre ou le composant définit explicitement un autre poids.
- Ne préciser un poids local (Light, Medium, Semibold, Bold, etc.) que pour une variation intentionnelle. Les surfaces rendues séparément, comme les infobulles, utilisent le même poids par défaut à leur racine.

## Dimensions et zoom

- Exprimer les dimensions de conception, tailles de texte, espacements, icônes et bordures en `rems()`. La référence initiale est de 16 pixels logiques par rem ; `ui_rem_size` la conserve dans la configuration (10 à 32, par pas de 1). Le réglage « Échelle de l'interface » et Ctrl + molette appliquent le changement immédiatement. Cliquer sur la valeur du réglage rétablit 16.
- `Window::set_rem_size` et les conversions de `src/ui/scale.rs` partagent la même référence, synchronisée sur le thread UI avant le rendu et lors du zoom. Utiliser `rem_pixels` lorsqu'une API impose des `Pixels` (canvas, texte mesuré, listes virtuelles, défilement) ; utiliser `logical` pour convertir les anciennes réservations numériques définies à l'échelle 16. Ces conversions appartiennent au thread UI, jamais aux tâches de fond.
- Les positions de souris, bounds et offsets fournis par GPUI sont déjà en pixels : ne pas les convertir à nouveau. Les ratios, compteurs et durées ne sont pas des dimensions. Les bounds de la fenêtre native restent également en pixels pour que le zoom change le contenu, pas la taille de la fenêtre.
- Les réservations de colonnes et la largeur sauvegardée de la sidebar restent à l'échelle de référence 16 pour préserver les espaces de travail existants. Les convertir au rendu et reconvertir les déplacements de redimensionnement avant sauvegarde. Rendu et sélection de la grille partagent `GridGeometry` pour garder leurs zones alignées.

## Internationalisation

Le contexte `cx.i18n()` fournit la langue active et résout les clés de `Message` vers des `SharedString`.

- Pour ajouter ou modifier un texte destiné à l'utilisateur, ajouter ou réutiliser une variante `Message`, son identifiant dans `Message::key()`, et sa traduction dans **les deux** catalogues. Compléter le test qui vérifie les messages et paramètres des catalogues.
- Conserver les phrases entières dans Fluent. Utiliser `text(Message)` pour une phrase simple et `text_with_args(Message, &FluentArgs)` pour les paramètres et pluriels. Fournir une valeur numérique aux sélecteurs de pluriel ; formater séparément le nombre présenté si nécessaire.
- Garder les noms de fichiers, chemins, extensions, libellés reçus du système et autres données utilisateur comme des données ; ne pas les transformer en clés de traduction. Fluent isole les paramètres pour le texte bidirectionnel. Retirer ces marqueurs uniquement si le résultat devient une donnée de système de fichiers, comme un nom de dossier généré.
- Garder les erreurs d'interface sous forme de clé `Message` dans l'état quand elles peuvent rester visibles après un changement de langue. Les détails techniques peuvent être fournis en paramètre ; les journaux backend ne sont pas des textes UI.
- Les catalogues sont construits au démarrage ou au changement de langue, pas à chaque rendu. Ne pas ajouter de lecture de fichier ou de construction de catalogue dans un chemin de rendu fréquent.
- Pour ajouter une langue, étendre `Language`, son identifiant stable, les formats dépendants de la locale, le catalogue, le sélecteur de préférences et les tests.

Les clés manquantes ou impossibles à formater retombent sur le français ; si le repli échoue, l'interface affiche un marqueur et journalise un avertissement. Les langues actives sont le français et l'anglais.

## Composants partagés et modales

Factoriser lorsqu'au moins deux usages partagent une structure ou un comportement stable. Une ressemblance visuelle seule ne suffit pas : préserver les différences utiles de taille, d'interaction, de focus et d'accessibilité.

`src/ui/components/modal.rs` fournit `Modal`, un composant de présentation sans dépendance à `OxydeApp` :

- l'appelant fournit le contenu, les boutons et les callbacks ;
- l'état métier et les opérations restent dans la vue appelante ;
- le clic extérieur ne ferme la modale que si l'appelant configure `on_backdrop_dismiss` ;
- la gestion du clavier, du focus initial et de l'ordre de rendu reste explicite dans l'appelant ;
- la modale bloque les interactions et le défilement sur le contenu recouvert.

Appliquer le même principe aux futurs composants partagés : API de présentation claire, callbacks et état appartenant à l'appelant, et aucun couplage au modèle métier sans nécessité.

## Préférences et persistance

Les préférences UI sont stockées dans `AppConfig` et lues au démarrage. Elles incluent le thème, la langue, les options d'affichage des fichiers cachés et système et les aperçus médias.

Le regroupement par date est propre à chaque onglet (`TabState.group_by_date`) et persisté dans `SavedTab`, avec une valeur par défaut désactivée pour les anciens espaces de travail. Le bouton calendrier et le réglage des paramètres modifient l’onglet concerné uniquement. Les sections repliées restent un état temporaire de cet onglet. Les périodes suivent les jours du calendrier local et les semaines commençant le lundi, comme dans Tauri ; le tri courant est conservé dans chaque période. La grille groupée virtualise ses lignes de hauteurs différentes ; la géométrie de sélection utilise les mêmes lignes que le rendu.

- Appliquer les changements de thème et de langue immédiatement via les globals GPUI, sans redémarrer ni recharger le dossier.
- Sauvegarder les changements en arrière-plan dans l'ordre. Préserver les autres options et champs de configuration inconnus lors de l'écriture.
- Garder les anciennes configurations lisibles avec des valeurs par défaut pour les champs absents ou inconnus.
- Si la configuration est illisible, ne pas l'écraser automatiquement. Présenter l'état à l'utilisateur et ne pas prétendre que les changements sont persistés.
- Les formats passent par `src/ui/format.rs`. Les nombres et tailles suivent la langue ; les dates suivent la préférence persistée `date_format` (US, European, ISO). Sans choix enregistré, la locale régionale système en-US sélectionne US, les autres European. `show_network` contrôle la visibilité du voisinage réseau dans la sidebar.

Le panneau des paramètres reprend deux colonnes : Apparence et Fichiers à gauche ; Région & langue, Performances, Données de l’application et À propos à droite. Le panneau compact est ancré sous le bouton Oxyde GPUI, sans fond assombri. Les sélecteurs de thème, langue et format de date ouvrent des sous-menus latéraux défilants, sans agrandir le panneau principal. Les sous-menus s’ouvrent à gauche si nécessaire et restent dans les limites de la fenêtre. Une sélection applique le choix sans fermer le sous-menu ; Échap ou un clic extérieur le ferme. Les options reprennent la taille de texte des menus contextuels (0,8125 rem). Les rubriques Performances et Données de l’application restent désactivées avec un libellé « À venir » tant que leurs actions ne sont pas implémentées.

## Réactivité et validation

- Garder les opérations de fichiers, les calculs de taille et les autres travaux coûteux hors du rendu synchrone. Utiliser les mécanismes asynchrones et de fond déjà présents.
- Avant de terminer une modification UI, lancer `cargo check --all-targets --offline` et résoudre les avertissements du projet.
- Exécuter les tests pertinents pour les catalogues, formats, préférences et comportements modifiés.
- Vérifier l'interface en français et en anglais, dans chaque thème. Inclure les libellés longs, les menus, les modales, les états vides et les erreurs, ainsi que les interactions clavier et souris.
- Une compilation réussie ne valide pas à elle seule le contraste, la disposition ou le comportement visuel.

## Références

- [GPUI](https://gpui.rs/)
- [API Rust Fluent](https://docs.rs/fluent-bundle/0.16.0/fluent_bundle/)
- [Sélecteurs et pluriels Fluent](https://projectfluent.org/fluent/guide/selectors.html)
