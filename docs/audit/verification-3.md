# Vérification 3 : constats critiques et élevés (CORE, UI-APP, UI-COMP, UI-REST)

Rapports relus : CORE-01 à CORE-03, UI-APP-01 à UI-APP-09, UI-COMP-01 à UI-COMP-06, UI-REST-01 à UI-REST-03. Seuls CORE-01, UI-APP-04, UI-APP-07, UI-COMP-01 et UI-COMP-03 contiennent des constats critiques ou élevés (aucun critique). Les autres rapports n'en déclarent aucun.

| # | Constat | Verdict | Gravité corrigée |
| --- | --- | --- | --- |
| 1 | CORE-01 n° 1, `src/commands/search.rs:460` : la recherche de contenu bloque sur un FIFO | Confirmé | Moyen |
| 2 | UI-APP-04, `src/ui/app/filters.rs:43-50` : la sélection masquée par un filtre reste active | Confirmé | Moyen |
| 3 | UI-APP-07 n° 1, `src/ui/app/trash_settings/actions.rs:70` : purge de la corbeille sans confirmation | Confirmé | Élevé (maintenu) |
| 4 | UI-COMP-01 n° 1, `src/ui/components/conflict_dialog/controls.rs:126-136` : boutons de conflit déclenchés à l'appui et par répétition | Confirmé | Élevé (maintenu) |
| 5 | UI-COMP-03 n° 1, `src/ui/components/input.rs:771-808` : aucune gestion de la composition IME | Confirmé | Élevé (maintenu) |

## Détail

1. **Recherche de contenu bloquée par un FIFO : confirmé, moyen.** Le filtre de métadonnées n'écarte que les dossiers. Le contrôle binaire ouvre donc tout autre type d'entrée (FIFO, ou lien vers un FIFO) avant de vérifier qu'il s'agit d'un fichier ordinaire, et cette ouverture n'est pas annulable. Scénario sous Linux : une recherche de contenu dans une arborescence qui contient un tube nommé lisible fige le fil de recherche pour toujours. Gravité abaissée : ce fil est indépendant et détaché, il ne tient pas le verrou disque à ce moment-là, et l'annulation côté interface remet bien l'onglet à l'état arrêté. Il reste un fil perdu et une recherche qui tourne sans fin si l'utilisateur ne l'annule pas. Windows n'est pas concerné, sauf pour un partage réseau qui ne répond plus.

2. **Sélection masquée par un filtre : confirmé, moyen.** Changer de filtre relance la préparation de l'onglet sans jamais élaguer la sélection, et Supprimer comme Copier/Couper lisent directement l'ensemble des chemins sélectionnés. Scénario : Ctrl+A, puis un filtre de nom qui ne laisse voir qu'un fichier, puis Couper/Coller ou Suppr : les fichiers masqués suivent aussi. Gravité abaissée : la confirmation de suppression est activée par défaut et affiche le nombre et les noms des éléments, et la corbeille permet de les récupérer. L'action reste silencieuse pour Couper/Copier et quand la confirmation est désactivée.

3. **Purge de la corbeille sans confirmation : confirmé, élevé.** Dans la vue corbeille, Suppr passe directement à la purge, avant le test de la préférence de confirmation. Le menu contextuel et les entrées 7 et 10 de la barre (supprimer, tout supprimer) appellent la même purge, sans dialogue. Côté Windows, la vidange système est appelée avec le drapeau qui supprime sa propre confirmation. Scénario : Ctrl+A puis Suppr, ou un clic sur « tout supprimer », et la corbeille est vidée de façon définitive.

4. **Boutons du dialogue de conflits : confirmé, élevé.** Ignorer et Remplacer agissent à l'appui du bouton gauche, sans regarder le nombre de clics, et à chaque Entrée ou Espace, sans écarter la répétition de touche. En mode simple, chaque choix passe au conflit suivant, et le dernier applique toutes les décisions. Scénario : un double-clic sur Remplacer, ou Entrée maintenue, remplace les fichiers cibles du conflit suivant que l'utilisateur n'a pas vu.

5. **Composition IME absente : confirmé, élevé.** Le champ annonce toujours « aucune plage marquée », ne fait rien en fin de composition, et traite le texte en préédition comme du texte définitif. La couche Windows vendue transmet chaque préédition sans plage, puis le résultat, eux aussi sans plage, qui remplacent donc la sélection repliée en fin de texte. Scénario : une saisie en japonais, en chinois ou en coréen dans un champ de renommage, d'adresse ou de filtre accumule chaque étape de préédition puis le texte validé. Le résultat est dupliqué et inutilisable. Le constat était marqué « plausible » ; le chemin d'appel le confirme sous Windows. Le chemin Linux n'a pas été suivi, mais il passe par la même interface.

Bilan : 5 constats vérifiés, 5 confirmés (dont 2 abaissés à moyen), 0 faux positif, 0 incertain.

<!-- LOT TERMINÉ -->
