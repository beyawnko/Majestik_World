// common/tests/uniform_range_inclusive.rs
use rand::{Rng, SeedableRng, distr::Uniform};
use rand_chacha::ChaCha8Rng;

#[test]
fn uniform_range_samples_are_inclusive_0_1() {
    const SEED: [u8; 32] = [0u8; 32]; // Chosen for determinism; covers bound cases with inclusive dist
    let mut rng = ChaCha8Rng::from_seed(SEED);
    let dist = Uniform::new_inclusive(0.0f64, 1.0f64).expect("valid range");
    // Keep CI fast by default; opt-in longer runs with LONG_TESTS env var.
    const DEFAULT_ITERS: usize = 1_000;
    let iters: usize = if std::env::var("LONG_TESTS").is_ok() {
        10_000
    } else {
        DEFAULT_ITERS
    };
    for _ in 0..iters {
        let v: f64 = rng.sample(dist);
        assert!((0.0..=1.0).contains(&v), "v={} out of [0,1]", v);
    }
}

#[test]
fn uniform_inclusive_integer_range_bounds() {
    let mut rng = ChaCha8Rng::from_seed([1u8; 32]);
    let dist = Uniform::new_inclusive(0u32, 10u32).expect("valid range");
    let mut min_seen = u32::MAX;
    let mut max_seen = u32::MIN;
    const ITERATIONS: usize = 10_000; // good coverage with reasonable runtime
    for _ in 0..ITERATIONS {
        let v = rng.sample(dist);
        assert!((0..=10).contains(&v));
        if v < min_seen {
            min_seen = v;
        }
        if v > max_seen {
            max_seen = v;
        }
    }
    // For integer ranges with 10_000 draws, expect both bounds to be seen.
    assert_eq!(min_seen, 0, "minimum bound 0 was not seen");
    assert_eq!(max_seen, 10, "maximum bound 10 was not seen");
}

#[test]
fn uniform_very_small_float_range_bounds() {
    let mut rng = ChaCha8Rng::from_seed([2u8; 32]);
    let a = 0.1234_f64;
    let b = a + 1e-12;
    let dist = Uniform::new_inclusive(a, b).expect("valid range");
    for _ in 0..1_000 {
        let v: f64 = rng.sample(dist);
        assert!((a..=b).contains(&v), "v={} not in [{}, {}]", v, a, b);
    }
}
