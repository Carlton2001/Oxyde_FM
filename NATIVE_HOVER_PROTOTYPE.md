# Prototype des aperçus en fenêtre native

Branche : `codex/native-hover-preview`.

Sous Windows, les infobulles de fichiers avec aperçu image ou PDF utilisent une
fenêtre GPUI native réutilisée par fenêtre principale. Cela concerne les listes
de fichiers utilisant la session de survol partagée, en modes Détails et Grille.
Les préférences d'aperçu et le délai de survol existants restent applicables.
Les autres infobulles, les vidéos et les aperçus texte conservent leur rendu actuel.
Sous Linux, le rendu actuel reste utilisé.

La fenêtre native ne prend pas le focus, laisse passer la souris, n'apparaît pas
dans la barre des tâches et appartient à sa fenêtre Oxyde. Son placement suit le
pointeur et les limites de la zone de travail du moniteur, même en dehors des
bords de la fenêtre principale. Elle est masquée quand la session devient
invalide et supprimée à la fermeture de la fenêtre principale.

Le chargement et l'isolation des décodeurs existants sont conservés. Un contrôle
sur le thread UI toutes les 100 ms assure uniquement la disparition lors d'une
désactivation, même sans nouveau rendu de la fenêtre principale. Le déplacement
suit directement les événements de souris, sans minuterie de suivi. Les
placements inchangés ne font rien ; les déplacements seuls ne redimensionnent
pas la fenêtre.

Lors du passage à un autre fichier image ou PDF, le nom et les métadonnées
changent immédiatement ; l'aperçu se complète pendant son chargement en arrière-plan.
La taille native est ajustée dès que le rendu mesure le nouveau contenu.
L'écart de 24 pixels logiques par axe est mesuré jusqu'au cadre visible : la
marge transparente de l'ombre n'ajoute plus de distance au pointeur. Près d'un
bord d'écran, la bulle se retourne ou est limitée à la zone de travail.

Les images décodées disposent, comme les PDF, d'un cache RAM de 32 entrées et
64 Mio maximum. Tous les pixels des animations comptent dans ce plafond. Les
retours sur un fichier inchangé réutilisent le même `RenderImage` ; un contrôle
de taille/date de modification sur le worker invalide les fichiers modifiés.
Les erreurs et les résultats de fichiers modifiés pendant le décodage ne sont
pas conservés. Le cache GPUI des tâches est toujours libéré au dernier survol ;
le cache de pixels indépendant évite de relancer les décodeurs aux retours.
Les demandes de placement sont regroupées puis appliquées dans la boucle de
messages Windows, pour éviter les rappels de déplacement/redimensionnement
pendant une mise à jour de vue GPUI. Une demande de masquage remplace les
placements en attente. Les ombres dessinées par les infobulles GPUI ont une
opacité réduite de moitié.
La décoration non cliente DWM reste désactivée sur la fenêtre d'aperçu native.
Une seule ombre GPUI légère est conservée, identique à celle des infobulles
intérieures, avec leurs coins arrondis et leur bordure sémantique. La fenêtre
transparente réserve une marge autour du contenu pour afficher ce halo sans le
couper ni dessiner un rectangle opaque dans les coins.

## Essai manuel

Lancer `cargo run --offline`, avec les aperçus activés dans les préférences.

1. Survoler une image puis un PDF dans les modes Détails et Grille.
2. Essayer près de chacun des bords d'une fenêtre Oxyde non maximisée : la bulle
   peut dépasser Oxyde, mais doit rester dans la zone de travail de l'écran.
3. Passer rapidement entre les fichiers, défiler, changer de dossier et cliquer.
   Vérifier la disparition et l'absence de nom ou d'aperçu obsolète.
4. Vérifier que la souris peut sélectionner un fichier sous la bulle et que les
   raccourcis clavier restent dirigés vers la fenêtre principale.
5. Faire Alt+Tab, minimiser Oxyde, ouvrir une fenêtre secondaire et fermer Oxyde.
   Vérifier qu'aucune bulle ne reste visible et qu'aucun processus ne persiste.
6. Tester le zoom de l'interface et, si disponible, deux écrans avec des échelles
   différentes. Vérifier la taille, le placement et la lisibilité.

Ce prototype modifie le placement : la bulle complète passe à gauche ou au-dessus
du pointeur quand l'espace manque. La marge transparente du halo participe au
calcul de placement pour rester dans la zone de travail de l'écran.
