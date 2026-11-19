# Audit UI-REST-01

## Fichiers lus en entier

- [x] src/ui/assets.rs
- [x] src/ui/audio_covers.rs
- [x] src/ui/audio_covers/tests.rs
- [x] src/ui/date_groups.rs
- [x] src/ui/date_groups/families.rs
- [x] src/ui/date_groups/grouping.rs
- [x] src/ui/elements/bar_button.rs
- [x] src/ui/elements/button.rs
- [x] src/ui/elements/button/style.rs
- [x] src/ui/elements/checkbox.rs
- [x] src/ui/elements/color_picker.rs
- [x] src/ui/elements/focus_ring.rs
- [x] src/ui/elements/mod.rs
- [x] src/ui/elements/motion.rs
- [x] src/ui/elements/overlay.rs
- [x] src/ui/elements/progress_bar.rs
- [x] src/ui/elements/radio.rs
- [x] src/ui/elements/resize_separator.rs
- [x] src/ui/elements/rounded_fill.rs
- [x] src/ui/elements/segmented.rs
- [x] src/ui/elements/separator.rs
- [x] src/ui/elements/stepper.rs
- [x] src/ui/elements/toggle.rs
- [x] src/ui/errors.rs
- [x] src/ui/file_types.rs
- [x] src/ui/format.rs
- [x] src/ui/format/permissions.rs

Bilan : aucun défaut critique, élevé ou moyen. Les primitives graphiques (checkbox, radio, toggle, segmented, separator, focus_ring, motion, overlay, rounded_fill, progress_bar, bar_button, permissions) n'appellent aucune remarque : pas d'état partagé, pas d'indexation risquée, les fractions sont bornées et les appelants vérifiés (opérations, propriétés) ne peuvent pas fournir de valeur non numérique.

## Optimisations

1. `src/ui/audio_covers.rs:14-32` (et 54-59) — optimisation, plausible. Le pool de partage de pixels garde son verrou global pendant le hachage de tout le tampon d'image, puis parcourt jusqu'à 4096 entrées faibles à chaque nouvelle image. Scénario : défilement d'une grille de musique dont chaque pochette est unique ; tous les threads de chargement se sérialisent sur ce verrou, et une pochette de plusieurs mégaoctets en survol ajoute son temps de hachage au blocage.

2. `src/ui/date_groups.rs:24-29` (appelé à chaque rendu par `src/ui/workspace/presentation.rs:26`) — optimisation, plausible. La référence calendaire interroge le système (fuseau dynamique de Windows) et construit une chaîne à chaque rendu quand le regroupement est calendaire, sur le thread UI, alors que `src/ui/app/calendar.rs` la confie à un thread bloquant ailleurs. Scénario : dossier regroupé par date, chaque image rendue (défilement, survol) refait l'appel système avant même de tester le cache.

3. `src/ui/date_groups/grouping.rs:163-198` et `src/ui/date_groups/families.rs:38-67` — optimisation, confirmé. Le regroupement complet est recalculé hors du cache de présentation par `type_select.rs:41,57` (à chaque frappe), `keyboard_menu.rs:37`, `cards.rs:77,115`, `workspace/tab/scroll.rs:36` et `workspace/tab.rs:83`, avec jusqu'à trois allocations de nom en minuscules par fichier en regroupement par type (plus celles de la détection de code). Scénario : dossier de 100 000 fichiers regroupé par type, saisie rapide au clavier : chaque touche reparcourt et réalloue tout sur le thread UI.

## Faible

4. `src/ui/audio_covers.rs:25-29` — faible, confirmé. Quand le pool atteint 4096 entrées vivantes, il supprime une clé arbitraire, éventuellement encore utilisée. Une pochette identique chargée ensuite n'est plus partagée et sa mémoire est dupliquée (pas de corruption). Scénario : plus de 4096 pochettes distinctes tenues par des cartes et le cache disque.

5. `src/ui/date_groups/grouping.rs:178` — faible, plausible. Le jour de référence du regroupement est relu indépendamment de celui utilisé pour valider le cache de présentation. Scénario : le minuit passe entre les deux lectures, les lignes sont groupées avec le nouveau jour mais mémorisées avec l'ancien, jusqu'au rendu suivant qui reconstruit tout (auto-corrigé, un seul rendu incohérent).

6. `src/ui/date_groups/grouping.rs:121-123` — faible, confirmé. En regroupement par date, une date de modification absente (zéro, affichée comme un tiret dans la liste) tombe dans « Plus ancien », alors que les modes création et suppression la rangent dans « date inconnue ». Scénario : fichier ou entrée de corbeille sans mtime, regroupé par date de modification.

7. `src/ui/date_groups/grouping.rs:141-147` — faible, plausible. La clé de regroupement par origine est la chaîne brute du dossier parent, sans normalisation. Scénario : sous Windows, éléments de corbeille dont l'origine est notée avec une casse différente (« C:\Users » et « c:\users ») ou avec ou sans préfixe étendu ; ils forment des sections distinctes.

8. `src/ui/format.rs:69-85` — faible, confirmé. L'analyse d'une taille saisie accepte un signe plus devant la partie entière ou décimale, l'analyseur d'entiers non signés l'autorisant. Scénario : saisie « 1,+5 » dans un filtre de taille, acceptée comme 1,05 au lieu d'être rejetée.

9. `src/ui/format.rs:155-160` — faible, plausible. L'analyse d'un jour saisi accepte une année sur un ou deux chiffres (le motif d'année de chrono n'impose pas quatre chiffres) et la lit comme l'an 26. Scénario : saisie « 01/02/26 » dans un filtre de date, qui devient 0026-02-01 sans erreur et sélectionne tout ou rien.

10. `src/ui/format.rs:12-25` — faible, plausible. Le nom d'un volume sans libellé dont la taille est inconnue retire les séparateurs finals du chemin ; pour la racine sous Linux le chemin devient vide et l'on affiche un nom du type « Disque local () ». Scénario : racine « / » sans libellé et volume_bytes à zéro.

11. `src/ui/errors.rs:134-142` — faible, plausible. La détection d'un nom déjà existant repose sur le texte anglais du message ; la conversion des erreurs d'E/S (`src/models/error.rs:651`) perd le genre d'erreur. Scénario : course où la cible est créée entre la vérification et le renommage, l'erreur système (« File exists » sous Linux, texte localisé sous Windows) est présentée comme « nom invalide ».

12. `src/ui/errors.rs:48,91` — faible, plausible. Le repli des annulations et rétablissements renvoie le Debug complet de l'erreur comme détail (chemins, textes système), contraire à l'intention annoncée en tête de `path_load_message`. De plus, le rétablissement ne traite pas les échecs de récupération et d'identité d'historique, qui retombent sur ce repli. Scénario : échec inattendu d'un undo ou redo, détail technique brut montré à l'utilisateur.

13. `src/ui/elements/stepper.rs:115-127` — faible, plausible. La répétition d'un bouton maintenu ne s'arrête que sur un relâchement reçu par l'élément (dans ou hors de lui, dans la fenêtre). Scénario : perte de focus ou relâchement hors fenêtre pendant le maintien ; la valeur continue de progresser toutes les 80 ms jusqu'à la borne, y compris après la fermeture de la page de réglages.

14. `src/ui/elements/button.rs:75-93` — faible, plausible. Le bouton agit sur l'appui (pas au relâchement, sans annulation en sortant de la zone) et n'est ni focalisable ni activable au clavier, de sorte que le style de focus visible hérité de `button/style.rs` n'apparaît jamais. Scénario : bouton « Ouvrir » du visualiseur ou « effacer » de la recherche, inaccessibles à la navigation clavier.

15. `src/ui/file_types.rs:6-273` — faible, plausible. Le catalogue de code accepte comme noms entiers des termes courants (build, config, env, make, rest, inc) et des suffixes d'une lettre ou deux (c, d, l, m, p, r, s, t, re, cl), ainsi que des formats binaires (wasm, fasl). Scénario : fichier sans extension nommé « config » ou binaire « .wasm » rangé en Code, avec icône de code et aperçu texte tenté sur des données binaires ; l'icône (`components/icons.rs:313`) et le type d'aperçu (371) refont de plus une allocation par appel.

<!-- LOT TERMINÉ -->
