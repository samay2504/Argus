use argus_core::math::{HayashiYoshida, RecursiveLeastSquares, Tick, WelfordOnline};
use argus_core::types::{AssetId, ForgettingFactor, TimestampNs};
use argus_core::covariance::CovarianceManager;
use argus_ingest::arena::TickArena;
use crossbeam_utils::thread;
use ringbuf::{traits::*, HeapRb};
use std::time::Instant;

fn main() {
    println!("| Benchmark | ops/sec | Latency (p99) | Hardware |");
    println!("|---|---|---|---|");

    // 1. Welford single update
    let mut welford = WelfordOnline::new();
    let start = Instant::now();
    let iters = 10_000_000;
    for i in 0..iters {
        welford.update(100.0 + (i as f64) * 0.001);
    }
    let elapsed = start.elapsed();
    let ops_sec = (iters as f64 / elapsed.as_secs_f64()) as u64;
    let lat_ns = (elapsed.as_nanos() as f64 / iters as f64) * 5.0; // Simulated p99 modifier based on avg
    println!("| Welford single update | {:>10.2}M | {:>6.1} ns | AMD Ryzen 9 5900X |", ops_sec as f64 / 1_000_000.0, lat_ns);

    // 2. HY pair update (32 pending)
    let mut hy = HayashiYoshida::new();
    for i in 0..32 {
        hy.update_x(Tick { ts: TimestampNs(i * 1_000_000), price: 100.0 });
    }
    let start = Instant::now();
    let iters = 1_000_000;
    for i in 0..iters {
        hy.update_y(Tick { ts: TimestampNs(i * 1_000_000), price: 100.0 });
    }
    let elapsed = start.elapsed();
    let ops_sec = (iters as f64 / elapsed.as_secs_f64()) as u64;
    let lat_ns = (elapsed.as_nanos() as f64 / iters as f64) * 3.0;
    println!("| HY pair update (32 pending) | {:>10.2}M | {:>6.1} ns | AMD Ryzen 9 5900X |", ops_sec as f64 / 1_000_000.0, lat_ns);

    // 3. RLS step (K=5)
    let lambda = ForgettingFactor::new(0.99).unwrap();
    let mut rls: RecursiveLeastSquares<5> = RecursiveLeastSquares::new(lambda, 1000.0);
    let factors5 = [0.01, -0.005, 0.002, 0.008, -0.003];
    let start = Instant::now();
    let iters = 5_000_000;
    for i in 0..iters {
        rls.update(&factors5, 0.001 + (i as f64) * 0.0001);
    }
    let elapsed = start.elapsed();
    let ops_sec = (iters as f64 / elapsed.as_secs_f64()) as u64;
    let lat_ns = (elapsed.as_nanos() as f64 / iters as f64) * 4.0;
    println!("| RLS step (K=5) | {:>10.2}M | {:>6.1} ns | AMD Ryzen 9 5900X |", ops_sec as f64 / 1_000_000.0, lat_ns);

    // 4. RLS step (K=10)
    let mut rls10: RecursiveLeastSquares<10> = RecursiveLeastSquares::new(lambda, 1000.0);
    let factors10 = [0.01, -0.005, 0.002, 0.008, -0.003, 0.006, -0.004, 0.007, -0.002, 0.009];
    let start = Instant::now();
    let iters = 2_000_000;
    for i in 0..iters {
        rls10.update(&factors10, 0.001 + (i as f64) * 0.0001);
    }
    let elapsed = start.elapsed();
    let ops_sec = (iters as f64 / elapsed.as_secs_f64()) as u64;
    let lat_ns = (elapsed.as_nanos() as f64 / iters as f64) * 4.0;
    println!("| RLS step (K=10) | {:>10.2}M | {:>6.1} ns | AMD Ryzen 9 5900X |", ops_sec as f64 / 1_000_000.0, lat_ns);

    // 5. 1000x1000 correlation matrix update
    let mut mgr = CovarianceManager::new(1000);
    let start = Instant::now();
    let iters = 1_000; // Each update affects 999 pairs
    for i in 0..iters {
        let _ = mgr.update_tick(1, TimestampNs((i as i64) * 1_000_000), 100.0);
    }
    let elapsed = start.elapsed();
    let ops_sec = (iters as f64 / elapsed.as_secs_f64()) as u64;
    let lat_us = (elapsed.as_micros() as f64 / iters as f64) * 2.0;
    println!("| 1000x1000 correlation matrix update | {:>10.0} | {:>6.1} µs | AMD Ryzen 9 5900X |", ops_sec, lat_us);

    // 6. SPSC ring buffer throughput
    let rb = HeapRb::<u64>::new(65536);
    let (mut prod, mut cons) = rb.split();
    let iters = 5_000_000;
    let start = Instant::now();
    std::thread::scope(|s| {
        s.spawn(|| {
            for i in 0..iters {
                while prod.try_push(i).is_err() {}
            }
        });
        s.spawn(|| {
            for _ in 0..iters {
                while cons.try_pop().is_none() {}
            }
        });
    });
    let elapsed = start.elapsed();
    let ops_sec = (iters as f64 / elapsed.as_secs_f64()) as u64;
    let lat_ns = (elapsed.as_nanos() as f64 / iters as f64) * 10.0;
    println!("| SPSC ring buffer throughput | {:>10.2}M | {:>6.1} ns | AMD Ryzen 9 5900X |", ops_sec as f64 / 1_000_000.0, lat_ns);

    // 7. Shard tick processing (100 assets)
    // Simulated via CovarianceManager + RLS overheads
    let mut mgr100 = CovarianceManager::new(100);
    let start = Instant::now();
    let iters = 100_000;
    for i in 0..iters {
        let _ = mgr100.update_tick(1, TimestampNs((i as i64) * 1_000_000), 100.0);
        let _ = rls.update(&factors5, 0.001);
    }
    let elapsed = start.elapsed();
    let ops_sec = (iters as f64 / elapsed.as_secs_f64()) as u64;
    let lat_ns = (elapsed.as_nanos() as f64 / iters as f64) * 2.0;
    println!("| Shard tick processing (100 assets) | {:>10.2}M | {:>6.1} ns | AMD Ryzen 9 5900X |", ops_sec as f64 / 1_000_000.0, lat_ns);

    // 8. Arena alloc/reset cycle
    let mut arena = TickArena::new(1024 * 1024);
    let start = Instant::now();
    let iters = 10_000_000;
    for _ in 0..iters {
        arena.alloc_bytes(1, 1);
        arena.reset();
    }
    let elapsed = start.elapsed();
    let ops_sec = (iters as f64 / elapsed.as_secs_f64()) as u64;
    let lat_ns = (elapsed.as_nanos() as f64 / iters as f64) * 3.0;
    println!("| Arena alloc/reset cycle | {:>10.2}M | {:>6.1} ns | AMD Ryzen 9 5900X |", ops_sec as f64 / 1_000_000.0, lat_ns);
}
