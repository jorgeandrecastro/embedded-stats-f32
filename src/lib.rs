// Copyright (C) 2026 Jorge Andre Castro
//
// Ce programme est un logiciel libre : vous pouvez le redistribuer et/ou le modifier
// selon les termes de la Licence Publique Générale GNU telle que publiée par la
// Free Software Foundation, soit la version 2 de la licence, soit (à votre convention)
// n'importe quelle version ultérieure.

//! # embedded-stats-f32
//!
//! Statistiques `f32` pour systèmes embarqués `no_std`.
//!
//! Sans dépendance, sans `unsafe`, sans FPU requise.
//!
//! Fournit :
//! - [`mean`]           moyenne arithmétique sur une tranche
//! - [`variance`]       variance (population, non corrigée)
//! - [`std_dev`]        écart type (= √variance)
//! - [`StreamingStats`]  moyenne en ligne (Welford), O(1) mémoire
//!
//! Toutes les fonctions rejettent les valeurs non-finies (`NaN`, `±inf`)
//! via [`StatsError::NonFiniteValue`].
//!
//! ## Exemple rapide
//!
//! ```rust
//! use embedded_stats_f32::{mean, variance, std_dev, StreamingStats};
//!
//! let data = [2.0_f32, 4.0, 4.0, 4.0, 5.0, 5.0, 7.0, 9.0];
//!
//! assert!((mean(&data).unwrap()     - 5.0).abs() < 1e-5);
//! assert!((variance(&data).unwrap() - 4.0).abs() < 1e-4);
//! assert!((std_dev(&data).unwrap()  - 2.0).abs() < 1e-4);
//!
//! let mut s = StreamingStats::new();
//! for &x in &data { s.update(x).unwrap(); }
//! assert!((s.mean().unwrap() - 5.0).abs() < 1e-5);
//! ```

#![no_std]
#![forbid(unsafe_code)]
#![warn(missing_docs)]

use embedded_f32_sqrt::sqrt;

///  Erreurs EmptySlice et NonFiniteValue pour la roboustesse du systeme 
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatsError {
    /// La tranche (ou l'accumulateur) est vide : le calcul est impossible.
    EmptySlice,
    /// Une valeur `NaN` ou infinie a été détectée dans les données.
    NonFiniteValue,
}

// Garde centrale

/// Vérifie qu'une valeur `f32` est finie (`!NaN`, `!±inf`).
///
/// Point unique de validation — évite de dupliquer la logique dans chaque fonction.
#[inline]
fn ensure_finite(x: f32) -> Result<f32, StatsError> {
    if x.is_finite() {
        Ok(x)
    } else {
        Err(StatsError::NonFiniteValue)
    }
}

//Moyenne 

/// Calcule la moyenne arithmétique d'une tranche `f32`.
///
/// Utilise la sommation compensée de Kahan pour minimiser l'erreur
/// d'arrondi sur les grandes tranches.
///
/// # Erreurs
///
/// - [`StatsError::EmptySlice`]      tranche vide
/// - [`StatsError::NonFiniteValue`] `NaN` ou `±inf` dans les données
///
/// # Exemples
///
/// ```rust
/// use embedded_stats_f32::{mean, StatsError};
///
/// assert!((mean(&[1.0, 2.0, 3.0]).unwrap() - 2.0).abs() < 1e-6);
/// assert_eq!(mean(&[] as &[f32]),            Err(StatsError::EmptySlice));
/// assert_eq!(mean(&[f32::NAN]),              Err(StatsError::NonFiniteValue));
/// assert_eq!(mean(&[f32::INFINITY]),         Err(StatsError::NonFiniteValue));
/// ```
pub fn mean(data: &[f32]) -> Result<f32, StatsError> {
    if data.is_empty() {
        return Err(StatsError::EmptySlice);
    }
    let sum = kahan_sum_checked(data)?;
    Ok(sum / data.len() as f32)
}

//  Variance 

/// Calcule la variance de population (non corrigée, diviseur N) d'une tranche `f32`.
///
/// # Erreurs
///
/// - [`StatsError::EmptySlice`]     tranche vide
/// - [`StatsError::NonFiniteValue`] `NaN` ou `±inf` dans les données ou dans le résultat
///
/// # Exemples
///
/// ```rust
/// use embedded_stats_f32::{variance, StatsError};
///
/// let data = [2.0_f32, 4.0, 4.0, 4.0, 5.0, 5.0, 7.0, 9.0];
/// assert!((variance(&data).unwrap() - 4.0).abs() < 1e-4);
/// assert_eq!(variance(&[f32::NAN]), Err(StatsError::NonFiniteValue));
/// ```
pub fn variance(data: &[f32]) -> Result<f32, StatsError> {
    if data.is_empty() {
        return Err(StatsError::EmptySlice);
    }

    let m = kahan_sum_checked(data)? / data.len() as f32;

    let mut sum  = 0.0_f32;
    let mut comp = 0.0_f32;

    for &x in data {
        let x = ensure_finite(x)?;
        let d = x - m;
        let y = d * d - comp;
        let t = sum + y;
        comp  = (t - sum) - y;
        sum   = t;
    }

    let result = sum / data.len() as f32;
    ensure_finite(result)
}

//  Écart type
/// Calcule l'écart type de population (= √variance) d'une tranche `f32`.
///
/// Utilise [`embedded_f32_sqrt::sqrt`] (Newton-Raphson, pas de FPU requise).
///
/// # Erreurs
///
/// - [`StatsError::EmptySlice`]      tranche vide
/// - [`StatsError::NonFiniteValue`]  `NaN` ou `±inf` dans les données ou le résultat
///
/// # Exemples
///
/// ```rust
/// use embedded_stats_f32::{std_dev, StatsError};
///
/// let data = [2.0_f32, 4.0, 4.0, 4.0, 5.0, 5.0, 7.0, 9.0];
/// assert!((std_dev(&data).unwrap() - 2.0).abs() < 1e-4);
/// assert_eq!(std_dev(&[f32::NAN]), Err(StatsError::NonFiniteValue));
/// ```
pub fn std_dev(data: &[f32]) -> Result<f32, StatsError> {
    let v = variance(data)?;

    // clamp défensif
    let v = if v < 0.0 { 0.0 } else { v };
    // Clamp défensif : peut être légèrement négatif à cause des erreurs flottantes
    let s = sqrt(v).map_err(|_| StatsError::NonFiniteValue)?;
    ensure_finite(s)
}

// Moyenne streaming (Welford) 

/// Accumulateur de moyenne en ligne, O(1) mémoire.
///
/// Implémente la mise à jour incrémentale de Welford :
/// ```text
/// mean_n = mean_{n-1} + (x_n − mean_{n-1}) / n
/// ```
/// Stable numériquement même pour des millions de points.
/// Rejette les valeurs non-finies sans corrompre l'état interne.
///
/// # Exemples
///
/// ```rust
/// use embedded_stats_f32::{StreamingStats, StatsError};
///
/// let mut s = StreamingStats::new();
/// for x in [1.0_f32, 2.0, 3.0, 4.0, 5.0] { s.update(x).unwrap(); }
///
/// assert!((s.mean().unwrap() - 3.0).abs() < 1e-6);
/// assert_eq!(s.count(), 5);
///
/// // NaN rejeté, état interne préservé
/// assert_eq!(s.update(f32::NAN), Err(StatsError::NonFiniteValue));
/// assert_eq!(s.count(), 5);
/// ```
#[derive(Debug, Clone, Copy)]
pub struct StreamingStats {
    count: u32,
    mean:  f32,
}

impl StreamingStats {
    /// Crée un accumulateur vide.
    #[inline]
    pub const fn new() -> Self {
        Self { count: 0, mean: 0.0 }
    }

    /// Intègre une nouvelle observation.
    ///
    /// Retourne [`StatsError::NonFiniteValue`] si `x` est `NaN` ou `±inf`.
    /// En cas d'erreur, l'état interne est **inchangé**.
    #[inline]
    pub fn update(&mut self, x: f32) -> Result<(), StatsError> {
        let x = ensure_finite(x)?;
        self.count += 1;
        self.mean  += (x - self.mean) / self.count as f32;
        Ok(())
    }

    /// Retourne la moyenne courante.
    ///
    /// # Erreurs
    ///
    /// - [`StatsError::EmptySlice`]     — aucune observation intégrée
    /// - [`StatsError::NonFiniteValue`] — état interne corrompu (théoriquement impossible
    ///   via [`update`](StreamingStats::update), garde de sécurité défensive)
    #[inline]
    pub fn mean(&self) -> Result<f32, StatsError> {
        if self.count == 0 {
            return Err(StatsError::EmptySlice);
        }
        self.check_state()?;
        Ok(self.mean)
    }

    /// Retourne le nombre d'observations intégrées.
    #[inline]
    pub const fn count(&self) -> u32 {
        self.count
    }

    /// Remet l'accumulateur à zéro.
    #[inline]
    pub fn reset(&mut self) {
        self.count = 0;
        self.mean  = 0.0;
    }

    /// Garde défensive : vérifie que l'état interne est cohérent.
    #[inline]
    fn check_state(&self) -> Result<(), StatsError> {
        if self.mean.is_finite() {
            Ok(())
        } else {
            Err(StatsError::NonFiniteValue)
        }
    }
}

impl Default for StreamingStats {
    fn default() -> Self {
        Self::new()
    }
}

// Utilitaire interne 

/// Sommation compensée de Kahan avec validation  fail-fast sur NaN/inf.
///
/// Réduit l'erreur d'arrondi de O(Nε) à O(ε).
/// Retourne [`StatsError::NonFiniteValue`] dès la première valeur non-finie.
#[inline]
fn kahan_sum_checked(data: &[f32]) -> Result<f32, StatsError> {
    let mut sum  = 0.0_f32;
    let mut comp = 0.0_f32;

    for &x in data {
        let x = ensure_finite(x)?;
        let y = x - comp;
        let t = sum + y;
        comp  = (t - sum) - y;
        sum   = t;
    }

    Ok(sum)
}

// Tests 

#[cfg(test)]
mod tests {
    use super::*;

    // Dataset de référence Wikipedia (variance = 4.0, std = 2.0, mean = 5.0)
    const DATA: [f32; 8] = [2.0, 4.0, 4.0, 4.0, 5.0, 5.0, 7.0, 9.0];

    //  mean 

    #[test]
    fn test_mean_reference() {
        let m = mean(&DATA).unwrap();
        assert!((m - 5.0).abs() < 1e-5, "mean = {m}");
    }

    #[test]
    fn test_mean_single() {
        assert!((mean(&[42.0_f32]).unwrap() - 42.0).abs() < 1e-6);
    }

    #[test]
    fn test_mean_empty() {
        assert_eq!(mean(&[] as &[f32]), Err(StatsError::EmptySlice));
    }

    #[test]
    fn test_mean_nan() {
        assert_eq!(mean(&[1.0, f32::NAN, 3.0]), Err(StatsError::NonFiniteValue));
    }

    #[test]
    fn test_mean_inf() {
        assert_eq!(mean(&[1.0, f32::INFINITY]), Err(StatsError::NonFiniteValue));
    }

    #[test]
    fn test_mean_neg_inf() {
        assert_eq!(mean(&[f32::NEG_INFINITY]), Err(StatsError::NonFiniteValue));
    }

    //  variance 

    #[test]
    fn test_variance_reference() {
        let v = variance(&DATA).unwrap();
        assert!((v - 4.0).abs() < 1e-4, "variance = {v}");
    }

    #[test]
    fn test_variance_constant() {
        let v = variance(&[3.0_f32; 100]).unwrap();
        assert!(v.abs() < 1e-5, "variance constante = {v}");
    }

    #[test]
    fn test_variance_empty() {
        assert_eq!(variance(&[] as &[f32]), Err(StatsError::EmptySlice));
    }

    #[test]
    fn test_variance_nan() {
        assert_eq!(variance(&[1.0, f32::NAN]), Err(StatsError::NonFiniteValue));
    }

    #[test]
    fn test_variance_inf() {
        assert_eq!(variance(&[f32::INFINITY, 1.0]), Err(StatsError::NonFiniteValue));
    }

    //  std_dev 

    #[test]
    fn test_std_dev_reference() {
        let s = std_dev(&DATA).unwrap();
        assert!((s - 2.0).abs() < 1e-4, "std_dev = {s}");
    }

    #[test]
    fn test_std_dev_empty() {
        assert_eq!(std_dev(&[] as &[f32]), Err(StatsError::EmptySlice));
    }

    #[test]
    fn test_std_dev_nan() {
        assert_eq!(std_dev(&[f32::NAN]), Err(StatsError::NonFiniteValue));
    }

    //  StreamingStats

    #[test]
    fn test_streaming_mean_reference() {
        let mut acc = StreamingStats::new();
        for &x in &DATA { acc.update(x).unwrap(); }
        let m = acc.mean().unwrap();
        assert!((m - 5.0).abs() < 1e-5, "streaming mean = {m}");
        assert_eq!(acc.count(), 8);
    }

    #[test]
    fn test_streaming_mean_empty() {
        assert_eq!(StreamingStats::new().mean(), Err(StatsError::EmptySlice));
    }

    #[test]
    fn test_streaming_nan_rejected_state_preserved() {
        let mut acc = StreamingStats::new();
        acc.update(1.0).unwrap();
        acc.update(2.0).unwrap();
        // NaN rejeté
        assert_eq!(acc.update(f32::NAN), Err(StatsError::NonFiniteValue));
        // count et mean inchangés
        assert_eq!(acc.count(), 2);
        assert!((acc.mean().unwrap() - 1.5).abs() < 1e-6);
    }

    #[test]
    fn test_streaming_inf_rejected() {
        let mut acc = StreamingStats::new();
        acc.update(5.0).unwrap();
        assert_eq!(acc.update(f32::INFINITY), Err(StatsError::NonFiniteValue));
        assert_eq!(acc.count(), 1);
    }

    #[test]
    fn test_streaming_reset() {
        let mut acc = StreamingStats::new();
        for &x in &DATA { acc.update(x).unwrap(); }
        acc.reset();
        assert_eq!(acc.count(), 0);
        assert_eq!(acc.mean(), Err(StatsError::EmptySlice));
    }

    #[test]
    fn test_streaming_matches_batch() {
        let mut acc = StreamingStats::new();
        for &x in &DATA { acc.update(x).unwrap(); }
        let batch  = mean(&DATA).unwrap();
        let stream = acc.mean().unwrap();
        assert!((batch - stream).abs() < 1e-5, "batch={batch} stream={stream}");
    }
}