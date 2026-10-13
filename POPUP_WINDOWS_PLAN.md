# Plan autovalidé — fenêtres d'infobulles et de menus

Branche de travail : `codex/native-hover-preview`.

## Périmètre et décisions validées

Migrer les surfaces transitoires d'Oxyde vers des fenêtres GPUI possédées sous
Windows. Conserver le repli intégré sous Linux, où le mécanisme natif n'est pas
encore validé. Les dialogues autonomes et les panneaux permanents ne deviennent
pas des menus. Les textes, thèmes, icônes et actions existants restent réutilisés.

Le placement distingue explicitement le curseur, un point local de la fenêtre
appelante et les bounds d'un élément. Les menus ancrés ne doivent pas se mettre
à suivre la souris. Les coordonnées des sous-menus et callbacks doivent rester
dans le repère attendu par la vue propriétaire.

Les infobulles sont passives, sans focus, transparents aux clics, avec un seul
halo GPUI. Préserver les caches bornés, 24 px jusqu'au cadre visible, le délai
initial et les 150 ms de transition stable des aperçus. Une fenêtre réutilisée
par propriétaire évite de créer une fenêtre par événement souris.

Les menus sont interactifs : clic gauche, clavier, Échap, fermeture extérieure,
focus restauré uniquement quand approprié, sous-menus et défilement. Une action
doit viser sa vue propriétaire et ne pas être exécutée deux fois. Les menus du
shell gardent leurs requêtes et identifiants natifs existants.

Toutes les opérations Win32 restent derrière `Platform`. Les modifications
natives pouvant rappeler GPUI passent par des messages différés et regroupés.
Les propriétaires ferment leurs fenêtres transitoires avant leur destruction.
Aucun changement de curseur n'est ajouté.

## Inventaire à couvrir

- Infobulles de fichiers : Détails, Grille, corbeille, arborescence, réseaux,
  images, PDF, textes et vidéos.
- Infobulles ordinaires : boutons, raccourcis, onglets/chemins, lecteurs,
  recherches, doublons, opérations, préférences et propriétés.
- Menus de la fenêtre principale : fichiers/fond, sidebar, onglets, titre,
  actions système, sous-menus, filtres avec en-têtes et filtre nominal éditable.
- Menus ancrés : fil d'Ariane, rail des favoris, paramètres et sous-menus,
  listes de formulaires réseau et choix des propriétés de raccourci.
- Suggestions de chemins : menu cliquable ancré au champ, sans prendre son
  focus ni interrompre la saisie ; conserver sa navigation clavier existante.
- Menus secondaires : recherche avancée, propriétés, conflits et corbeille,
  y compris les menus contextuels des champs texte.

## Étapes et commits

1. Enregistrer ce plan et les critères ; finaliser les points d'entrée communs.
2. Généraliser les infobulles, avec repli, ancrage et cycles de vie vérifiés.
3. Ajouter les fenêtres interactives de menus, routage des actions et clavier.
4. Migrer les familles de menus principales puis les menus ancrés et secondaires.
5. Exécuter les tests ciblés, `cargo check` sans avertissement et la compilation.
6. Auditer le diff complet, corriger les problèmes, réexécuter les contrôles
   affectés et mettre à jour `UI_ARCHITECTURE.md` et les consignes applicables.

## Autovalidation et audit

Les étapes de développement sont réalisées. L’audit, les corrections, les
résultats des tests et les limites de validation sont conservés dans
[POPUP_WINDOWS_AUDIT.md](POPUP_WINDOWS_AUDIT.md). Les essais visuels et la
compilation Linux complète restent distincts des validations Windows obtenues.

La couverture est contrôlée par recherche des anciens points d'entrée de rendu,
pas seulement par compilation. Les tests doivent découvrir un nombre non nul
de scénarios : ancrage souris/élément, limites d'écran/DPI, transitions obsolètes,
cache, fermeture, chaîne de sous-menus et routage des callbacks.

L'audit final vérifie les références fortes, tâches/abonnements, destruction des
fenêtres, ordre focus/fermeture/action, activation entre sous-menus, coordonnées
des événements, dimensions/zoom, champs éditables, scrollbar, chargements
obsolètes, références d'icônes et repli Linux. Toute limite de validation GUI
ou de cible doit être signalée ; une compilation ne vaut pas un essai visuel.

Les validations autonomes autorisent chaque étape de développement et les
commits demandés. Les essais visuels natifs qui nécessitent l'utilisateur sont
rapportés séparément et ne sont pas présentés comme réalisés.
