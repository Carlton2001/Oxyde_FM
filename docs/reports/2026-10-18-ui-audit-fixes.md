# Corrections de l’audit UI — 18 octobre 2026

Branche : `codex/ui-audit-fixes`, créée depuis `d1f912f`.
Les douze points de l’audit sont traités dans leur ordre, avec un commit de correction par point. Aucun push n’est effectué. Aucun curseur, tracé SVG, worker, traitement de fichiers ou catalogue de traductions n’est modifié.

## Changements et endroits précis à observer

Dans les paramètres ouverts par **Oxyde GPUI**, en haut à gauche, **Échelle de l’interface** est exprimée en pixels par rem : **16 px = 100 %**, **24 px = 150 %**, **32 px = 200 %**. Revenir à 16 px via la valeur centrale permet de comparer les dispositions.

| Point | Correction | Parcours visuel et résultat attendu |
| --- | --- | --- |
| 1 | Angles fixes des dégradés du fil d’Ariane. | Ouvrir un dossier profondément imbriqué puis utiliser un panneau assez étroit pour faire défiler le chemin. Passer de 16 à 24 puis 32 px et faire défiler le fil d’Ariane. Les fondus des extrémités restent horizontaux, orientés vers leur bord respectif. |
| 2 | Couleur du texte sur accent adaptée aux palettes lumineuses ; fond destructif distinct du texte d’erreur. | Paramètres → Thème : essayer Ayu Light/Dark, Monokai, One Light/Dark, GitHub Dark, Solarized Dark et Windows Dark. Ouvrir **Outils → Recherche avancée** : le libellé et l’icône du bouton **Rechercher** doivent rester lisibles. Même correction sur les actions principales de Renommer, Compression, Propriétés et Réseau. Si une confirmation de déconnexion forcée se présente, son action destructive a désormais un fond rouge plus sombre. |
| 3 | Texte secondaire renforcé en conservant sa hiérarchie. | Dans les thèmes Ayu, One, Monokai, Solarized et Windows, regarder les colonnes Type/Taille/Modifié de la liste, les segments parents du chemin, les onglets inactifs et les libellés des formulaires. Ils doivent être plus lisibles. Les fichiers masqués conservent leur atténuation intentionnelle. |
| 4 | Couleur sémantique de texte de succès, distincte du vert décoratif des notifications. | En thème clair, effectuer une copie de fichiers de test, ouvrir le panneau des opérations puis son détail. Examiner le statut de réussite et les badges de résultat. Dans un conflit entre deux fichiers de test de tailles différentes, le badge indiquant le plus volumineux doit aussi être plus lisible. |
| 5 | Même bordure réservée pour toutes les variantes de bouton. | **Propriétés** et **Recherche avancée** : comparer les hauteurs d’Annuler et de l’action principale. Passer de l’un à l’autre avec Tab/Maj+Tab : la bordure de focus doit apparaître sans déplacer ou agrandir le bouton. |
| 6 | Ascenseurs communs sur les surfaces auparavant défilables sans poignée. | À 24 ou 32 px dans une fenêtre basse : ouvrir les paramètres, leur liste de thèmes et leurs sous-menus ; vérifier aussi les Propriétés et les modales dont le contenu dépasse. Le rail apparaît au débordement ; la molette et le glissement de la poignée parcourent le même contenu. Dans les paramètres très étroits, vérifier aussi l’ascenseur horizontal. La liste des lecteurs des propriétés de la Corbeille bénéficie du même composant si elle dépasse sa hauteur maximale. |
| 7 | Focus visible sur les contrôles de la recherche avancée. | **Outils → Recherche avancée**, puis Tab/Maj+Tab : suivre successivement les onglets Paramètres/Aide, le sélecteur d’unité de taille et les deux boutons de calendrier. Le fond ou la bordure doit signaler le contrôle ciblé avant activation par Entrée/Espace. |
| 8 | Cadre standard partagé pour la lettre d’un lecteur réseau. | Clic droit sur **Réseau** dans la barre latérale → **Connecter un lecteur réseau**, sans valider l’opération. Comparer **Lettre** aux champs Dossier/Utilisateur/Mot de passe : même hauteur de 2,25 rem, même cadre et même bordure de focus. |
| 9 | Ombres des paramètres mises à l’échelle. | Ouvrir les paramètres puis le sous-menu Thème ou Langue à 16, 24 et 32 px. L’étendue et le décalage de leurs ombres suivent désormais la taille des menus, comme ceux des autres menus de sélection. |
| 10 | Hauteur maximale de recherche avancée, corps défilant. | Régler l’échelle à 32 px puis ouvrir **Outils → Recherche avancée** sur un écran de hauteur courante. Le formulaire peut défiler ; son titre et ses actions restent accessibles. Passer Paramètres → Aide → Paramètres et vérifier que le corps retrouve sa disposition. |
| 11 | Formulaires adaptés à la largeur en rems et pieds capables de changer de ligne. | À 24/32 px, ouvrir les Propriétés d’un fichier, d’une sélection et d’un raccourci : libellés et valeurs s’empilent quand la place manque. Dans Recherche avancée, les bornes Min/Max sont présentées verticalement et les options se replient. Les boutons restent entiers. Revenir à 16 px restaure les dispositions plus larges. La largeur native n’est pas agrandie artificiellement. |
| 12 | Pile de notifications limitée à l’espace utile et défilable. | Dans une fenêtre basse à 24/32 px, lancer plusieurs copies de fichiers de test dans des destinations distinctes. En bas à droite, les cartes restent dans l’espace disponible ; la molette et la poignée donnent accès à celles qui dépassent. Les cartes de progression continuent d’ouvrir le panneau des opérations. Les notifications ordinaires et leurs commandes de fermeture restent accessibles. |

Ces parcours sont une liste de vérification pour l’utilisateur ; ils ne sont pas présentés comme des essais visuels déjà effectués. La validation de ce changement reste sans Computer Use.

Après retour visuel sur « 1 chemin copié », la largeur fixe introduite au point 12 a été retirée des notifications ordinaires et du cadre de pile : leur largeur suit à nouveau le contenu, avec un maximum pour les messages longs. La fermeture utilise une marge automatique pour rester au bord droit. Les limites de hauteur et le défilement sont conservés.

## Historique des étapes

| Point | Commit / message |
| --- | --- |
| 1 | `b61762f` — Fix breadcrumb fade angles at every UI scale |
| 2 | `cd1bd96` — Improve filled action contrast across all themes |
| 3 | `01d2688` — Make secondary information readable in every theme |
| 4 | `5f7c2e2` — Separate readable success text from decorative green |
| 5 | `7c7cbc8` — Keep button geometry stable across variants and focus |
| 6 | `1264fa4` — Add shared scrollbars to constrained forms and settings |
| 7 | `2440ac0` — Show keyboard focus on advanced search controls |
| 8 | `ae6ac20` — Align the network drive picker with standard form fields |
| 9 | `cc86e35` — Scale settings menu shadows with the interface |
| 10 | `fdf4225` — Keep advanced search within the available screen height |
| 11 | `4f5c9be` — Reflow secondary forms and actions in narrow zoomed windows |
| 12 | Commit contenant ce rapport — Constrain notification stacks to the available viewport |

## Vérification

La compilation de toutes les cibles est passée sans avertissement du projet au fil des étapes. Les tests ciblés ont vérifié les couples de couleurs et la hiérarchie de texte dans les treize palettes, le texte des badges teintés, les bordures communes des boutons et les calculs de glissement des ascenseurs à plusieurs échelles. Un test supplémentaire vérifie la hauteur disponible pour les notifications.

L’exécution restreinte n’a pas constitué une validation native : trois tests de persistance de miniatures ont échoué, puis le script complet a échoué sur des opérations de handles/jonctions et a été interrompu. La relance hors de ce bac à sable, sur les fixtures privées du script, a résolu ces échecs.

Cette relance a rencontré une fois `interrupted_preparation_leaves_every_original_archive_unchanged` : l’annulation a renvoyé un rapport `ArchiveRecoveryFailure` inattendu. Le rejeu isolé exact a réussi. L’incident d’annulation correspondant était déjà signalé dans le [bilan du 17 octobre](2026-10-17-code-review.md) ; le code d’archives, de plateforme et son test n’ont aucune différence par rapport à `d1f912f`. Aucune correction d’archives n’est attribuée à cette branche UI, et un rejeu réussi ne clôture pas cet incident.

La passe finale de `./scripts/test-windows.ps1 -Offline` a réussi :

- `cargo check --offline --locked --all-targets` et construction de tous les binaires sans avertissement du projet ;
- **1 191 tests réussis, 21 ignorés, aucun échec**, en excluant du total les résultats des sous-processus qui rejouent un test déjà compté ;
- parmi eux, **840 tests de bibliothèque**, **287 tests du binaire** et **31 tests d’intégration du pipeline d’archives** réussis ;
- `git diff --check` sans erreur.

Journaux locaux : `target/ui-audit-native-unrestricted-tests.log`, `target/ui-audit-archive-retry.log` et `target/ui-audit-native-final-tests.log` ; ces fichiers ne sont pas versionnés.

Les parcours visuels réels, les multiples DPI et Linux restent à vérifier ; les tests de calcul et la compilation ne prouvent pas le rendu à l’écran.
