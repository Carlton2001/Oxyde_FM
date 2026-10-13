# Audit — fenêtres d’infobulles et de menus

Date : 13 octobre 2026. Branche : `codex/native-hover-preview`.
Plan : [POPUP_WINDOWS_PLAN.md](POPUP_WINDOWS_PLAN.md).

## Couverture de l’implémentation

| Famille | Transport Windows | Référence |
| --- | --- | --- |
| Infobulles ordinaires, raccourcis, boutons, opérations, recherches, propriétés | `NativeTooltipExt` et proxy GPUI | Curseur ou bounds explicites |
| Fichiers, images, PDF, vidéos, arborescence, lecteurs et réseaux | Session de survol et fenêtre passive réutilisée | Curseur, avec délais et caches existants |
| Fichiers/fond, Shell, sidebar, onglets, titre et filtres | Factory de `context_menu_render` | Point source figé ; lignes réelles pour les sous-menus |
| Fil d’Ariane | Surface ancrée et `GenericDropdown` | Bounds du bouton chevron |
| Suggestions de chemins | Factory de complétion, interactive sans activation | Bounds du champ ; focus et clavier conservés dans le champ |
| Rail des favoris | Factory des favoris | Bouton du rail |
| Paramètres, thème, langue, date, compression | Factories du panneau et des sélecteurs | Bouton principal et bounds de la ligne |
| Lettres de lecteur réseau | Factory des lettres | Bounds du champ |
| Mode d’exécution des raccourcis | Factory des trois choix | Bounds du contrôle |
| Menus texte de recherche avancée, propriétés, conflits et corbeille | `popup::text_menu` | Clic dans la fenêtre secondaire propriétaire |

La recherche des appels `.tooltip(` dans `src/` ne conserve que les deux branches de l’adaptateur. Les anciens rendus `anchored` de menus restent pour Linux et pour le repli Windows en cas d’échec ; ils ne sont pas des cas oubliés. Les modales de confirmation et calendriers restent des dialogues, hors transport des menus.

## Problèmes relevés et corrections

1. **Focus et actions** : les callbacks recevant un `Window` pouvaient agir sur la fenêtre de menu au lieu de la vue source. Le routage commun vise le propriétaire d’origine ; réglage d’échelle, confirmation et menus texte imbriqués suivent ce contrat. Le retour de focus distingue le parent logique d’un sous-menu et la source métier, et préserve un nouveau focus posé par une action. Aucun retour n’est demandé si une autre application/fenêtre indépendante est active.
2. **Coordonnées** : favoris, champs texte, hover des lignes et scrollbars utilisent le repère source après conversion DPI. Les sous-menus sont placés sur les lignes réellement peintes, séparateurs et défilement compris. L’absence momentanée de ligne survolée ne remplace plus un ancrage latéral valide par un point approximatif. La géométrie des lignes est republiée lorsqu’elle change.
3. **Événements** : seuls mouvements et relâchements nécessaires aux glissements sont retransmis au propriétaire. Les clics restent des callbacks explicites, sans clic synthétique supplémentaire. La propagation des événements du menu est rétablie après cette retransmission. Les modificateurs initiaux et leurs changements sont synchronisés ; Alt+Entrée ne devient pas une activation de ligne ordinaire. Les champs éditables gardent leur traitement local.
4. **Fermeture** : les fenêtres secondaires et les menus retirent leurs descendants et infobulles avant la destruction du propriétaire, y compris les sorties par boutons, libération du modèle et fermeture système. Le nettoyage respecte le refus de fermeture d’un dialogue occupé : ses surfaces ne sont pas retirées avant cette décision. Les registres sont nettoyés à la fermeture, avec suppression des tâches, références de lignes et callbacks clavier. La fermeture utilise la spec actuelle du menu réutilisé.
5. **Demandes obsolètes** : les ouvertures en attente conservent la demande la plus récente ; une demande annulée ne bloque plus sa remplaçante. La fermeture annule les demandes différées. La destruction vérifie encore l’état actuel au moment d’exécuter le retrait, et l’existence de la fenêtre source avant création.
6. **Clavier** : ajout de navigation circulaire, début/fin, activation, sous-menus et retour. Les mêmes actions sont disponibles pour les sélecteurs des paramètres, compression, favoris, lettres et modes de raccourci. Les lignes désactivées sont exclues ; la sélection clavier cède au mouvement du pointeur. Les raccourcis métier sont retransmis quand le menu porte le focus.
7. **Hit testing et glissements** : la surface des menus est interactive, leur marge d’ombre est transparente aux clics et les infobulles restent entièrement passives. La capture du pointeur termine les glissements hors de la fenêtre. Les scrollbars de menus simultanés ont des identifiants distincts.
8. **Erreurs et dimensions** : rejet des ancrages non finis/négatifs ; bornage sur la zone de travail réelle. Un échec de création/configuration conserve l’état et réactive le rendu intégré du propriétaire. Les bounds de l’ancienne surface native sont effacés dans ce repli. Le rem suit le propriétaire et la première fenêtre est créée près de lui.
9. **Structure et conventions** : extraction du choix des lettres, du menu de fil d’Ariane et de la mesure du cadre. Les APIs Win32 restent dans `Platform` et ses modules Windows. Aucun SVG, texte produit ou changement de curseur n’a été introduit. Les nouveaux tests sont dans les sous-répertoires de leur module.
10. **Suggestions de chemins** : le contrôle d’inventaire final a repéré le menu manuel des champs texte. Son rendu est extrait et transporté dans une fenêtre interactive sans activation, afin de conserver le focus, les actions Haut/Bas/Tab et la saisie dans le champ. La géométrie du champ sert d’ancrage ; la liste défile pour révéler la sélection. Une perte de focus ou un changement d’application retire les suggestions et annule les résultats en attente.

Les caches d’images/PDF, annulations vidéo et délais de survol ne sont pas remplacés par le transport. La politique de déplacement reste différée et regroupée afin d’éviter les callbacks GPUI synchrones pendant un emprunt de `App`/`Window`. La sortie finale du processus n’a pas été modifiée.

## Vérifications réalisées

- `cargo check --offline` : succès, aucun avertissement du projet.
- `cargo test --offline --bin oxyde ui:: -- --test-threads=1` : **208 réussis, 1 ignoré**, dont les tests de caches, placement et transitions d’infobulles.
- Après la dernière correction clavier, `cargo test --offline --bin oxyde ui::components::popup -- --test-threads=1` : **1 réussi**, filtre non vide.
- `cargo test --offline --lib windows_hover_window::tests -- --test-threads=1` : **11 réussis**. Ces tests créent de vrais handles Win32 et vérifient passivité, ownership, styles, fermeture parent/enfant, regroupement des placements, offset visible, conversion de coordonnées, ancrage d’élément et transparence de l’ombre, ainsi que l’interactivité sans activation des suggestions.
- `cargo build --offline` : exécutable de développement reconstruit.
- `git diff --check` et inventaire des points d’entrée : vérifiés.

## Limites et essais encore à réaliser

La compilation Linux a été tentée avec `cargo check --offline --target x86_64-unknown-linux-gnu`. Elle s’arrête dans `yeslogic-fontconfig-sys` : l’hôte Windows ne dispose pas du sysroot et de la configuration `pkg-config` pour cette cible. Le code de repli et les conditions de compilation ont été relus ; la compilation Linux complète reste non vérifiée.

L’environnement ne permet pas de piloter visuellement l’application native. Les tests Win32 ne remplacent pas une session Oxyde réelle. Restent à essayer : transfert rapide entre menus et sous-menus, filtres éditables et presse-papiers, scrollbar hors surface, ouverture des propriétés/confirmations depuis un menu, Alt+Tab, fermeture avec aperçus actifs, écrans multiples à DPI différents, zoom 12–24 et thèmes clairs/sombres. L’injection d’un échec de création GPUI n’a pas été exécutée ; son repli a été audité dans le code.

Aucun autre défaut certain n’a été identifié après les corrections et contrôles ci-dessus. Cette conclusion couvre le code et les tests exécutés, pas les essais visuels manquants.

## Correction après essai utilisateur — 13 octobre 2026

L’ouverture réelle des menus a révélé un plantage dans `Window::draw` : l’initialisation des modificateurs appelait `dispatch_event` dans le constructeur d’`open_window`, alors que `Window::root` était encore absent. L’événement est maintenant envoyé après installation de la racine via `AnyWindowHandle::update`, sans emprunt typé de la racine pendant le dessin. Les tests Win32 de la couche plateforme ne pouvaient pas découvrir ce défaut du cycle de vie GPUI.

Les infobulles simples et fixes perdaient également l’origine de prépaint donnée par leur cadre natif : `CursorTooltip` remettait le contenu à `(0, 0)` et décalait ainsi le cadre visible de la marge d’ombre. Il conserve maintenant l’origine de ses bounds ; le placement intégré et les aperçus gardent leurs chemins existants.

`cargo check --offline` est sans avertissement ; les cinq tests de `tooltip::position` passent. Ils vérifient la géométrie historique conservée ; le rendu natif corrigé et l’ouverture des menus demandent encore confirmation dans une session graphique réelle. L’exécutable est reconstruit après correction.

### Isolation des événements de pointeur

L’essai utilisateur suivant a confirmé l’amélioration des positions et révélé que les éléments sous les menus réagissaient à la souris. La retransmission synthétique des mouvements et relâchements à `Window::dispatch_event` relançait le hit testing de la fenêtre source. Elle est supprimée : les menus appellent directement les gestionnaires de défilement, limités aux glissements `ContextMenuV` et `BreadcrumbV`. Les événements locaux du menu continuent à circuler ; les gestionnaires locaux des lettres réseau sont conservés. Un test de non-régression vérifie que ces gestionnaires excluent les glissements de la liste de fichiers et de la sidebar. La vérification visuelle reste à confirmer dans Oxyde.

### Cache et délai de transition — 14 octobre 2026

Le délai de 150 ms était imposé à tous les remplacements et redémarrait à chaque item, même avec un cache hit. Cela conservait inutilement la bulle précédente lors d’un parcours rapide. Un asset GPUI déjà résolu est maintenant publié immédiatement ; les workers de cache images/PDF transmettent leur statut de réutilisation après validation du fichier. Un cache hit prêt contourne les 150 ms, tandis qu’une nouvelle génération conserve la grâce. Aucun résultat incomplet ou issu d’un survol obsolète n’est publié. La vérification asynchrone et le prochain contrôle de publication peuvent encore prendre quelques millisecondes ; aucune lecture de fichier n’est ajoutée au thread UI.

Les 36 tests d’infobulles passent, dont les nouveaux scénarios cache immédiat, résultat pas encore prêt, génération avec grâce et source modifiée invalidant le cache. `cargo check --offline` reste sans avertissement. Le délai initial et la tolérance entre éléments ne sont pas modifiés.
