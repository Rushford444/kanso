# kanso

[English](README.md) | [Français](README.fr.md)

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="docs/assets/banner-dark.jpg">
  <img alt="kanso : des fiches d’instructions désordonnées sont progressivement rangées" src="docs/assets/banner-light.jpg" width="100%">
</picture>

Repérez les liens cassés, les secrets potentiellement exposés et les contextes trop volumineux dans les instructions avant la prochaine exécution.

[CI](https://github.com/Rushford444/kanso/actions/workflows/ci.yml) [![Licence : MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE) [![Dernière version](https://img.shields.io/badge/release-v0.1.0-blue)](https://github.com/Rushford444/kanso/releases/latest)

Renommez un guide de démarrage sans mettre à jour `AGENTS.md` : l’agent risque de suivre un lien mort. Répétez une consigne dans plusieurs fichiers : elle occupe du contexte qui pourrait servir au travail. Kanso vérifie localement les fichiers pris en charge et indique les résultats à examiner. Il ne garantit pas que l’agent respectera les instructions.

## Une vérification réelle

Dans un répertoire dont le fichier `AGENTS.md` contient un lien vers `missing.md` absent, voici la sortie obtenue avec la v0.1.0 :

```text
$ kanso check .
./AGENTS.md:5:5: warning KS004: relative link target or anchor not found
Score: 95 / 100 (15 tokens)
$ kanso score .
./AGENTS.md: 95 / 100
Global: 95 / 100
$ kanso explain KS004
A relative Markdown link refers to a missing file or heading anchor.
```

La première commande renvoie 1 à cause de l’avertissement ; les deux autres renvoient 0. Le [script d’enregistrement](docs/demo.tape) recrée cet exemple.

## Installation

Téléchargez l’archive de votre plateforme depuis la [version v0.1.0](https://github.com/Rushford444/kanso/releases/tag/v0.1.0) dans un nouveau répertoire vide. Ces binaires ciblent Linux x86-64 (glibc), macOS Intel et Apple Silicon, et Windows x86-64. Pour les autres architectures, compilez depuis les sources. Sans `gh`, téléchargez les deux mêmes fichiers depuis cette page.

La version publie `SHA256SUMS`, avec une ligne SHA-256 par archive ; les commandes ci-dessous ne vérifient que la ligne sélectionnée. Les sommes détectent une corruption, pas la compromission du compte de publication.

Linux (x86-64) :

```sh
gh release download v0.1.0 -R Rushford444/kanso -p 'kanso-0.1.0-x86_64-unknown-linux-gnu.tar.gz' -p SHA256SUMS
grep 'kanso-0.1.0-x86_64-unknown-linux-gnu.tar.gz$' SHA256SUMS | sha256sum -c -
tar -xzf kanso-0.1.0-x86_64-unknown-linux-gnu.tar.gz
```

macOS (Intel) :

```sh
gh release download v0.1.0 -R Rushford444/kanso -p 'kanso-0.1.0-x86_64-apple-darwin.tar.gz' -p SHA256SUMS
grep 'kanso-0.1.0-x86_64-apple-darwin.tar.gz$' SHA256SUMS | shasum -a 256 -c -
tar -xzf kanso-0.1.0-x86_64-apple-darwin.tar.gz
```

macOS (Apple Silicon) :

```sh
gh release download v0.1.0 -R Rushford444/kanso -p 'kanso-0.1.0-aarch64-apple-darwin.tar.gz' -p SHA256SUMS
grep 'kanso-0.1.0-aarch64-apple-darwin.tar.gz$' SHA256SUMS | shasum -a 256 -c -
tar -xzf kanso-0.1.0-aarch64-apple-darwin.tar.gz
```

Windows (x86-64, Git Bash) :

```sh
gh release download v0.1.0 -R Rushford444/kanso -p 'kanso-0.1.0-x86_64-pc-windows-msvc.zip' -p SHA256SUMS
grep 'kanso-0.1.0-x86_64-pc-windows-msvc.zip$' SHA256SUMS | sha256sum -c -
unzip kanso-0.1.0-x86_64-pc-windows-msvc.zip
```

Placez le binaire extrait dans un répertoire du PATH. Chaque archive contient aussi les README, LICENSE, CONTRIBUTING.md, SECURITY.md et les pages de documentation liées par les README, afin que ces liens fonctionnent dans l’archive.

Les binaires macOS ne sont pas notariés ; selon la cible, le binaire peut être signé ad hoc ou non signé. La notarisation exigerait un compte Apple Developer. Un téléchargement par navigateur peut ajouter l’attribut de quarantaine, et Gatekeeper peut alors refuser de lancer le binaire ou bloquer son premier démarrage pendant qu’il l’analyse. Dans ce cas, retirez l’attribut avant de l’exécuter :

```sh
xattr -d com.apple.quarantine kanso
./kanso --version
```

Une compilation depuis les sources avec Cargo évite l’attribut de quarantaine.

Vous pouvez aussi installer depuis Git (Rust et un accès réseau sont nécessaires) :

```sh
GIT_TERMINAL_PROMPT=0 cargo install --git https://github.com/Rushford444/kanso --tag v0.1.0 --locked
```

L’installation depuis Git exige un accès réseau et, tant que le dépôt est privé, des identifiants Git préconfigurés. `GIT_TERMINAL_PROMPT=0` désactive les invites de Git : sans identifiants, l’authentification échoue au lieu de les demander. Le jour où le dépôt sera public, la même commande n’aura besoin que du réseau.

La version minimale prise en charge de Rust est 1.88.0, car `serde-saphyr` 0.0.21, épinglé dans `Cargo.lock`, utilise les let-chains stabilisées en Rust 1.88.0 ; l’édition 2024 impose au moins 1.85.0. La CI la compile, la vérifie avec Clippy et la teste dans un job `msrv` dédié. Une publication sur crates.io est prévue, mais pas encore disponible. Pour vérifier ce dépôt sans attendre la publication, lancez `cargo build --locked`, puis utilisez `target/debug/kanso` (sous Windows : `target\debug\kanso.exe`).

## Démarrer en trois commandes

Après installation, à la racine de votre projet :

```sh
kanso init
kanso check .
kanso score .
```

`init` crée `kanso.toml` sans écraser un fichier existant. `check` renvoie 1 pour tout diagnostic autre qu’un alias, y compris informatif ; `score` renvoie 0 si l’analyse a réussi, même en présence de résultats.

## Usages courants

```sh
kanso explain KS007
kanso fix .
kanso fix . --write
```

## JSON et SARIF

```sh
kanso check . --json
kanso check . --format sarif > results.sarif
```

`fix` affiche un aperçu par défaut ; `--write` modifie les fichiers. Relisez le diff. La redirection SARIF crée ou remplace `results.sarif`, même si la commande renvoie 1. Le schéma JSON est `kanso/v1` ; SARIF est en version 2.1.0. Un rapport SARIF déclare chaque règle dans `runs[0].tool.driver.rules` et relie chaque résultat à sa règle par `ruleId` et `ruleIndex`.

## Configuration

`kanso init` écrit exactement ce fichier :

```toml
# kanso configuration; uncomment to override the defaults.
# Warning and error thresholds for a single file, in estimated tokens.
# token_warn = 2000
# token_error = 6000
# Aggregate root-context limits.
# total_token_warn = 10000
# total_token_error = 20000
# similarity_threshold = 0.85
# max_file_bytes = 2097152
# disabled_rules = []
# ignore = []
```

Consultez la [référence de configuration](docs/fr/configuration.md) et le [guide d’utilisation](docs/fr/usage.md).

## GitHub Actions

Ce workflow installe le tag indiqué et échoue si des diagnostics autres que des alias sont détectés. Il nécessite le réseau et, tant que le dépôt est privé, des identifiants Git préconfigurés sur le runner ; il ne publie pas de rapport SARIF.

```yaml
name: Agent instructions
on: [push, pull_request]
permissions:
  contents: read
jobs:
  kanso:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@11d5960a326750d5838078e36cf38b85af677262 # v4
      - uses: dtolnay/rust-toolchain@b725eda525362e7cf34af9a7243cac1e3f01a7ec # stable
      - run: GIT_TERMINAL_PROMPT=0 cargo install --git https://github.com/Rushford444/kanso --tag v0.1.0 --locked
      - run: kanso check .
```

Pour conserver un rapport SARIF, utilisez la redirection ci-dessus, préservez le code de sortie, puis ajoutez une étape distincte de publication. Les chemins ne sont pas masqués dans les rapports.

## Pour les agents IA

Installez sans interaction avec la commande Git épinglée ci-dessus ; elle définit `GIT_TERMINAL_PROMPT=0`, donc l’absence d’identifiants Git provoque un échec au lieu d’une invite. Hors ligne, utilisez une compilation locale. `kanso check . --json` produit des résultats lisibles par machine ; `--quiet` masque la sortie humaine de succès. Les commandes sont non interactives ; `--yes` est accepté, mais aucune confirmation n’est nécessaire. En CI sans interaction, utilisez `CI=true kanso check . --json --yes`. L’analyse ne nécessite pas de réseau. Codes de sortie : 0 succès, 1 diagnostics autres que des alias pour `check`, 2 erreur d’utilisation, 3 erreur d’exécution. `score` renvoie 0 si l’analyse réussit, même avec des résultats. En CI, traitez 1 comme des résultats, pas comme un crash.

À coller dans `AGENTS.md` ou `CLAUDE.md` :

```markdown
## Vérification des fichiers d’instructions

Exécuter `kanso check . --json` avant de soumettre des modifications d’instructions.
Le code 1 signale des résultats à examiner ; 2 signale une erreur d’utilisation et 3 une erreur d’exécution.
Prévisualiser avec `kanso fix .` avant `--write`, puis vérifier chaque modification.
```

## Contrôles effectués

Kanso découvre les fichiers d’instructions pris en charge en appliquant les règles gitignore. Il vérifie les métadonnées de skills (KS001), les budgets de tokens par fichier et à la racine (KS002–KS003), les liens (KS004), les instructions identiques ou similaires (KS005–KS006), les chaînes ressemblant à des secrets (KS007), les titres (KS008) et les commandes shell non reconnues à titre informatif (KS009). KS000 signale les problèmes d’E/S et n’est pas configurable. Les contrôles entre fichiers ne concernent que les fichiers racine chargés ensemble ; `SKILL.md` et les périmètres imbriqués sont exclus du contexte racine, mais restent soumis aux contrôles par fichier. Le comptage utilise o200k.

Le score part de 100 : chaque erreur retire 15 points, chaque avertissement 5 et chaque information 1, avec un maximum de 30 points retirés par règle et un minimum de zéro. Le score global applique cette formule à tous les résultats, sans moyenner les scores par fichier. Les alias ne sont pas notés et ne font pas échouer `check`. L’analyse est hors ligne et n’exécute jamais les commandes trouvées.

## Limites

Il s’agit d’une analyse statique, pas d’un audit de sécurité ni d’une preuve de justesse des instructions. KS007 masque les valeurs détectées, mais pas les chemins : ne mettez pas de secrets dans les noms de fichiers. La détection contextuelle examine au plus les 128 octets précédents sur la même ligne et comporte des exclusions documentées ; elle peut donc manquer des secrets. Les corrections sont prudentes : les fichiers contenant du HTML brut sont ignorés et les corrections d’espaces concernent les lignes de liste simples. Les liens symboliques vers des fichiers réguliers dans le projet sont traités comme des alias ; une cible hors du sous-arbre sélectionné peut être analysée même si elle est ignorée. Les cibles non sûres ou non régulières sont signalées.

## Questions fréquentes

**Mes fichiers sont-ils envoyés ailleurs ?** Non. L’analyse est hors ligne ; l’installation Git et la préparation CI nécessitent le réseau.

**Pourquoi `check` échoue-t-il sur une information ?** Tout diagnostic autre qu’un alias entraîne le code 1 ; la CI peut appliquer sa propre politique à partir du JSON.

**Un score de 100 garantit-il la sécurité ?** Non. Il indique seulement que ces contrôles n’ont retiré aucun point.

**Puis-je l’utiliser sans écrire de fichier ?** Oui. `check`, `score`, `explain` et l’aperçu `fix` n’écrivent pas. `init`, `fix --write` et la redirection shell écrivent.

**Est-ce un agent ou un linter Markdown généraliste ?** Non. C’est un vérificateur statique ciblé sur les fichiers d’instructions d’agents pris en charge.

## Alternatives

[markdownlint](https://github.com/DavidAnson/markdownlint) vérifie le style Markdown général ; Kanso cible les instructions d’agents et leur contexte commun. [pre-commit](https://pre-commit.com/) orchestre des hooks plutôt que de fournir les contrôles de Kanso ; vous pouvez y exécuter Kanso. Ces outils sont complémentaires.

## Contribuer

Les tickets et contributions sont bienvenus. Consultez [CONTRIBUTING.md](CONTRIBUTING.md) pour le développement et les vérifications, et [SECURITY.md](SECURITY.md) pour signaler une faille en privé.

## Licence

[MIT](LICENSE). Copyright (c) 2026 Kodawari.
