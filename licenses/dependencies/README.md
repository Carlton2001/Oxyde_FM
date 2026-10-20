# Licences des dépendances

`THIRD-PARTY-NOTICES.md` contient les textes de licence et leurs mentions de copyright, avec les crates et versions concernées. `cargo-about` utilise les sources résolues par `Cargo.lock`, y compris GPUI et GPUI Platform depuis la révision Zed épinglée. Toutes les cibles et dépendances de construction sont incluses ; cet inventaire peut donc dépasser les crates utilisées dans un binaire Windows.

Le script `build-distribution.ps1` régénère automatiquement ces notices à chaque distribution,
avant compilation et assemblage du ZIP ou du Setup, même avec `-SkipBuild`.
Installer une fois le générateur en local ; le workflow GitHub l'installe automatiquement.
La commande suivante permet aussi une actualisation manuelle :

```powershell
cargo install cargo-about --version 0.9.2 --locked --features cli
./scripts/update-third-party-licenses.ps1
```

La génération accède aux dépôts amont pour retrouver les licences absentes des archives Cargo. `-Offline` est disponible lorsque les sources nécessaires sont en cache ; ce mode peut perdre les copyrights spécifiques aux licences récupérées à distance. Préférer la génération en ligne pour une livraison.

Le même fichier regroupe aussi les fichiers NOTICE, COPYRIGHT et AUTHORS fournis dans les sources locales des crates, dans une section complémentaire indiquant le paquet, sa version et le chemin source. Les textes sont conservés sans réécriture. Les auteurs Cargo ne sont pas considérés comme une liste de titulaires de droits. Les textes de licence identiques sont mutualisés par `cargo-about`, avec la liste des crates concernées ; les textes dont les copyrights diffèrent restent distincts.

`inputs.json` enregistre les empreintes des manifests, de la configuration, du template et du rapport. Après génération, la distribution contrôle ces empreintes et s'arrête en cas d'échec. Elle copie ce dossier dans le ZIP portable et le Setup classique ; les avis de grammaires restent dans `licenses/syntaxes/`.

GPUI et GPUI Platform déclarent Apache-2.0 ; leurs licences figurent dans le rapport unique avec les autres crates de la révision Zed épinglée.

`LUCIDE-LICENSE.txt` conserve la notice officielle ISC et MIT/Feather ; sa source est enregistrée dans `LUCIDE-SOURCE.txt`. Les révisions originales des SVG du projet n'ont pas été enregistrées.
