use criterion::{criterion_group, criterion_main, Criterion, black_box};
use argus_core::math::{WelfordOnline, HayashiYoshida, RecursiveLeastSquares, Tick};
use argus_core::types::{ForgettingFactor, TimestampNs, AssetId};
use argus_core::covariance::CovarianceManager;
use argus_core::risk_policy::{StaticLimitPolicy, RiskPolicy, ExposureSnapshot};

fn bench_welford_update(c: &mut Criterion) {
    let mut welford = WelfordOnline::new();
    let mut i = 0u64;

    c.bench_function("welford_single_update", |b| {
        b.iter(|| {
            i += 1;
            welford.update(black_box(100.0 + (i as f64) * 0.001));
        });
    });
}

fn bench_hayashi_yoshida_update(c: &mut Criterion) {
    let mut hy = HayashiYoshida::new();
    let mut ts = 1_000_000_000i64;

    // Seed with initial ticks
    hy.update_x(Tick { ts: TimestampNs(0), price: 100.0 });
    hy.update_y(Tick { ts: TimestampNs(500_000_000), price: 50.0 });

    c.bench_function("hy_pair_update", |b| {
        b.iter(|| {
            ts += 1_000_000_000;
            let tick = Tick {
                ts: TimestampNs(black_box(ts)),
                price: black_box(100.0 + (ts as f64) * 0.000001),
            };
            hy.update_x(tick);
        });
    });
}

fn bench_rls_update_k5(c: &mut Criterion) {
    let lambda = ForgettingFactor::new(0.99).unwrap();
    let mut rls: RecursiveLeastSquares<5> = RecursiveLeastSquares::new(lambda, 1000.0);
    let mut i = 0u64;

    c.bench_function("rls_step_k5", |b| {
        b.iter(|| {
            i += 1;
            let factors = [
                black_box(0.01 * (i as f64)),
                black_box(-0.005 * (i as f64)),
                black_box(0.002 * (i as f64)),
                black_box(0.008 * (i as f64)),
                black_box(-0.003 * (i as f64)),
            ];
            let asset_return = black_box(0.001 * (i as f64));
            rls.update(&factors, asset_return);
        });
    });
}

fn bench_rls_update_k10(c: &mut Criterion) {
    let lambda = ForgettingFactor::new(0.99).unwrap();
    let mut rls: RecursiveLeastSquares<10> = RecursiveLeastSquares::new(lambda, 1000.0);
    let mut i = 0u64;

    c.bench_function("rls_step_k10", |b| {
        b.iter(|| {
            i += 1;
            let factors = [
                black_box(0.01 * (i as f64)),
                black_box(-0.005 * (i as f64)),
                black_box(0.002 * (i as f64)),
                black_box(0.008 * (i as f64)),
                black_box(-0.003 * (i as f64)),
                black_box(0.006 * (i as f64)),
                black_box(-0.004 * (i as f64)),
                black_box(0.007 * (i as f64)),
                black_box(-0.002 * (i as f64)),
                black_box(0.009 * (i as f64)),
            ];
            let asset_return = black_box(0.001 * (i as f64));
            rls.update(&factors, asset_return);
        });
    });
}

fn bench_covariance_manager_update(c: &mut Criterion) {
    // 100 assets = 4950 HY pairs
    let mut mgr = CovarianceManager::new(100);
    let mut ts = 1_000_000_000i64;
    
    c.bench_function("covariance_manager_update_100_assets", |b| {
        b.iter(|| {
            ts += 1_000_000_000;
            let _ = mgr.update_tick(
                black_box(42),
                TimestampNs(black_box(ts)),
                black_box(100.0)
            );
        });
    });
}

fn bench_risk_policy_evaluate(c: &mut Criterion) {
    let policy = StaticLimitPolicy::new(1.0, 1.0, 10.0);
    let n = 100;
    
    let snapshot = ExposureSnapshot {
        asset_ids: (0..n).map(|i| AssetId(i as u32)).collect(),
        variances: vec![0.05; n],
        covariance_matrix: vec![vec![0.02; n]; n],
        factor_betas: vec![vec![0.5; 5]; n],
    };
    
    c.bench_function("risk_policy_eval_100_assets", |b| {
        b.iter(|| {
            black_box(policy.evaluate(black_box(&snapshot)));
        });
    });
}

criterion_group!(
    benches,
    bench_welford_update,
    bench_hayashi_yoshida_update,
    bench_rls_update_k5,
    bench_rls_update_k10,
    bench_covariance_manager_update,
    bench_risk_policy_evaluate,
);
criterion_main!(benches);
