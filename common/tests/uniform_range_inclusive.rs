// common/tests/uniform_range_inclusive.rs
use rand::{Rng, SeedableRng, distr::Uniform};
use rand_chacha::ChaCha8Rng;

#[test]
fn uniform_range_samples_are_inclusive_0_1() {
    let mut rng = ChaCha8Rng::from_seed([0u8; 32]);
    let dist = Uniform::new_inclusive(0.0f64, 1.0f64);
    for _ in 0..10_000 {
        let v: f64 = rng.sample(&dist);
        assert!(v >= 0.0 && v <= 1.0, "v={} out of [0,1]", v);
    }
}

#[test]
fn uniform_inclusive_integer_range_bounds() {
    let mut rng = ChaCha8Rng::from_seed([1u8; 32]);
    let dist = Uniform::new_inclusive(0u32, 10u32);
    let mut min_seen = u32::MAX;
    let mut max_seen = u32::MIN;
    const ITERATIONS: usize = 10_000; // good coverage with reasonable runtime
    for _ in 0..ITERATIONS {
        let v = rng.sample(&dist);
        assert!(v <= 10 && v >= 0);
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
    let dist = Uniform::new_inclusive(a, b);
    for _ in 0..1_000 {
        let v: f64 = rng.sample(&dist);
        assert!(v >= a && v <= b, "v={} not in [{}, {}]", v, a, b);
    }
}
