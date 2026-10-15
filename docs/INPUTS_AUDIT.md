# Champs de saisie — inventaire et modèle partagé

État du code au 15 octobre 2026, après centralisation des champs GPUI.

## Constat

Les douze sites de construction utilisent déjà le même éditeur `TextInput` :
saisie, sélection de texte, presse-papier, menu contextuel, mot de passe,
défilement horizontal et complétion de chemins. Leurs cadres étaient cependant
répartis entre l'éditeur et les vues appelantes : hauteurs de 1,75 à 2,5 rem,
rayons différents, bordures locales et parfois superposées. Les paramètres
`minimal` et `compact` mêlaient l'affichage du bouton Effacer, les marges et
la présence d'une bordure. Un nouvel appel ne garantissait donc pas un champ
classique cohérent sans connaître les conventions de chaque fenêtre.

## Référence commune

`src/ui/components/input/field.rs` porte la présentation de référence, utilisée
par le champ autonome et par les champs composés :

- hauteur de 2,25 rem, texte de 0,8125 rem et interligne de 1,25 ;
- fond `surface`, texte `text`, bordure `border`, puis `accent` au focus ;
- bordure de 0,0625 rem, rayon `rounded_md`, marges horizontales de 0,75 rem
  pour l'éditeur ;
- largeur et disposition laissées au formulaire, avec `min_w_0` pour les textes longs.

Les dimensions suivent le zoom et les couleurs le thème courant. Le modèle
n'ajoute aucun changement de curseur ; les curseurs existants de l'éditeur sont
conservés. Les callbacks, entités, identifiants des interactions existantes,
menus et décisions métier restent dans les vues appelantes.

## Inventaire et rattachement

| Usage | Modèle retenu | Particularités conservées |
| --- | --- | --- |
| Renommage en dialogue et création d'archive (`app/rename.rs`) | `Standard` | Sélection initiale du nom, validation, erreur et actions du dialogue. |
| Connexion réseau (`app/network.rs`, `components/network_dialog.rs`) | `Standard` | Mot de passe masqué, bouton Effacer existant et remplacement par `read_only_field` pendant le traitement. |
| Propriétés des raccourcis (`app/properties/interactions.rs`, `shortcut.rs`) | `Standard` | Cible, arguments, dossier de départ et commentaire ; validation et menus existants. |
| Filtres de nom et d'emplacement (`app/filters.rs`, `context_menu_render/name_filter.rs`) | `Standard` | Menu flottant, largeur calculée selon le placeholder et callbacks de filtre. |
| Recherche avancée : nom, dossier et contenu (`app/advanced_search/`) | `field_frame` + `Embedded` | Préfixes regex/dossier, police monospace selon le mode et bouton Effacer. |
| Recherche avancée : tailles et dates | `field_frame` + `Embedded` | Bornes, unité, calendrier et navigation clavier. Les cadres locaux sont supprimés. |
| Filtre des doublons (`app/duplicates/`) | `field_frame` + `Embedded` | Loupe, filtrage et bouton Effacer. L'ancien cadre de 2,5 rem rejoint le modèle. |
| Filtre des conflits (`components/conflict_dialog/`) | `field_frame` + `Embedded` | Loupe à droite et filtre. Suppression du cadre local. |
| Filtre des éléments d'une opération (`components/operations/files/window.rs`) | `field_frame` + `Embedded` | Loupe, filtrage en arrière-plan et bouton Effacer. |
| Taille maximale de corbeille (`app/trash_settings/`) | `field_frame` + `Bare` | Contrôle composé avec unité ; désactivation et valeur non éditable existantes. L'ancien cadre de 1,75 rem rejoint le modèle. |
| Renommage dans la liste ou la grille (`app/rename.rs`, `components/file_list/`) | `Inline` | Hauteur et typographie de la ligne, sélection initiale, validation extérieure et retour d'erreur spécifiques. |
| Édition du fil d'Ariane (`app/navigation.rs`, `components/top_bar.rs`) | `Inline` | Hauteur de navigation, suggestions, bouton de validation et animation d'erreur. |
| Recherche rapide (`app/search.rs`) | `Bare` | Champ intégré à la navigation, animation, déclenchement différé, arrêt/effacement externes. |

Les listes déroulantes, calendriers, steppers et valeurs simplement affichées
ne sont pas des éditeurs texte et conservent leurs composants dédiés. Seule la
partie éditable d'un contrôle composé relève de ce modèle.

## Ajouter un champ

Importer `TextInput` et `TextInputOptions` depuis `components::input`. À l'appel
de `TextInput::new`, fournir les callbacks existants, puis :

```rust
TextInputOptions::default(),
window,
```

Le résultat est un champ classique complet : l'ajouter directement au
formulaire, sans recréer sa hauteur, son fond, sa bordure ou ses marges.
Utiliser `modal::form_row` pour une ligne libellé/champ. Le bouton Effacer est
une option explicite :

```rust
TextInputOptions {
    clearable: true,
    ..Default::default()
}
```

Pour un champ avec préfixe ou suffixe, créer l'éditeur avec
`appearance: InputAppearance::Embedded`, puis composer son cadre avec
`field_frame(&input, window, cx)`. Placer l'éditeur dans un conteneur
`flex_1().min_w_0().h_full()` ; ajouter les icônes ou commandes au même cadre.
Le cadre porte la bordure et son état de focus ; l'éditeur n'en dessine pas une
seconde. Ne pas découper le cadre avec `overflow_hidden` si les suggestions
doivent déborder dans le repli intégré.

`Bare` est réservé aux éditeurs dont le parent définit déjà les marges ;
`Inline` aux éditions qui doivent suivre la géométrie du contenu environnant.
Une nouvelle fenêtre de formulaire utilise `Standard` par défaut. Le masquage
du mot de passe et la complétion restent indépendants de la présentation.

## Organisation et validation

- `input/options.rs` : variantes et options nommées, sans état métier.
- `input/field.rs` : surface commune, cadre composé et valeur non éditable.
- `input/render.rs` : assemblage de l'éditeur et de ses interactions existantes.
- `input.rs` : état, saisie, sélection et élément de texte bas niveau.
- `input/tests/field.rs` : égalité des surfaces autonome/composée, focus sans
  changement de géométrie et absence de bordure ajoutée par les éditeurs intégrés.

La centralisation ne change pas les workers ni les opérations système.
Vérifications exécutées sous Windows : `cargo check --offline` sans avertissement,
`cargo test --offline --bin oxyde ui::components::input::` avec 7 tests réussis
(dont l'égalité des surfaces et les couleurs de focus dans les 13 thèmes), et
`git diff --check` sans erreur.
La compilation et les tests de style ne remplacent pas les essais visuels :
vérifier les thèmes, le zoom, les textes longs, le focus clavier/souris, les
champs réseau occupés et les suggestions de chemins dans l'application.
