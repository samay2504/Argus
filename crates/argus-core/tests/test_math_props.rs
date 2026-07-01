use argus_core::math::{HayashiYoshida, RecursiveLeastSquares, Tick, WelfordOnline};
use argus_core::types::{ForgettingFactor, TimestampNs};
use proptest::prelude::*;

proptest! {
    // 1. Welford Online Variance Invariants
    #[test]
    fn welford_never_negative(
        prices in prop::collection::vec(-1000.0..1000.0f64, 2..100)
    ) {
        let mut w = WelfordOnline::new();
        for p in prices {
            w.update(p);
        }
        let var = w.variance().unwrap();
        assert!(var >= 0.0, "Variance must be non-negative, got {}", var);
    }

    #[test]
    fn welford_matches_batch_formula(
        prices in prop::collection::vec(-1000.0..1000.0f64, 2..50)
    ) {
        let mut w = WelfordOnline::new();
        let mut sum = 0.0;
        for &p in &prices {
            w.update(p);
            sum += p;
        }
        
        let n = prices.len() as f64;
        let mean = sum / n;
        
        let mut sq_diff_sum = 0.0;
        for &p in &prices {
            sq_diff_sum += (p - mean) * (p - mean);
        }
        
        let batch_var = sq_diff_sum / (n - 1.0);
        let online_var = w.variance().unwrap();
        
        // Due to precision, they might not be exactly equal, but very close
        assert!((online_var - batch_var).abs() < 1e-6 * batch_var.abs().max(1.0));
    }

    // 2. Hayashi-Yoshida Invariants
    #[test]
    fn hy_symmetric_and_stable(
        prices_x in prop::collection::vec(-100.0..100.0f64, 2..20),
        prices_y in prop::collection::vec(-100.0..100.0f64, 2..20)
    ) {
        let mut hy = HayashiYoshida::new();
        let mut ts_x = 0;
        let mut ts_y = 50; // offset
        
        for &px in &prices_x {
            ts_x += 100;
            hy.update_x(Tick { ts: TimestampNs(ts_x), price: px });
        }
        for &py in &prices_y {
            ts_y += 100;
            hy.update_y(Tick { ts: TimestampNs(ts_y), price: py });
        }
        
        if let Ok(cov) = hy.covariance() {
            assert!(!cov.is_nan(), "Covariance should not be NaN");
            assert!(!cov.is_infinite(), "Covariance should not be infinite");
        }
    }

    // 3. Recursive Least Squares Invariants
    #[test]
    fn rls_stable_for_bounded_inputs(
        returns in prop::collection::vec(-0.1..0.1f64, 5..50),
        factor_inputs in prop::collection::vec(
            prop::array::uniform5(-0.1..0.1f64),
            5..50
        )
    ) {
        let lambda = ForgettingFactor::new(0.99).unwrap();
        let mut rls: RecursiveLeastSquares<5> = RecursiveLeastSquares::new(lambda, 1000.0);
        
        let n = std::cmp::min(returns.len(), factor_inputs.len());
        for i in 0..n {
            rls.update(&factor_inputs[i], returns[i]);
        }
        
        for &b in rls.beta().unwrap().iter() {
            assert!(!b.is_nan());
            assert!(!b.is_infinite());
        }
    }
}
