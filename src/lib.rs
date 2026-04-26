// Copyright (C) 2026 Jorge Andre Castro
//
// Ce programme est un logiciel libre : vous pouvez le redistribuer et/ou le modifier
// selon les termes de la Licence Publique Générale GNU telle que publiée par la
// Free Software Foundation, soit la version 2 de la licence, soit (à votre convention)
// n'importe quelle version ultérieure.

//! # embedded-stats-f32
//!
//! Statistiques `f32` par Newton-Raphson pour systèmes embarqués `no_std`.
//!
//! Sans dépendance, sans `unsafe`, sans FPU requise.
//!
//! Fournit :
//! - [`mean`]  moyenne arithmétique sur une tranche
//! - [`variance`]  variance (population, non corrigée)
//! - [`std_dev`]  écart type (= √variance)
//! - [`StreamingStats`]  moyenne en ligne (algorithme de Welford), O(1) mémoire
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
//! for &x in &data { s.update(x); }
//! assert!((s.mean().unwrap() - 5.0).abs() < 1e-5);
//! ```

#![no_std]
#![forbid(unsafe_code)]
#![warn(missing_docs)]

use embedded_f32_sqrt::sqrt;

// Erreurs 

/// Erreurs possibles lors d'un calcul statistique.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatsError {
    /// La tranche (ou l'accumulateur) est vide : le calcul est impossible.
    EmptySlice,
}

//  Moyenne

/// Calcule la moyenne arithmétique d'une tranche `f32`.
///
/// Utilise la sommation compensée de Kahan pour minimiser l'erreur
/// d'arrondi sur les grandes tranches.
///
/// # Erreurs
///
/// Retourne [`StatsError::EmptySlice`] si `data` est vide.
///
/// # Exemples
///
/// ```rust
/// use embedded_stats_f32::mean;
///
/// assert!((mean(&[1.0, 2.0, 3.0]).unwrap() - 2.0).abs() < 1e-6);
/// assert_eq!(mean(&[] as &[f32]), Err(embedded_stats_f32::StatsError::EmptySlice));
/// ```
pub fn mean(data: &[f32]) -> Result<f32, StatsError> {
    if data.is_empty() {
        return Err(StatsError::EmptySlice);
    }
    Ok(kahan_sum(data) / data.len() as f32)
}

//  Variance 

/// Calcule la variance de population (non corrigée, diviseur N) d'une tranche `f32`.
///
/// Algorithme de Welford en deux passes pour la stabilité numérique.
///
/// # Erreurs
///
/// Retourne [`StatsError::EmptySlice`] si `data` est vide.
///
/// # Exemples
///
/// ```rust
/// use embedded_stats_f32::variance;
///
/// // Dataset classique : variance = 4.0
/// let data = [2.0_f32, 4.0, 4.0, 4.0, 5.0, 5.0, 7.0, 9.0];
/// assert!((variance(&data).unwrap() - 4.0).abs() < 1e-4);
/// ```
pub fn variance(data: &[f32]) -> Result<f32, StatsError> {
    if data.is_empty() {
        return Err(StatsError::EmptySlice);
    }
    let m = kahan_sum(data) / data.len() as f32;
    // Somme compensée des carrés des écarts
    let mut sum = 0.0_f32;
    let mut comp = 0.0_f32;
    for &x in data {
        let d = x - m;
        let y = d * d - comp;
        let t = sum + y;
        comp = (t - sum) - y;
        sum = t;
    }
    Ok(sum / data.len() as f32)
}

//  Écart type 

/// Calcule l'écart type de population (= √variance) d'une tranche `f32`.
///
/// Utilise [`embedded_f32_sqrt::sqrt`] (Newton-Raphson, pas de FPU requise).
///
/// # Erreurs
///
/// Retourne [`StatsError::EmptySlice`] si `data` est vide.
///
/// # Exemples
///
/// ```rust
/// use embedded_stats_f32::std_dev;
///
/// let data = [2.0_f32, 4.0, 4.0, 4.0, 5.0, 5.0, 7.0, 9.0];
/// assert!((std_dev(&data).unwrap() - 2.0).abs() < 1e-4);
/// ```
pub fn std_dev(data: &[f32]) -> Result<f32, StatsError> {
    let v = variance(data)?;
    // sqrt() ne peut échouer ici : variance >= 0 par construction
    Ok(sqrt(v).unwrap_or(0.0))
}

//  Moyenne streaming (Welford) 

/// Accumulateur de moyenne en ligne, O(1) mémoire.
///
/// Implémente la mise à jour incrémentale de Welford :
/// ```text
/// mean_n = mean_{n-1} + (x_n − mean_{n-1}) / n
/// ```
/// Stable numériquement même pour des millions de points,
/// sans jamais stocker le tableau complet.
///
/// # Exemples
///
/// ```rust
/// use embedded_stats_f32::StreamingStats;
///
/// let mut s = StreamingStats::new();
/// for x in [1.0_f32, 2.0, 3.0, 4.0, 5.0] { s.update(x); }
///
/// assert!((s.mean().unwrap() - 3.0).abs() < 1e-6);
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
    /// Mise à jour O(1), pas d'allocation.
    #[inline]
    pub fn update(&mut self, x: f32) {
        self.count += 1;
        // delta = (x - mean) / n  — formule de Welford
        self.mean += (x - self.mean) / self.count as f32;
    }

    /// Retourne la moyenne courante, ou [`StatsError::EmptySlice`] si aucun point.
    #[inline]
    pub fn mean(&self) -> Result<f32, StatsError> {
        if self.count == 0 {
            Err(StatsError::EmptySlice)
        } else {
            Ok(self.mean)
        }
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
}

impl Default for StreamingStats {
    fn default() -> Self {
        Self::new()
    }
}

//Utilitaire interne

/// Sommation compensée de Kahan , réduit l'erreur d'arrondi à O(ε).
#[inline]
fn kahan_sum(data: &[f32]) -> f32 {
    let mut sum  = 0.0_f32;
    let mut comp = 0.0_f32;
    for &x in data {
        let y = x - comp;
        let t = sum + y;
        comp  = (t - sum) - y;
        sum   = t;
    }
    sum
}

//  Tests 

#[cfg(test)]
mod tests {
    use super::*;

    // Dataset de référence Wikipedia (variance = 4.0, std = 2.0, mean = 5.0)
    const DATA: [f32; 8] = [2.0, 4.0, 4.0, 4.0, 5.0, 5.0, 7.0, 9.0];

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
    fn test_variance_reference() {
        let v = variance(&DATA).unwrap();
        assert!((v - 4.0).abs() < 1e-4, "variance = {v}");
    }

    #[test]
    fn test_variance_constant() {
        // Variance d'une constante = 0
        let v = variance(&[3.0_f32; 100]).unwrap();
        assert!(v.abs() < 1e-5, "variance constante = {v}");
    }

    #[test]
    fn test_variance_empty() {
        assert_eq!(variance(&[] as &[f32]), Err(StatsError::EmptySlice));
    }

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
    fn test_streaming_mean_reference() {
        let mut acc = StreamingStats::new();
        for &x in &DATA { acc.update(x); }
        let m = acc.mean().unwrap();
        assert!((m - 5.0).abs() < 1e-5, "streaming mean = {m}");
        assert_eq!(acc.count(), 8);
    }

    #[test]
    fn test_streaming_mean_empty() {
        let acc = StreamingStats::new();
        assert_eq!(acc.mean(), Err(StatsError::EmptySlice));
    }

    #[test]
    fn test_streaming_reset() {
        let mut acc = StreamingStats::new();
        for &x in &DATA { acc.update(x); }
        acc.reset();
        assert_eq!(acc.count(), 0);
        assert_eq!(acc.mean(), Err(StatsError::EmptySlice));
    }

    #[test]
    fn test_streaming_incremental_matches_batch() {
        let mut acc = StreamingStats::new();
        for &x in &DATA { acc.update(x); }
        let batch   = mean(&DATA).unwrap();
        let stream  = acc.mean().unwrap();
        assert!((batch - stream).abs() < 1e-5, "batch={batch} stream={stream}");
    }
}