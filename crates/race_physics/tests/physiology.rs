//! Independent energy-accounting and analytic-solution tests.
use race_physics::{step_physiology, PhysiologyParams};
use std::time::Duration;
fn p() -> PhysiologyParams {
    PhysiologyParams::new(250.0, 20_000.0, 1250.0).unwrap()
}
fn close(a: f64, b: f64) {
    assert!((a - b).abs() < 1e-8, "{a} != {b}");
}
#[test]
fn below_cp_recovers_with_the_skiba_time_constant() {
    // CP=250, P=150: tau=20000/100=200 s. After tau, 1/e of the deficit remains.
    let s = step_physiology(&p(), 10_000.0, 150.0, Duration::from_secs(200)).unwrap();
    close(s.balance_j, 16_321.205588285578);
    close(s.delivered_power_w, 150.0);
}
#[test]
fn above_cp_consumes_exactly_the_excess_work() {
    let s = step_physiology(&p(), 20_000.0, 350.0, Duration::from_secs(10)).unwrap();
    close(s.balance_j, 19_000.0);
    close(s.delivered_power_w, 350.0);
}
#[test]
fn maximal_sprint_decays_analytically_and_cannot_spend_missing_energy() {
    // k=(1250-250)/20000 = 0.05 /s: an all-out 20 s sprint leaves W'/e.
    let s = step_physiology(&p(), 20_000.0, 5000.0, Duration::from_secs(20)).unwrap();
    close(s.balance_j, 7357.588823428847);
    close((s.delivered_power_w - 250.0) * 20.0, 20_000.0 - s.balance_j);
    let spent = step_physiology(&p(), 0.0, 5000.0, Duration::from_secs(1)).unwrap();
    close(spent.delivered_power_w, 250.0);
    close(spent.balance_j, 0.0);
}
#[test]
fn splitting_an_interval_agrees_even_when_the_ceiling_starts_binding() {
    for requested in [0.0, 150.0, 250.0, 800.0, 5000.0] {
        let whole = step_physiology(&p(), 20_000.0, requested, Duration::from_secs(60)).unwrap();
        let mut b = 20_000.0;
        let mut work = 0.0;
        for _ in 0..1200 {
            let s = step_physiology(&p(), b, requested, Duration::from_millis(50)).unwrap();
            b = s.balance_j;
            work += s.delivered_power_w * 0.05;
        }
        assert!((b - whole.balance_j).abs() < 1e-6);
        assert!((work - whole.delivered_power_w * 60.0).abs() < 1e-6);
    }
}
#[test]
fn cp_holds_balance_and_recovery_improves_a_second_sprint() {
    let first = step_physiology(&p(), 20_000.0, 2000.0, Duration::from_secs(10)).unwrap();
    let held = step_physiology(&p(), first.balance_j, 250.0, Duration::from_secs(10)).unwrap();
    close(first.balance_j, held.balance_j);
    let tired = step_physiology(&p(), first.balance_j, 2000.0, Duration::from_secs(10)).unwrap();
    assert!(tired.delivered_power_w < first.delivered_power_w);
    let recovered = step_physiology(&p(), first.balance_j, 0.0, Duration::from_secs(120)).unwrap();
    let again =
        step_physiology(&p(), recovered.balance_j, 2000.0, Duration::from_secs(10)).unwrap();
    assert!(again.delivered_power_w > tired.delivered_power_w);
}
#[test]
fn invalid_values_are_rejected_including_deserialization() {
    for x in [f64::NAN, f64::INFINITY, -1.0, 0.0] {
        assert!(PhysiologyParams::new(x, 20_000.0, 1000.0).is_err());
    }
    assert!(PhysiologyParams::new(250.0, 20_000.0, 250.0).is_err());
    assert!(PhysiologyParams::new(1.0, f64::MIN_POSITIVE, f64::MAX).is_err());
    assert!(PhysiologyParams::new(f64::MIN_POSITIVE, f64::MAX, 2.0 * f64::MIN_POSITIVE).is_err());
    for b in [f64::NAN, f64::INFINITY, -1.0, 20001.0] {
        assert!(step_physiology(&p(), b, 100.0, Duration::from_secs(1)).is_err());
    }
    assert!(step_physiology(&p(), 0.0, 0.0, Duration::ZERO).is_err());
    assert!(serde_json::from_str::<PhysiologyParams>(
        r#"{"criticalPowerW":250,"wPrimeJ":0,"peakPowerW":1000}"#
    )
    .is_err());
}
