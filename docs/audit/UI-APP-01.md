# Audit UI-APP-01

Fichiers lus en entier :

- [x] `src/ui/app/advanced_search.rs`
- [x] `src/ui/app/advanced_search/calendar.rs`
- [x] `src/ui/app/advanced_search/help.rs`
- [x] `src/ui/app/advanced_search/render.rs`
- [x] `src/ui/app/advanced_search/settings.rs`
- [x] `src/ui/app/advanced_search/validation.rs`
- [x] `src/ui/app/advanced_search/window.rs`
- [x] `src/ui/app/archive_password.rs`
- [x] `src/ui/app/archive_protection.rs`
- [x] `src/ui/app/archives.rs`
- [x] `src/ui/app/calendar.rs`
- [x] `src/ui/app/cards.rs`
- [x] `src/ui/app/clipboard.rs`
- [x] `src/ui/app/context_menu_render/column_filter.rs`
- [x] `src/ui/app/context_menu_render/items.rs`
- [x] `src/ui/app/context_menu_render/mod.rs`
- [x] `src/ui/app/context_menu_render/native.rs`
- [x] `src/ui/app/context_menus.rs`

Aucun constat critique ni élevé. Les fichiers `advanced_search.rs`, `archive_protection.rs` (hors mots de passe), `column_filter.rs`, `items.rs` et `mod.rs` de `context_menu_render` n'ont rien donné de justifiable au-delà des entrées ci-dessous.

## Moyen

- `src/ui/app/archive_password.rs:108` (aussi 131-133) — moyen, confirmé. Quand une invite de mot de passe est déjà ouverte, une seconde demande renvoie « traité » et jette sa reprise sans rien afficher. L'appelant `file_operations.rs:271` s'arrête alors sans toast ni alerte. Scénario : extraire deux archives chiffrées d'un coup. La première ouvre l'invite, la seconde échoue sans message. Après la saisie, seule la première est relancée. Même perte pour une navigation (`open_locked_archive`), un renommage ou une ouverture externe.

- `src/ui/app/archive_password.rs:282` (et 287-311) — moyen, plausible. La reprise après mot de passe remet en file l'opération entière, sans tenir compte de ce qui a déjà été fait. Elle repart aussi d'une opération neuve : les résolutions de conflit et les réglages de l'opération d'origine sont perdus. Scénario : copie ou déplacement de plusieurs sources dont une dans une archive chiffrée. Les sources déjà traitées sont recopiées en doublons « (2) », ou échouent parce qu'elles ont déjà été déplacées. Les conflits sont redemandés.

- `src/ui/app/advanced_search/window.rs:25` — moyen, confirmé. Si la fenêtre de recherche avancée existe déjà, elle est seulement réactivée. Son volet, sa racine et ses options restent ceux de la première ouverture. Scénario : ouvrir la recherche depuis le volet gauche, la laisser ouverte, activer le volet droit dans un autre dossier, relancer la commande. La fenêtre propose toujours l'ancien dossier, et la validation navigue dans l'ancien volet.

- `src/ui/app/clipboard.rs:32` (face à 69-77) — moyen, plausible. Copier écrit le presse-papiers dans une tâche bloquante, coller le lit dans une autre. Rien n'ordonne les deux. Scénario : couper A, puis copier B sur une grosse sélection ou un partage lent, puis coller aussitôt. Le collage peut lire encore A avec son indicateur de coupe et déplacer A au lieu de copier B. Côté non-Windows, le texte de repli est écrit après la tâche, d'où la même course.

## Faible

- `src/ui/app/archive_password.rs:184` — faible, confirmé. Le texte saisi est extrait de l'état avant validation. Un mot de passe vide ou trop long (plus de 1024 octets) est refusé sans message ni effet. Le champ garde son contenu, mais l'état est vide, donc une nouvelle validation sans modification ne fait rien. Le message « trop long » existe pour la création d'archive mais pas ici.

- `src/ui/app/archive_password.rs:104` — faible, plausible. Un refus « mauvais mot de passe » arrivant d'ailleurs efface le mot de passe de l'archive, même si l'utilisateur vient de saisir le bon et qu'il est en cours de vérification. La vérification ne trouve alors plus de mot de passe, et son résultat n'est pas « mauvais » (ligne 195). Elle est donc prise pour un succès. L'invite se ferme, la reprise s'exécute sans mot de passe et l'invite revient. Scénario : un rafraîchissement ou une miniature de la même archive échoue pendant la saisie.

- `src/ui/app/archive_password.rs:176` (avec 187) — faible, confirmé. Le mot de passe est mémorisé avant d'être vérifié. Annuler pendant la vérification ferme l'invite, mais le mot de passe non vérifié reste en mémoire. Le prochain accès l'utilise, échoue et affiche directement « mauvais mot de passe » sans que l'utilisateur ait rien saisi.

- `src/ui/app/archive_password.rs:143` (aussi `archive_protection.rs:41` et `97`) — faible, plausible. Chaque frappe copie le mot de passe en clair dans des `String` d'état. L'extraction vers le secret ne les efface pas, et le clone de la ligne 97 non plus. Ces copies ne sont pas zeroizées alors que le magasin de secrets l'est. Elles restent dans le tas jusqu'à réutilisation.

- `src/ui/app/advanced_search/validation.rs:75` — faible, confirmé. Si le volet visé n'existe plus ou que le chemin est identique à celui de l'onglet actif, la navigation ne fait rien, sans message. Pourtant l'erreur est effacée. Scénario : fermer le volet d'origine, ou relancer exactement la même recherche pour l'actualiser.

- `src/ui/app/advanced_search/validation.rs:26` — faible, plausible. Une date valide est rejetée avec le message générique dans deux cas. Le premier : minuit local inexistant, comme lors de certains changements d'heure (Chili, Cuba). Le second : date antérieure à 1970 (conversion en non signé). L'utilisateur ne peut pas filtrer à partir de ces dates.

- `src/ui/app/advanced_search/calendar.rs:36` — faible, confirmé. Choisir une date écrit le champ par `set_content`, qui ne déclenche pas le rappel de changement. Le drapeau d'erreur n'est donc pas remis à zéro. Scénario : valider avec une date invalide, corriger via le calendrier, l'erreur reste affichée jusqu'à la validation suivante.

- `src/ui/app/advanced_search/calendar.rs:19` (et 29-32) — faible, confirmé. L'ouverture du calendrier ne tronque pas les espaces, alors que la validation le fait. Une date précédée d'un espace ouvre donc le calendrier sur aujourd'hui. Les pas de mois sur le jour 29 à 31 se ramènent au dernier jour du mois. Aller-retour : 31 janvier, février (28), mars (28), et le jour d'origine est perdu.

- `src/ui/app/advanced_search/help.rs:171` — faible, confirmé. L'ouverture du lien regex101 ignore l'erreur de `open_item`. Sans navigateur ou association, le clic ne produit aucun retour.

- `src/ui/app/clipboard.rs:29` — faible, confirmé. Copier ou couper prend la sélection dans un ensemble non ordonné. L'ordre dans le presse-papiers est donc aléatoire, et l'ordre de collage, comme l'attribution des « (2) », varie. La sélection peut aussi contenir des éléments masqués par un filtre ou disparus, alors que « copier le texte » (ligne 13) se limite aux fichiers filtrés.

- `src/ui/app/clipboard.rs:18` — faible, confirmé. Le nombre annoncé dans le toast vient du comptage de lignes du texte. Un nom de fichier contenant un saut de ligne (possible sous Linux) fausse le nombre.

- `src/ui/app/context_menus.rs:305` — faible, plausible. Après une commande « Nouveau » du menu natif, l'élément créé est deviné par la différence d'un instantané du dossier. S'il n'y a qu'un seul ajout, c'est le bon : si un autre processus crée un fichier au même moment (téléchargement, fichier temporaire), l'ajout unique peut être celui-là. Le renommage en ligne porte alors sur un fichier étranger. Le chemin est aussi converti avec perte (ligne 331) pour les noms non UTF-8 ou UTF-16 invalides.

- `src/ui/app/context_menus.rs:206` — faible, plausible. Le chargement du menu natif se fait dans une tâche bloquante sans délai ni annulation. Une extension shell qui se bloque (réseau) garde le fil de travail indéfiniment, même après la fermeture du menu. Le résultat tardif est ignoré, le fil reste occupé.

- `src/ui/app/context_menus.rs:85` — faible, plausible. La détection du sous-menu « Ouvrir avec » repose sur cinq libellés. Sur un Windows dans une autre langue, sans verbe canonique, le média du visualiseur n'est pas mis en pause avant l'ouverture externe.

- `src/ui/app/context_menu_render/native.rs:229` — faible, plausible. Un sous-menu est jugé déjà ouvert quand l'identifiant de son premier élément est celui du premier élément du sous-menu affiché. Windows renvoie -1 pour les éléments qui sont eux-mêmes des sous-menus. Deux sous-menus frères commençant par un tel élément sont confondus, et le second ne s'ouvre pas.

- `src/ui/app/archives.rs:85` — faible, confirmé. Le nom proposé pour l'archive remplace l'extension du premier élément. Un dossier « v1.2 » ou « rapport.final » devient « v1.zip » ou « rapport.zip ». Le nom est tronqué, et une archive existante portant ce nom pousse l'opération vers un nom « (2) ».

- `src/ui/app/archives.rs:80` — faible, plausible. La compression prend les éléments sélectionnés parmi tous les fichiers de l'onglet, filtre compris. Un fichier sélectionné puis masqué par un filtre est donc compressé sans être visible.

- `src/ui/app/archives.rs:192` — faible, confirmé. Les suffixes composés sont comparés en respectant la casse. « DOSSIER.TAR.GZ » produit un dossier d'extraction « DOSSIER.TAR ».

## Optimisation

- `src/ui/app/archive_password.rs:58` — optimisation, plausible. Chaque ouverture d'un 7z ou RAR sans mot de passe mémorisé, chiffré ou non, lance d'abord un listage complet en tâche bloquante, puis la navigation déclenche un second listage. Double lecture, et latence doublée sur un partage réseau ou une grosse archive.

- `src/ui/app/cards.rs:60` (et 84) — optimisation, confirmé. Chaque cran de molette Ctrl recalcule, sur le thread UI, les lignes d'affichage de chaque volet en grille deux fois, soit O(N) par volet. Il déclenche aussi une sauvegarde des préférences par cran, chaînées les unes derrière les autres.

- `src/ui/app/calendar.rs:6` — optimisation, confirmé. Une boucle perpétuelle se réveille toutes les 5 s. À chaque tour, deux tâches bloquantes lisent la référence calendaire, dont une signature de fuseau obtenue par la plateforme. Le travail utile n'a lieu qu'au changement de jour ou de fuseau.

- `src/ui/app/archives.rs:122` (aussi 18-28 et 46) — optimisation, confirmé. Résoudre chaque chemin sélectionné parcourt linéairement les fichiers de tous les onglets. L'extraction de S archives est en O(S × N) sur le thread UI. Une recherche par table de hachage suffirait.

- `src/ui/app/advanced_search/validation.rs:58` — optimisation, plausible. La validation compile les regex du nom et du contenu sur le thread UI à chaque validation, puis le worker les recompile. Un motif volumineux, dans la limite de 10 Mo par défaut, peut geler brièvement la fenêtre.

<!-- LOT TERMINÉ -->
