// common/tests/uniform_range_inclusive.rs
use rand::{
    SeedableRng,
    distr::{Distribution, Uniform},
};
use rand_chacha::{ChaCha8Rng, ChaCha20Rng};

// Chi-square critical values for df = 9 (10 bins - 1): 95% ≈ 16.92, 97.5% ≈
// 19.02, 99% ≈ 21.67. Choose a 20.0 cutoff to reduce flakes while staying near
// the 97.5% quantile.
const CRITICAL_95_DF9: f64 = 16.92;
const CHI_SQUARE_THRESHOLD: f64 = 20.0; // Shared cutoff for all chi-square checks in this module.

#[test]
fn uniform_range_samples_are_inclusive_0_1() {
    const SEED: [u8; 32] = [0u8; 32]; // Chosen for determinism; covers bound cases with inclusive dist
    let mut rng = ChaCha8Rng::from_seed(SEED);
    let dist = Uniform::new_inclusive(0.0f64, 1.0f64).unwrap_or_else(|error| {
        unreachable!("inclusive unit interval rejected: {error}");
    });
    // Keep CI fast by default; opt-in longer runs with LONG_TESTS env var.
    const DEFAULT_ITERS: usize = 1_000;
    let iters: usize = if std::env::var("LONG_TESTS").is_ok() {
        10_000
    } else {
        DEFAULT_ITERS
    };
    for _ in 0..iters {
        let v: f64 = dist.sample(&mut rng);
        assert!((0.0..=1.0).contains(&v), "v={} out of [0,1]", v);
    }
}

#[test]
fn uniform_inclusive_integer_range_bounds() {
    let mut rng = ChaCha8Rng::from_seed([1u8; 32]);
    let dist = Uniform::new_inclusive(0u32, 10u32).unwrap_or_else(|error| {
        unreachable!("inclusive integer range rejected: {error}");
    });
    let mut min_seen = u32::MAX;
    let mut max_seen = u32::MIN;
    const ITERATIONS: usize = 10_000; // good coverage with reasonable runtime
    for _ in 0..ITERATIONS {
        let v = dist.sample(&mut rng);
        assert!((0..=10).contains(&v));
        min_seen = min_seen.min(v);
        max_seen = max_seen.max(v);
    }
    // For integer ranges with 10_000 draws, expect both bounds to be seen.
    assert_eq!(min_seen, 0, "minimum bound 0 was not seen");
    assert_eq!(max_seen, 10, "maximum bound 10 was not seen");
}

#[test]
fn chi_square_uniform_multiple_seeds() {
    let seeds: &[u64] = &[1337, 2025, 987654321];
    let bins = 10usize;
    let draws = 10_000usize;
    let dist = Uniform::new_inclusive(0.0_f64, 1.0_f64).unwrap_or_else(|error| {
        unreachable!("inclusive unit interval rejected: {error}");
    });

    for &seed in seeds {
        let mut rng = ChaCha8Rng::seed_from_u64(seed);
        let mut hist = vec![0usize; bins];
        for _ in 0..draws {
            let x: f64 = dist.sample(&mut rng);
            debug_assert!(
                x >= 0.0,
                "uniform [0, 1] sample must be non-negative before binning, got {x}"
            );
            debug_assert!(
                x <= 1.0,
                "uniform [0, 1] sample must be within [0, 1], got {x}"
            );
            let idx = ((x * bins as f64).floor() as usize).min(bins - 1);
            hist[idx] += 1;
        }

        let expected = (draws as f64) / (bins as f64);
        let chi2: f64 = hist
            .iter()
            .map(|&obs| {
                let o = obs as f64;
                let d = o - expected;
                (d * d) / expected
            })
            .sum();

        assert!(
            chi2 < CHI_SQUARE_THRESHOLD,
            "chi-square too large for seed {}: {} (threshold {}, 95% critical {})",
            seed,
            chi2,
            CHI_SQUARE_THRESHOLD,
            CRITICAL_95_DF9,
        );
    }
}

#[test]
fn uniform_range_chi_square_is_reasonable() {
    // Deterministic stream to keep test stable in CI.
    let mut rng = ChaCha20Rng::from_seed([1u8; 32]);
    let dist = Uniform::new_inclusive(0.0f64, 1.0).unwrap_or_else(|error| {
        unreachable!("inclusive unit interval rejected: {error}");
    });

    const BINS: usize = 10;
    const N: usize = 50_000;
    let mut counts = [0usize; BINS];

    for _ in 0..N {
        let x = dist.sample(&mut rng);
        let idx = if x == 1.0 {
            BINS - 1
        } else {
            (x * BINS as f64) as usize
        };
        counts[idx] += 1;
    }

    let expected = (N as f64) / (BINS as f64);
    let chi_sq: f64 = counts
        .iter()
        .map(|&c| {
            let diff = c as f64 - expected;
            diff * diff / expected
        })
        .sum();

    // 9 degrees of freedom; criticals 95% ≈ 16.92, 97.5% ≈ 19.02, 99% ≈ 21.67.
    // Use 20.0 to reduce flakes while keeping statistical power near the 97.5%
    // cutoff.
    let threshold = CHI_SQUARE_THRESHOLD;
    assert!(
        chi_sq < threshold,
        "chi^2={} exceeds threshold {} with counts={:?}",
        chi_sq,
        threshold,
        counts
    );
}

#[test]
fn uniform_very_small_float_range_bounds() {
    let mut rng = ChaCha8Rng::from_seed([2u8; 32]);
    let a = 0.1234_f64;
    let b = a + 1e-12;
    let dist = Uniform::new_inclusive(a, b).unwrap_or_else(|error| {
        unreachable!("inclusive micro-range rejected: {error}");
    });
    for _ in 0..1_000 {
        let v: f64 = dist.sample(&mut rng);
        assert!((a..=b).contains(&v), "v={} not in [{}, {}]", v, a, b);
    }
}
