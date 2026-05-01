# Changelog

All notable changes to `embedded-stats-f32` will be documented in this file.

The format is based on Keep a Changelog, and this project follows Semantic Versioning.

---

## [0.3.0] - 2026-05-01

### 🚀 Added

- Ajout des statistiques **streaming avancées** via `StreamingStats`
  - `running_variance()` en O(1) mémoire
  - `running_std_dev()` en O(1) mémoire
- Intégration complète de l’algorithme de **Welford** (variance stable numériquement)
- Renforcement des garde-fous sur les calculs streaming
- Vérification de cohérence interne via `check_state()` (défensive)

### 🧠 Improved

- Refactor complet du streaming stats :
  - stockage interne `m2` ajouté pour variance streaming
  - amélioration de la stabilité numérique sur longues séquences
- Amélioration de la robustesse globale des API statistiques
- Cohérence entre version batch (`mean/variance/std_dev`) et streaming

### 🛡️ Safety / Robustness

- Rejet systématique des valeurs `NaN`, `+inf`, `-inf` dans toutes les entrées
- Garantie que l’état interne de `StreamingStats` n’est jamais corrompu après un input invalide
- Ajout de vérifications défensives supplémentaires sur les résultats (`is_finite()`)

### 📚 Documentation

- Documentation enrichie sur :
  - Welford (streaming mean/variance)
  - stabilité numérique (Kahan + Welford)
- README mis à jour avec :
  - section migration
  - explication des algorithmes
  - compatibilité embedded targets

### 🧪 Tests

- Ajout de tests de robustesse :
  - stabilité longue durée streaming
  - rejet des valeurs non finies sans corruption d’état
  - validation streaming vs batch (cohérence numérique)
  - tests de reset et invariants internes

---

## [0.2.0] - 2026-xx-xx

### Added

- Protection contre `NaN` et `±inf`
- `StatsError::NonFiniteValue`
- `ensure_finite()` garde centrale
- `kahan_sum_checked()` fail-fast
- `StreamingStats` initial version sécurisée

---

## [0.1.0] - initial release

### Added

- `mean`
- `variance`
- `std_dev`
- version initiale sans protection NaN/inf