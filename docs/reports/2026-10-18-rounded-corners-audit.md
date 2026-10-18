# Audit des coins arrondis — 18 octobre 2026

## Portée et preuve

Audit statique des arrondis de l'interface, sans utiliser l'origine de la capture
comme critère de recherche. Inventaire : 128 occurrences de `rounded` ou
`corner_radii`, dans 52 fichiers sous `src/`. Recherche complémentaire des
dessins directs (`paint_quad`, `PathBuilder`, etc.). Aucun essai visuel en session
Windows. Les corrections apportées après cet audit sont décrites ci-dessous.
Cet audit ne certifie donc pas
l'absence de défauts de rastérisation, d'anticrénelage ou liés au DPI.

La dépendance GPUI réellement utilisée a été inspectée dans le cache Cargo,
révision `33c9585`, `crates/gpui/src/style.rs`, méthode `overflow_mask` (ligne 638).
Le masque d'overflow ne contient que des bounds rectangulaires. Les rayons du
parent ne découpent pas ses enfants. Ajouter `overflow_hidden()` ne suffit pas
à corriger un enfant carré dans un cadre arrondi.

## Défauts géométriques identifiés

Ces constructions sont présentes dans le code ; leur visibilité exacte selon
le thème, le zoom et le DPI reste à vérifier dans l'application.

| Surface | Source | Construction et déclencheur |
| --- | --- | --- |
| Suggestions de chemins | `src/ui/components/input/completion_menu.rs:20` | Lignes sans rayon ni marge dans un menu `rounded_sm`. La première ou dernière ligne sélectionnée/survolée peut peindre les coins intérieurs. Vérifier aussi après défilement. |
| Histogrammes de taille | `src/ui/components/file_list/presentation.rs:235` et `grid_card.rs:67` | Fond absolu carré, attaché aux bords, dans une carte `rounded_md`. Le bord gauche est concerné ; à remplissage complet, le droit aussi. Le helper est partagé avec les lignes de détails, qui n'ont pas nécessairement le même besoin de rayon. |
| Tableau des emplacements de la Corbeille | `src/ui/app/trash_settings/render.rs:168` | Cadre `rounded_md` avec overflow rectangulaire. En-tête au fond carré en haut et lignes au fond carré en bas, notamment lors de la sélection et du survol. |
| Sources de la recherche de doublons | `src/ui/app/duplicates/sources.rs:44` et `:131` | Cadre arrondi, mais toutes les lignes ont un fond carré. Première et dernière lignes peuvent remplir les coins intérieurs malgré `overflow_hidden`. |
| Choix de lettre réseau | `src/ui/components/network_dialog/letters.rs:20` et `:78` | Cadre `rounded_md` sans marge interne protégeant les coins ; lignes de sélection/survol carrées. Défilement et barre de défilement à inclure dans la vérification. |
| Occupation d'un lecteur dans Propriétés | `src/ui/app/properties/general.rs:244` | Piste `rounded_sm`, remplissage carré de pleine hauteur. Coins gauches carrés dès que le remplissage est suffisant, coins droits à pleine capacité. |
| Progression compacte des opérations | `src/ui/components/operations/compact.rs:67` | Piste `rounded_full`, remplissage carré. Le reflet animé de finalisation utilise aussi des rectangles qui traversent les extrémités. |

## Points sensibles à confirmer, sans les classer comme défauts établis

- **Modales intégrées**, `src/ui/components/modal.rs:245–380` : le corps a un fond
  carré, le pied arrondi est conditionnel. Sans actions, le bas du corps peut
  remplir les coins. Les appels actuels trouvés ajoutent des actions ; ne pas
  présenter cette possibilité comme un défaut visible de tous les dialogues.
  Les modales standalone suivent une autre branche de rendu.
- **Cartes d'opérations et groupes de doublons**, `operations/card.rs:95–205`
  et `app/duplicates/results.rs:83–110` : les en-têtes possèdent déjà des rayons
  adaptés au cadre (9 contre 10 pixels à l'échelle de référence). Ce n'est pas
  un débordement carré évident. Contrôler surtout le focus, l'état replié,
  l'anticrénelage et les facteurs d'échelle fractionnaires.
- **Réglages et épingles**, `components/settings.rs:206–291` et
  `components/sidebar/pinned.rs:40–80` : marges et rayons des lignes protègent
  les coins au repos. Le défilement peut amener une coupe rectangulaire de ligne
  dans une zone arrondie. À confirmer aux limites haute et basse.
- **Progression détaillée**, `operations/card/metrics.rs:151–165` : remplissage
  déjà arrondi. Vérifier les très faibles pourcentages ; ne pas lui appliquer
  automatiquement le diagnostic de la progression compacte.

## Autres familles examinées

Boutons et commandes de barre d'outils, steppers, champs simples et composés,
recherche rapide et avancée, onglets et onglets glissés, conflits, formulaires
réseau, contrôles des réglages, badges et cases à cocher, notifications,
infobulles et aperçus, propriétés et fenêtres secondaires, scrollbars.

Aucun autre débordement de fond carré n'a été établi par cet examen. Un rayon
sans `overflow_hidden` n'est pas à lui seul un défaut : un contenu suffisamment
en retrait peut être correct. Les bordures fines et les ombres exigent une
inspection visuelle, en particulier avec des couleurs translucides.

## Vérification visuelle à effectuer

Tester les sept constructions identifiées en thèmes clair et sombre contrastés,
à zoom normal puis fractionnaire, avec sélection, survol et focus clavier.
Pour les listes, vérifier les deux extrémités et une position intermédiaire de
défilement. Pour les jauges, vérifier faible remplissage, mi-course, plein et
finalisation animée. Observer séparément bordure extérieure, fond du parent et
fond de l'enfant. La capture fournie ne permet pas d'attribuer un composant ni
de conclure à un défaut du moteur de rendu partagé.

## Corrections appliquées

- Jauges d'occupation et progression compacte, histogrammes en grille : dessin
  d'un quad aux dimensions de la surface arrondie, découpé à la fraction exacte
  du remplissage. Le dessin de grille reste à l'intérieur de sa bordure ; les
  histogrammes des lignes de détails conservent leur géométrie rectangulaire.
- Reflet animé de finalisation : confinement dans la partie droite de la piste,
  entre les deux extrémités courbes.
- Suggestions, lettres réseau, réglages et épingles : défilement dans une zone
  intérieure en retrait des coins. Les handles de défilement et les callbacks
  sont conservés. La scrollbar réseau est placée dans le même repère intérieur
  que la liste ; sa hauteur de viewport est ajustée.
- Sources de doublons : les sélections remplissent les lignes sans marge
  intérieure. La première et la dernière ligne reprennent les rayons intérieurs
  du cadre (0,3125 rem).
- Corbeille : rayon intérieur de l'en-tête et de la dernière ligne, sans retrait
  inférieur ; l'en-tête arrondit aussi son bas lorsque la liste est vide.
- Jauge d'occupation : rayon du remplissage aligné sur `rounded_sm`, soit
  0,25 rem (4 pixels à l'échelle de référence), après correction d'un rayon
  initial de 0,125 rem qui rendait le côté rempli moins arrondi.
- Modale intégrée sans actions : le corps reprend les rayons inférieurs.
- Cartes d'opérations et groupes de doublons : les rayons des en-têtes étaient
  déjà ajustés ; aucune modification arbitraire de ces rayons.

Validation : `cargo check --offline --locked --all-targets` réussi sans
avertissement ; test existant
`pinned_menu_aligns_with_outer_rail_edge_at_every_scale` réussi (1 test exécuté).
Ces vérifications ne constituent pas une validation visuelle du rendu.
