[![Crates.io](https://img.shields.io/crates/v/embedded-stats-f32.svg)](https://crates.io/crates/embedded-stats-f32)
[![Docs.rs](https://docs.rs/embedded-stats-f32/badge.svg)](https://docs.rs/embedded-stats-f32)
[![License: GPL v2](https://img.shields.io/badge/License-GPL_v2-blue.svg)](https://www.gnu.org/licenses/old-licenses/gpl-2.0.en.html)

# embedded-stats-f32
> Statistiques `f32` `no_std` : moyenne, variance, écart type, moyenne streaming  zéro dépendance externe, zéro `unsafe`, rejet garanti des `NaN` et `±inf` pour une meilleure approche numérique et gestion des edge cases .

---

# Update version 0.2.0  protection NaN/inf

La version 0.1.0 était **dangereuse en production embarquée** : un `NaN` ou un `±inf`
entrant dans l'accumulateur corrompait silencieusement toutes les mesures suivantes,
sans aucune remontée d'erreur.

La version 0.2.0 introduit :
- `StatsError::NonFiniteValue`:nouveau variant d'erreur explicite
- `ensure_finite()`:garde centrale unique, appelée à chaque entrée
- `kahan_sum_checked()`:fail-fast dès la première valeur non-finie
- `StreamingStats::update()`:nouvelle signature `-> Result<(), StatsError>` :
  l'état interne est **inaltérable** en cas de rejet, `count` et `mean` restent cohérents
- Garde défensive `check_state()` dans `mean()` : protection contre tout état corrompu

**Migrer depuis 0.1.0 :**
```rust
// Avant (0.1.0) silencieux, dangereux
acc.update(x);

// Après (0.2.0)  explicite, sûr
acc.update(x)?;
```

---

## Fonctionnalités

- `#![no_std]` :fonctionne sans bibliothèque standard
- `#![forbid(unsafe_code)]` :100 % safe Rust
- Pas de `libm`, pas de `std`
- Rejet **fail-fast** des `NaN` et `±inf`  aucune contamination silencieuse
- Sommation compensée de **Kahan** pour `mean` et `variance`
- Moyenne streaming par l'algorithme de **Welford** (O(1) mémoire)
- Écart type via [`embedded-f32-sqrt`](https://crates.io/crates/embedded-f32-sqrt) (Newton-Raphson, arithmétique logicielle)

---

## Fonctions disponibles

| Fonction / Type | Description |
|---|---|
| `mean(&[f32])` | Moyenne arithmétique (Kahan) |
| `variance(&[f32])` | Variance de population (diviseur N) |
| `std_dev(&[f32])` | Écart type (= √variance) |
| `StreamingStats` | Moyenne en ligne de Welford, O(1) |

---

## Installation

```toml
[dependencies]
embedded-stats-f32 = "0.2.0"
```

---

## Utilisation

```rust
use embedded_stats_f32::{mean, variance, std_dev, StreamingStats, StatsError};

let data = [2.0_f32, 4.0, 4.0, 4.0, 5.0, 5.0, 7.0, 9.0];

// Moyenne
let m = mean(&data).unwrap();
// → 5.0

// Variance de population
let v = variance(&data).unwrap();
// → 4.0

// Écart type
let s = std_dev(&data).unwrap();
// → 2.0

// Moyenne streaming : O(1) mémoire, aucun tableau à stocker
let mut acc = StreamingStats::new();
for &x in &data { acc.update(x).unwrap(); }
let ms = acc.mean().unwrap();
// → 5.0

// NaN rejeté, état interne préservé
assert_eq!(acc.update(f32::NAN), Err(StatsError::NonFiniteValue));
assert_eq!(acc.count(), 8); // inchangé

// Tranche vide → StatsError::EmptySlice
assert_eq!(mean(&[] as &[f32]), Err(StatsError::EmptySlice));

// Valeur infinie → StatsError::NonFiniteValue
assert_eq!(mean(&[f32::INFINITY]), Err(StatsError::NonFiniteValue));
```

---

## Erreurs

Toutes les fonctions retournent `Result<f32, StatsError>`.

| Variant | Cause |
|---|---|
| `StatsError::EmptySlice` | Tranche vide ou accumulateur non alimenté |
| `StatsError::NonFiniteValue` | `NaN` ou `±inf` dans les données ou le résultat |

---

## Algorithmes

### Kahan (`mean`, `variance`)

Réduit l'erreur d'arrondi accumulée de O(Nε) à O(ε), avec validation fail-fast :

```
ensure_finite(x)?   ← rejet immédiat si NaN/inf
y    = x - comp
t    = sum + y
comp = (t - sum) - y
sum  = t
```

### Welford (`StreamingStats`)

Mise à jour incrémentale O(1), stable pour des millions de points :

```
ensure_finite(x)?                          ← état préservé si rejet
mean_n = mean_{n-1} + (x_n − mean_{n-1}) / n
```

### Écart type

Délégué à [`embedded-f32-sqrt`](https://crates.io/crates/embedded-f32-sqrt) :
estimation initiale IEEE 754 puis 5 itérations Newton-Raphson.
Erreur relative < 1 ULP `f32`, en arithmétique logicielle pure.

---

## Compatibilité

| Cible | Statut |
|---|---|
| `thumbv6m-none-eabi` (RP2040, Cortex-M0+) | ✅ |
| `thumbv8m.main-none-eabihf` (RP2350, Cortex-M33) | ✅ |
| `thumbv7em-none-eabihf` (STM32, Cortex-M4F) | ✅ |
| tout target `no_std` | ✅ |

`.cargo/config.toml` pour Pico (RP2040) :

```toml
[build]
target = "thumbv6m-none-eabi"
```

Pour Pico 2 (RP2350) :

```toml
[build]
target = "thumbv8m.main-none-eabihf"
```

---

## Licence

Ce programme est un logiciel libre : vous pouvez le redistribuer et/ou le modifier
selon les termes de la **Licence Publique Générale GNU** telle que publiée par la
Free Software Foundation, soit la version 2 de la licence, soit (à votre choix)
n'importe quelle version ultérieure.

Ce programme est distribué dans l'espoir qu'il sera utile, mais **sans aucune garantie**.

Voir le fichier `LICENSE` ou [gnu.org/licenses](https://www.gnu.org/licenses/) pour les détails.

**Copyright (C) 2026 Jorge Andre Castro** — Licence GNU GPL v2 ou ultérieure.