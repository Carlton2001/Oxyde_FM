# Premier build Linux et garde Windows

Le premier jalon de compilation réussit sous **Fedora 44 WSL, Rust 1.98.1** :
toutes les cibles passent `cargo check` sans avertissement du projet et le binaire
principal est construit et lié. **Windows reste la plateforme de référence.**

Date : 26 octobre 2026. Base : `3b55f654764b811ccc85faa548e2feea7deba163`
(2.0.2), changements locaux sur `codex/linux-foundation`. Aucun commit ni push.
L'[audit préalable](2026-10-26-linux-portability-audit.md) reste une photographie
de cette base, avant les corrections et validations décrites ici.

## Changement limité à la compilation

- Le module GPUI du glissement du fil d'Ariane était déclaré Windows alors que
  ses appels étaient communs : sa déclaration devient commune.
- La commande Copier le chemin du fil d'Ariane conserve le presse-papiers natif
  Windows ; Linux utilise le presse-papiers de texte GPUI.
- Imports, constantes, fonctions et helpers de tests Windows sont conditionnés
  comme leurs consommateurs. Aucun scénario de test Windows n'est retiré.
- La mutabilité nécessaire à la normalisation des chemins Windows reste dans
  sa branche ; la validation Linux ne produit plus d'avertissement `unused_mut`.
- `scripts/check-linux.sh` fournit le check reproductible avec verrou Cargo,
  option `--offline` et cache Linux distinct du `target/` Windows. La règle
  `.gitattributes` est limitée aux scripts Bash pour conserver leurs fins LF.

Pas de changement de dépendance, de pin GPUI, de patch D3D11, de workflow,
de curseur ou d'algorithme natif Windows. Pas de réorganisation des modules.
Les opérations Linux et leurs limites de sécurité ne sont pas corrigées par
ce jalon. Les [instructions Fedora/WSL](../guides/linux-development.md) décrivent
comment reproduire la compilation.

## Validation réellement effectuée

| Environnement / commande | Résultat |
| --- | --- |
| Windows, `cargo check --offline --locked --all-targets` avant changement | Réussi, sans avertissement projet |
| Fedora WSL, `bash scripts/check-linux.sh --offline` | Toutes les cibles compilent, sans avertissement projet |
| Fedora WSL, `cargo build --offline --locked --bin oxyde` | Réussi ; premier build complet en 7 min 10 s, hors récupération initiale des sources |
| Fedora WSL, inspection `file` et `ldd` | ELF x86_64 ; aucune bibliothèque liée manquante dans cet environnement |
| Fedora WSL, `cargo test --offline --locked --lib utils::path_security::tests -- --test-threads=1` | 2 tests réussis |
| Fedora WSL, `cargo test --offline --locked --bin oxyde ui::components::top_bar::breadcrumb_glide::tests -- --test-threads=1` | 1 test réussi, aucune fenêtre ouverte |
| Windows natif, `./scripts/test-windows.ps1 -Offline` hors sandbox | Check sans avertissement, build explicite des binaires, **1 428 tests réussis, 0 échec, 51 ignorés** |

La première tentative de suite Windows dans le sandbox de l'agent a été
interrompue : une création de lien physique sur les fixtures était refusée avec
le code 5. Le même test isolé passe hors sandbox avec le même exécutable, puis
la suite native complète passe. Les résultats de cette tentative restreinte ne
sont pas assimilés à des régressions fonctionnelles.

Les journaux locaux sont dans `target/linux-foundation-*.log` et ne sont pas
versionnés. Les décomptes de suite excluent le résultat du sous-processus de
test filtré, déjà compris dans la suite parente. Les tests ignorés restent non
validés ; aucun essai visuel Windows n'a été effectué pour ce jalon.

## Limites et étape suivante

Aucun lancement graphique, test de corbeille, montage, éjection ou accès réseau
Linux. WSL ne valide pas GNOME/Wayland natif, Silverblue ou les GPU Intel/NVIDIA.
Les parsers Linux restent locaux et les workers auxiliaires n'offrent toujours
pas de confinement Linux : voir la [sécurité R5](../security/status.md).

La prochaine étape est un lancement sur profil de test en VM pour vérifier
fenêtres, focus, menus, échelle et navigation. Les risques de corbeille,
publication, noms de fichiers et drag identifiés dans l'audit doivent être
traités avant d'utiliser Linux pour modifier des données réelles.
