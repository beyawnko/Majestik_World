// common/tests/uniform_range_inclusive.rs
#[test]
fn uniform_range_samples_are_inclusive_0_1() {
    use rand::{Rng, distributions::Uniform, thread_rng};

    let mut rng = thread_rng();
    let dist = Uniform::new_inclusive(0.0_f64, 1.0_f64);
    for _ in 0..1000 {
        let x: f64 = rng.sample(dist);
        assert!(x >= 0.0 && x <= 1.0, "sample {} out of [0,1]", x);
    }
}
