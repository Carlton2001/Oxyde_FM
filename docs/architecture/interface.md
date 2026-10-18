# Conventions de l’interface

L’état métier et les actions appartiennent à `src/ui/app/`. Les composants reçoivent données et callbacks ; ils ne possèdent que leur état local de focus, défilement ou expansion. `src/ui/elements/` fournit les primitives. Le [guide de développement](../guides/development.md) décrit le découpage.

## Thèmes et dimensions

- Obtenir le thème par `cx.theme()` et utiliser les jetons d’`OxydeTheme`. Ajouter un jeton dans toutes les palettes si un rôle partagé manque ; réserver les valeurs locales aux effets propres au composant.
- Les treize thèmes sont déclarés par `ThemeId::ALL`. Les couleurs de fichiers sont centralisées dans `components/icons/colors.rs` et appliquées par `render_file_icon`.
- Exprimer les dimensions en `rems()`, référence initiale 16 pixels logiques. Utiliser `ui::scale::rem_pixels` pour une API en `Pixels`, `logical` pour les anciennes valeurs à l’échelle 16. Ces conversions restent sur le thread UI.
- Les coordonnées fournies par GPUI sont déjà en pixels. Ne pas les convertir une seconde fois. Le zoom du contenu ne redimensionne pas la fenêtre native.
- Le poids par défaut est Regular (400), depuis `ui/style.rs`. Les racines rendues séparément l’appliquent aussi. Un autre poids doit exprimer une variation intentionnelle.

## Langues et formats

`cx.i18n()` résout les variantes de `Message`. Pour tout texte utilisateur, ajouter ou réutiliser une clé et ses traductions dans `locales/{fr,en,de,es,it}.ftl`, puis vérifier les catalogues et paramètres.

Garder les phrases entières dans Fluent, avec des paramètres numériques pour les pluriels. Les chemins et noms utilisateur restent des données littérales ; retirer les isolats bidirectionnels seulement si le résultat devient un chemin système. Les erreurs persistantes gardent leur clé afin de suivre un changement de langue. Les détails techniques restent dans les logs.

`ui/format.rs` centralise nombres, tailles et dates. La langue et le format de date persisté sont distincts de la locale régionale système. Le repli des messages est français ; un échec de formatage est journalisé.

## Réutiliser les composants

| Besoin | Référence |
| --- | --- |
| Bouton | `elements/button/style.rs` : `button_style`, `ButtonVariant` |
| Formulaire ou modale | `components/modal.rs` : `Modal`, `form_row`, `action_button` |
| Fenêtre secondaire et pied de page | `components/tool_window` ; [contrats des fenêtres](windows.md) |
| Champ classique | `TextInput::new` avec `TextInputOptions::default()` |
| Champ composé | `input::field_frame` avec `InputAppearance::Embedded` |

Les vues gardent dimensions, identifiants et callbacks. Pour un bouton désactivé, retirer ou protéger son callback : le style seul ne suffit pas. Les modales sont déplaçables par leur titre, sans fond assombri ; leur couche transparente bloque les interactions derrière elles et réserve la barre de statut.

Le champ standard fournit hauteur 2,25 rem, texte 0,8125 rem, interligne 1,25, fond `surface`, bordure `border` puis `accent` au focus, rayon `rounded_md` et marges horizontales 0,75 rem. Ne pas recréer ce cadre dans chaque formulaire. `clearable: true` active explicitement Effacer.

Un champ composé place l’éditeur dans `flex_1().min_w_0().h_full()` et les commandes dans le même cadre, sans seconde bordure. `Bare` laisse les marges au parent ; `Inline` suit la géométrie du contenu pour le renommage et le chemin. Ne pas masquer les suggestions avec `overflow_hidden` dans le repli intégré.

## Préférences et réactivité

### Ascenseurs

`components/scrollbar.rs` possède le rendu commun : fond de rail permanent
(`scrollbar_track`), poignée, états de survol/glissement et marges aux extrémités.
Ne pas masquer le fond au repos ni ajouter une apparence locale par liste.
`TrackShape` adapte uniquement le contour au parent ; les menus de sélection
fournissent leur rayon intérieur via `select_menu`. Leur rail peut être plus long
que le viewport, sans modifier la plage de défilement du contenu. Conserver les
propriétaires des événements, identifiants et conversions de coordonnées.

Pour les formulaires, modales, paramètres et notifications, `components/scroll_area.rs`
compose ces ascenseurs avec un état local par surface et par fenêtre. La vue
appelante conserve le cadre, les contraintes et l’identifiant du viewport.
Les poignées réservent leur place uniquement en cas de débordement ; les deux
axes gardent leurs coordonnées et captures dans leur fenêtre propriétaire.

Les formulaires secondaires adaptent leur disposition à la largeur disponible
en rems : `modal::responsive_form_row` empile un libellé et sa valeur si la vue
le demande. Les pieds de fenêtre peuvent passer sur plusieurs lignes. Le zoom
ne doit pas déclencher un agrandissement de la largeur native pour compenser
une mise en page trop rigide.

`AppConfig` conserve les réglages ; les globals appliquent immédiatement thème et langue. La file de sauvegarde écrit en arrière-plan et dans l’ordre. Garder les anciennes configurations lisibles avec des valeurs par défaut ; ne pas écraser une configuration illisible. La réinitialisation conserve session, géométrie et préférences de doublons.

Le rendu ne fait ni I/O ni calcul lourd. Réutiliser les résumés d’historique/opérations et les instantanés préparés par révision. Les réponses asynchrones doivent vérifier leur propriétaire et leur révision avant publication. Voir [opérations](operations.md) et [validation visuelle](../guides/visual-checks.md).
