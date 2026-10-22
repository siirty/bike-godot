//! Generate four complete sessions through the same MixedSession used by Godot.
use race_engine::mixed::*;
use serde_json::{json, Value};
use std::path::PathBuf;

fn run(name: &str, cooperative: bool, miss: bool, exposed: bool) -> Value {
    let mut s = MixedSession::new(MixedOptions::new(300.0, 42, cooperative).unwrap()).unwrap();
    let mut samples = vec![];
    let mut counts = [[0u32; 3]; 8];
    let mut ends = vec![Value::Null; 8];
    while !s.is_complete() {
        let block = s.block();
        let power = if miss && block == 2 {
            204.0
        } else {
            s.target_w()
        };
        let step = s.step(power, if exposed { 2 } else { 1 }, None).unwrap();
        let h = step.sample.riders.iter().find(|r| r.id == PLAYER).unwrap();
        counts[block][0] += 1;
        counts[block][1] += u32::from(step.sample.in_band);
        counts[block][2] += u32::from(h.wheel.is_some());
        let front = step
            .sample
            .riders
            .iter()
            .map(|r| r.distance_m)
            .max_by(f64::total_cmp)
            .unwrap();
        ends[block] = json!({"front_gap_m":front-h.distance_m,"reserve_j":h.reserve_j});
        if s.state().simulation_time().as_millis().is_multiple_of(1000) {
            samples.push(step.sample);
        }
    }
    let blocks:Vec<_>=counts.iter().enumerate().map(|(i,c)|json!({"block":i,"seconds":c[0]/20,"band_fraction":f64::from(c[1])/f64::from(c[0]),"sheltered_fraction":f64::from(c[2])/f64::from(c[0]),"end":ends[i]})).collect();
    println!("{name}: {} samples", samples.len());
    for b in &blocks {
        println!("  {b}");
    }
    json!({"name":name,"samples":samples,"blocks":blocks})
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let out = std::env::args()
        .nth(1)
        .map_or_else(|| PathBuf::from("mixed-race-output"), PathBuf::from);
    let runs = [
        run("Prescribed", true, false, false),
        run("Miss chase", true, true, false),
        run("Exposed position", true, false, true),
        run("Pacer declines chase", false, false, false),
    ];
    let data = serde_json::to_string(&json!({"software_only":true,"runs":runs}))?;
    std::fs::create_dir_all(&out)?;
    std::fs::write(out.join("trace.json"), &data)?;
    std::fs::write(
        out.join("mixed-session.html"),
        include_str!("mixed_viewer.html").replace("__DATA__", &data.replace('<', "\\u003c")),
    )?;
    Ok(())
}
