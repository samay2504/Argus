//! Risk policy evaluation framework.
//!
//! Provides the [`RiskPolicy`] trait for extensible limit checking
//! and the [`StaticLimitPolicy`] as the built-in hard-threshold policy.

use crate::types::AssetId;

/// Snapshot of current risk exposures for policy evaluation.
///
/// Constructed by the engine on each evaluation cycle and passed to
/// every registered [`RiskPolicy`].
#[derive(Debug, Clone)]
pub struct ExposureSnapshot {
    /// Asset identifiers in the current universe.
    pub asset_ids: Vec<AssetId>,
    /// Per-asset variance estimates (same order as `asset_ids`).
    pub variances: Vec<f64>,
    /// Full covariance matrix, `covariance_matrix[i][j]`.
    pub covariance_matrix: Vec<Vec<f64>>,
    /// Factor beta loadings per asset, `factor_betas[asset][factor]`.
    pub factor_betas: Vec<Vec<f64>>,
}

/// Action recommended by a risk policy.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub enum RiskAction {
    /// No breach detected — within all limits.
    NoAction,
    /// A hard limit has been breached.
    LimitBreach {
        /// The asset that triggered the breach.
        asset_id: AssetId,
        /// Name of the metric that breached (e.g. "variance", "correlation").
        metric: String,
        /// Current observed value of the metric.
        value: f64,
        /// The configured limit that was exceeded.
        limit: f64,
    },
}

/// Extensibility seam for risk policies.
///
/// Implementations must be `Send + Sync` to allow concurrent evaluation
/// across shard threads.
pub trait RiskPolicy: Send + Sync {
    /// Evaluate the policy against the given exposure snapshot.
    ///
    /// Returns a (possibly empty) list of [`RiskAction`]s.
    fn evaluate(&self, exposures: &ExposureSnapshot) -> Vec<RiskAction>;
}

/// Static limit policy — checks hard thresholds on variance, correlation,
/// and factor beta magnitudes.
pub struct StaticLimitPolicy {
    /// Maximum allowed single-asset variance.
    pub max_single_asset_variance: f64,
    /// Maximum allowed absolute pairwise correlation.
    pub max_pairwise_correlation: f64,
    /// Maximum allowed absolute factor beta magnitude.
    pub max_factor_beta_magnitude: f64,
}

impl StaticLimitPolicy {
    /// Creates a new `StaticLimitPolicy` with the given thresholds.
    pub fn new(max_var: f64, max_corr: f64, max_beta: f64) -> Self {
        Self {
            max_single_asset_variance: max_var,
            max_pairwise_correlation: max_corr,
            max_factor_beta_magnitude: max_beta,
        }
    }
}

impl RiskPolicy for StaticLimitPolicy {
    fn evaluate(&self, exposures: &ExposureSnapshot) -> Vec<RiskAction> {
        let mut actions = Vec::new();
        let n = exposures.asset_ids.len();

        // Check per-asset variance limits.
        for (idx, &var) in exposures.variances.iter().enumerate() {
            if var > self.max_single_asset_variance {
                actions.push(RiskAction::LimitBreach {
                    asset_id: exposures.asset_ids[idx],
                    metric: "variance".into(),
                    value: var,
                    limit: self.max_single_asset_variance,
                });
            }
        }

        // Check pairwise correlation limits from the covariance matrix.
        for i in 0..n {
            for j in (i + 1)..n {
                let var_i = exposures.variances[i];
                let var_j = exposures.variances[j];
                let denom = (var_i * var_j).sqrt();
                if denom < f64::EPSILON {
                    continue;
                }
                let corr = exposures.covariance_matrix[i][j] / denom;
                if corr.abs() > self.max_pairwise_correlation {
                    actions.push(RiskAction::LimitBreach {
                        asset_id: exposures.asset_ids[i],
                        metric: format!(
                            "correlation({}, {})",
                            exposures.asset_ids[i], exposures.asset_ids[j]
                        ),
                        value: corr,
                        limit: self.max_pairwise_correlation,
                    });
                }
            }
        }

        // Check factor beta magnitude limits.
        for (idx, betas) in exposures.factor_betas.iter().enumerate() {
            for (k, &beta) in betas.iter().enumerate() {
                if beta.abs() > self.max_factor_beta_magnitude {
                    actions.push(RiskAction::LimitBreach {
                        asset_id: exposures.asset_ids[idx],
                        metric: format!("factor_beta[{k}]"),
                        value: beta,
                        limit: self.max_factor_beta_magnitude,
                    });
                }
            }
        }

        actions
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_snapshot() -> ExposureSnapshot {
        ExposureSnapshot {
            asset_ids: vec![AssetId(1), AssetId(2)],
            variances: vec![0.05, 0.10],
            covariance_matrix: vec![vec![0.05, 0.04], vec![0.04, 0.10]],
            factor_betas: vec![vec![0.8, -0.2], vec![1.5, 0.3]],
        }
    }

    #[test]
    fn no_breach_when_within_limits() {
        let policy = StaticLimitPolicy::new(1.0, 1.0, 10.0);
        let actions = policy.evaluate(&make_snapshot());
        assert!(actions.iter().all(|a| matches!(a, RiskAction::NoAction)) || actions.is_empty());
    }

    #[test]
    fn variance_breach_detected() {
        let policy = StaticLimitPolicy::new(0.08, 1.0, 10.0);
        let actions = policy.evaluate(&make_snapshot());
        assert!(actions.iter().any(|a| matches!(
            a,
            RiskAction::LimitBreach { metric, .. } if metric == "variance"
        )));
    }

    #[test]
    fn factor_beta_breach_detected() {
        let policy = StaticLimitPolicy::new(1.0, 1.0, 1.0);
        let actions = policy.evaluate(&make_snapshot());
        assert!(actions.iter().any(|a| matches!(
            a,
            RiskAction::LimitBreach { metric, .. } if metric.starts_with("factor_beta")
        )));
    }
}
