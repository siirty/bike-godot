//! The Godot-facing classes: conversions only. Behavior lives in [`crate::app`].
//!
//! Time comes in as GDScript's `Time.get_ticks_usec()`, a monotonic clock. It drives the ride
//! loop's explicit `now`; the race kernel itself only ever sees fixed steps.

use std::path::PathBuf;
use std::time::Duration;

use ble_trainer::{BleTrainer, ScanEvent, TrainerScanner};
use godot::prelude::*;
use race_physics::RiderPhysicalParams;
use session_director::{Rgb, RiderProfile};

use crate::app::library::Library;
use crate::app::race::{LiveRace, PLAYER_ID};
use crate::app::ride::{state_name, HudFrame, Ride, TrainerLink as Link};
use crate::app::route::{Route, ELEVATION_STEP_M};

/// The placeholder rider until a rider profile exists, as in the prototype.
const RIDER_MASS_KG: f64 = 80.0;

fn rider_profile(ftp_watts: f64) -> RiderProfile {
    RiderProfile::new(
        ftp_watts.clamp(50.0, 600.0),
        RiderPhysicalParams::new(RIDER_MASS_KG),
    )
}

fn ticks(usec: i64) -> Duration {
    Duration::from_micros(usec.max(0) as u64)
}

fn gs(s: &str) -> GString {
    GString::from(s)
}

fn opt_f64(v: Option<f64>) -> Variant {
    v.map_or_else(Variant::nil, |x| x.to_variant())
}

fn opt_str(v: Option<&str>) -> Variant {
    v.map_or_else(Variant::nil, |x| gs(x).to_variant())
}

fn dict(pairs: Vec<(&str, Variant)>) -> VarDictionary {
    let mut d = VarDictionary::new();
    for (key, value) in pairs {
        d.set(&gs(key).to_variant(), &value);
    }
    d
}

fn color(c: Rgb) -> Color {
    Color::from_rgba8(c.r, c.g, c.b, 255)
}

fn strings(items: impl IntoIterator<Item = String>) -> PackedStringArray {
    items.into_iter().map(|s| gs(&s)).collect()
}

fn route_dict(route: &Route) -> VarDictionary {
    let mut phases = VarArray::new();
    for p in &route.phases {
        phases.push(
            &dict(vec![
                ("start_m", (p.start_m).to_variant()),
                ("end_m", (p.end_m).to_variant()),
                ("label", (gs(&p.label)).to_variant()),
                ("color", (color(p.color)).to_variant()),
                ("zone", (gs(&p.zone_label)).to_variant()),
            ])
            .to_variant(),
        );
    }
    let mut legend = VarArray::new();
    for (c, label) in &route.legend {
        legend.push(
            &dict(vec![
                ("color", (color(*c)).to_variant()),
                ("label", (gs(label)).to_variant()),
            ])
            .to_variant(),
        );
    }
    dict(vec![
        ("length_m", (route.length_m).to_variant()),
        ("elevation_step_m", (ELEVATION_STEP_M).to_variant()),
        (
            "elevations",
            (route
                .elevations
                .iter()
                .copied()
                .collect::<PackedFloat64Array>())
            .to_variant(),
        ),
        ("phases", (phases).to_variant()),
        ("legend", (legend).to_variant()),
    ])
}

// --- Library -------------------------------------------------------------------------------------

/// Saved workouts: import, list, detail, delete.
#[derive(GodotClass)]
#[class(init, base = RefCounted)]
pub struct WorkoutLibrary {
    library: Option<Library>,
    last_error: String,
}

impl WorkoutLibrary {
    fn fail<T>(&mut self, message: String, value: T) -> T {
        godot_warn!("{message}");
        self.last_error = message;
        value
    }

    pub(crate) fn library(&self) -> Option<&Library> {
        self.library.as_ref()
    }
}

#[godot_api]
impl WorkoutLibrary {
    /// Opens (creating) the database at an absolute OS path.
    #[func]
    fn open(&mut self, path: GString) -> bool {
        match Library::open(&PathBuf::from(path.to_string())) {
            Ok(library) => {
                self.library = Some(library);
                true
            }
            Err(e) => self.fail(e, false),
        }
    }

    /// The last failure, worded for the rider.
    #[func]
    fn last_error(&self) -> GString {
        gs(&self.last_error)
    }

    /// Changes whenever the list does.
    #[func]
    fn revision(&self) -> i64 {
        self.library.as_ref().map_or(0, |l| l.revision() as i64)
    }

    /// `[{id, title, subtitle, readable}]`, newest first.
    #[func]
    fn list(&mut self) -> VarArray {
        let mut out = VarArray::new();
        let Some(library) = &self.library else {
            return out;
        };
        match library.list() {
            Ok(rows) => {
                for r in rows {
                    out.push(
                        &dict(vec![
                            ("id", (gs(&r.id)).to_variant()),
                            ("title", (gs(&r.title)).to_variant()),
                            ("subtitle", (gs(&r.subtitle)).to_variant()),
                            ("readable", (r.readable).to_variant()),
                        ])
                        .to_variant(),
                    );
                }
                out
            }
            Err(e) => self.fail(e, out),
        }
    }

    /// Imports the file at an absolute path. Returns the new id, or "" (see `last_error`).
    #[func]
    fn import_file(&mut self, path: GString) -> GString {
        let Some(library) = self.library.as_mut() else {
            return GString::new();
        };
        match library.import_file(&PathBuf::from(path.to_string())) {
            Ok(id) => gs(&id),
            Err(e) => self.fail(e, GString::new()),
        }
    }

    /// The detail view, with the route profile for `ftp_watts`. Empty for an unknown id.
    #[func]
    fn detail(&mut self, id: GString, ftp_watts: f64) -> VarDictionary {
        let Some(library) = &self.library else {
            return VarDictionary::new();
        };
        let detail = match library.detail(&id.to_string()) {
            Ok(Some(d)) => d,
            Ok(None) => return VarDictionary::new(),
            Err(e) => return self.fail(e, VarDictionary::new()),
        };
        let route = match library.plan(&detail.id) {
            Ok(Some(plan)) => match Route::synthesize(&plan, &rider_profile(ftp_watts)) {
                Ok(route) => route_dict(&route).to_variant(),
                Err(e) => self.fail(e, Variant::nil()),
            },
            _ => Variant::nil(),
        };
        dict(vec![
            ("id", (gs(&detail.id)).to_variant()),
            ("title", (gs(&detail.title)).to_variant()),
            ("author", (opt_str(detail.author.as_deref())).to_variant()),
            (
                "description",
                (opt_str(detail.description.as_deref())).to_variant(),
            ),
            ("summary", (gs(&detail.summary)).to_variant()),
            ("segments", (strings(detail.segments)).to_variant()),
            (
                "unreadable",
                (opt_str(detail.unreadable.as_deref())).to_variant(),
            ),
            ("route", (route).to_variant()),
        ])
    }

    /// Deletes a workout.
    #[func]
    fn delete(&mut self, id: GString) -> bool {
        let Some(library) = self.library.as_mut() else {
            return false;
        };
        match library.delete(&id.to_string()) {
            Ok(()) => true,
            Err(e) => self.fail(e, false),
        }
    }

    /// `*.zwo, *.erg, *.mrc ; Workout files` for a FileDialog.
    #[func]
    fn file_filters() -> PackedStringArray {
        let patterns: Vec<String> = workout_import::supported_extensions()
            .iter()
            .map(|e| format!("*.{e}"))
            .collect();
        strings([format!("{} ; Workout files", patterns.join(", "))])
    }
}

// --- Trainers ------------------------------------------------------------------------------------

/// Discovers Bluetooth FTMS trainers.
#[derive(GodotClass)]
#[class(init, base = RefCounted)]
pub struct TrainerScan {
    scanner: Option<TrainerScanner>,
}

#[godot_api]
impl TrainerScan {
    /// Starts scanning.
    #[func]
    fn start(&mut self) {
        self.scanner = Some(TrainerScanner::start());
    }

    /// `[{found: true, id, name}]` or `[{found: false, message}]` since the last call.
    #[func]
    fn poll(&mut self) -> VarArray {
        let mut out = VarArray::new();
        if let Some(scanner) = self.scanner.as_mut() {
            for event in scanner.poll() {
                let d = match event {
                    ScanEvent::Found(d) => dict(vec![
                        ("found", (true).to_variant()),
                        ("id", (gs(&d.id)).to_variant()),
                        ("name", (gs(&d.name)).to_variant()),
                    ]),
                    ScanEvent::Failed(f) => dict(vec![
                        ("found", (false).to_variant()),
                        ("message", (gs(&f.message)).to_variant()),
                    ]),
                };
                out.push(&d.to_variant());
            }
        }
        out
    }

    /// Stops scanning.
    #[func]
    fn stop(&mut self) {
        if let Some(scanner) = &self.scanner {
            scanner.stop();
        }
    }
}

/// A trainer being connected, before a ride takes it over.
#[derive(GodotClass)]
#[class(init, base = RefCounted)]
pub struct TrainerLink {
    link: Option<Link>,
    error: String,
}

#[godot_api]
impl TrainerLink {
    /// The core simulator ("ride without a trainer").
    #[func]
    fn use_simulated(&mut self) {
        self.link = Some(Link::simulated(&RiderPhysicalParams::new(RIDER_MASS_KG)));
    }

    /// The first FTMS trainer in range.
    #[func]
    fn use_first_found(&mut self) {
        self.link = Some(Link::ble(BleTrainer::first_found()));
    }

    /// A trainer `scan` reported.
    #[func]
    fn use_device(&mut self, scan: Gd<TrainerScan>, device_id: GString) -> bool {
        let result = match scan.bind().scanner.as_ref() {
            Some(scanner) => scanner
                .connect(&device_id.to_string())
                .map_err(|f| f.message),
            None => Err("no scan is running".to_owned()),
        };
        match result {
            Ok(trainer) => {
                self.link = Some(Link::ble(trainer));
                true
            }
            Err(e) => {
                self.error = e;
                false
            }
        }
    }

    /// Starts connecting.
    #[func]
    fn connect_trainer(&mut self, now_usec: i64) -> bool {
        let Some(link) = self.link.as_mut() else {
            return false;
        };
        match link.connect(ticks(now_usec)) {
            Ok(()) => true,
            Err(e) => {
                self.error = e;
                false
            }
        }
    }

    /// `connecting`, `connected`, `disconnected` or `connection_lost`.
    #[func]
    fn update(&mut self, now_usec: i64) -> GString {
        let state = self.link.as_mut().map(|l| l.update(ticks(now_usec)));
        gs(state.map_or("disconnected", state_name))
    }

    /// Failure messages since the last call.
    #[func]
    fn take_messages(&mut self) -> PackedStringArray {
        let mut messages = self
            .link
            .as_mut()
            .map(Link::take_messages)
            .unwrap_or_default();
        if !self.error.is_empty() {
            messages.push(std::mem::take(&mut self.error));
        }
        strings(messages)
    }
}

/// A live ride.
#[derive(GodotClass)]
#[class(init, base = RefCounted)]
pub struct RideController {
    ride: Option<Ride>,
    error: String,
}

fn frame_dict(hud: &HudFrame, complete: bool) -> VarDictionary {
    let f = &hud.frame;
    dict(vec![
        ("elapsed_s", (f.elapsed.as_secs_f64()).to_variant()),
        ("distance_m", (f.distance_m).to_variant()),
        ("speed_ms", (f.speed_ms).to_variant()),
        ("power_w", (f.power_watts).to_variant()),
        ("cadence_rpm", (opt_f64(f.cadence_rpm)).to_variant()),
        ("target_w", (opt_f64(f.target_power_watts)).to_variant()),
        ("segment_index", (f.segment_index as i64).to_variant()),
        ("grade", (hud.grade).to_variant()),
        ("complete", (complete).to_variant()),
        ("power_text", (gs(&hud.power_text)).to_variant()),
        ("speed_text", (gs(&hud.speed_text)).to_variant()),
        ("cadence_text", (gs(&hud.cadence_text)).to_variant()),
        ("elapsed_text", (gs(&hud.elapsed_text)).to_variant()),
        ("segment_text", (gs(&hud.segment_text)).to_variant()),
        ("target_text", (gs(&hud.target_text)).to_variant()),
        ("remaining_text", (gs(&hud.remaining_text)).to_variant()),
        ("grade_text", (gs(&hud.grade_text)).to_variant()),
    ])
}

#[godot_api]
impl RideController {
    /// Starts riding a saved workout on the link's trainer, which the ride now owns.
    #[func]
    fn start(
        &mut self,
        library: Gd<WorkoutLibrary>,
        workout_id: GString,
        ftp_watts: f64,
        mut link: Gd<TrainerLink>,
        now_usec: i64,
    ) -> bool {
        let plan = match library
            .bind()
            .library()
            .map(|l| l.plan(&workout_id.to_string()))
        {
            Some(Ok(Some(plan))) => plan,
            Some(Err(e)) => return self.fail(e),
            _ => return self.fail("That workout can't be ridden.".into()),
        };
        let trainer = link.bind_mut().link.as_mut().and_then(Link::take_trainer);
        let Some(trainer) = trainer else {
            return self.fail("No trainer is connected.".into());
        };
        match Ride::start(plan, rider_profile(ftp_watts), trainer, ticks(now_usec)) {
            Ok(ride) => {
                self.ride = Some(ride);
                true
            }
            Err(e) => self.fail(e),
        }
    }

    fn fail(&mut self, message: String) -> bool {
        godot_warn!("{message}");
        self.error = message;
        false
    }

    /// Advances to `now` and returns the newest HUD frame (empty before the first).
    #[func]
    fn advance(&mut self, now_usec: i64) -> VarDictionary {
        let Some(ride) = self.ride.as_mut() else {
            return VarDictionary::new();
        };
        let frame = ride.advance(ticks(now_usec)).cloned();
        frame.map_or_else(VarDictionary::new, |f| frame_dict(&f, ride.is_complete()))
    }

    /// The route being ridden.
    #[func]
    fn route(&self) -> VarDictionary {
        self.ride
            .as_ref()
            .map_or_else(VarDictionary::new, |r| route_dict(r.route()))
    }

    /// Whether the power slider applies (simulator only).
    #[func]
    fn has_manual_power(&mut self) -> bool {
        self.ride.as_mut().is_some_and(Ride::has_manual_power)
    }

    /// Sets the simulator's power.
    #[func]
    fn set_manual_power(&mut self, watts: f64) {
        if let Some(ride) = self.ride.as_mut() {
            ride.set_manual_power(watts);
        }
    }

    /// The simulator's internals, or empty on a real trainer.
    #[func]
    fn sim_debug(&mut self) -> VarDictionary {
        let Some(s) = self.ride.as_mut().and_then(Ride::sim_debug) else {
            return VarDictionary::new();
        };
        dict(vec![
            ("rider_power_w", (s.rider_power_w).to_variant()),
            ("speed_mps", (s.speed_mps).to_variant()),
            ("total_distance_m", (s.total_distance_m).to_variant()),
            ("elapsed_ticks", (s.elapsed_ticks as i64).to_variant()),
            ("running", (s.running).to_variant()),
            ("grade", (s.params.grade_fraction).to_variant()),
            ("headwind_ms", (s.params.headwind_ms).to_variant()),
            ("crr", (s.params.crr).to_variant()),
            ("cda_m2", (s.params.cda_m2).to_variant()),
        ])
    }

    /// Messages for the rider since the last call.
    #[func]
    fn take_messages(&mut self) -> PackedStringArray {
        let mut messages = self
            .ride
            .as_mut()
            .map(Ride::take_messages)
            .unwrap_or_default();
        if !self.error.is_empty() {
            messages.push(std::mem::take(&mut self.error));
        }
        strings(messages)
    }

    /// Ends the ride and releases the trainer.
    #[func]
    fn finish(&mut self) {
        self.ride = None;
    }
}

/// The race view's live kernel.
#[derive(GodotClass)]
#[class(base = RefCounted)]
pub struct RaceController {
    race: LiveRace,
}

#[godot_api]
impl IRefCounted for RaceController {
    fn init(_base: Base<RefCounted>) -> Self {
        Self {
            race: LiveRace::new(),
        }
    }
}

#[godot_api]
impl RaceController {
    /// Back to the start.
    #[func]
    fn reset(&mut self) {
        self.race = LiveRace::new();
    }

    /// Adds real time.
    #[func]
    fn advance(&mut self, delta_s: f64) {
        self.race.advance(delta_s);
    }

    /// `{time_s, finished, riders: [{id, distance_m, speed_m_s, slot, draft_target}], events}`;
    /// `events` is the latest feed lines, oldest first.
    #[func]
    fn state(&self) -> VarDictionary {
        let mut riders = VarArray::new();
        for r in self.race.riders() {
            riders.push(
                &dict(vec![
                    ("id", (gs(&r.id)).to_variant()),
                    ("distance_m", (r.distance_m).to_variant()),
                    ("speed_m_s", (r.speed_ms).to_variant()),
                    ("slot", (r.slot as i64).to_variant()),
                    (
                        "draft_target",
                        (opt_str(r.draft_target.as_deref())).to_variant(),
                    ),
                ])
                .to_variant(),
            );
        }
        dict(vec![
            ("time_s", (self.race.time_s()).to_variant()),
            ("finished", (self.race.is_finished()).to_variant()),
            ("riders", (riders).to_variant()),
            (
                "events",
                (self
                    .race
                    .event_feed()
                    .iter()
                    .map(|l| gs(l))
                    .collect::<PackedStringArray>())
                .to_variant(),
            ),
        ])
    }

    /// `{player_id, slot_count, length_m, elevation_step_m, elevations}`.
    #[func]
    fn course(&self) -> VarDictionary {
        dict(vec![
            ("player_id", (gs(PLAYER_ID)).to_variant()),
            ("slot_count", (self.race.slot_count() as i64).to_variant()),
            ("length_m", (self.race.length_m()).to_variant()),
            ("elevation_step_m", (ELEVATION_STEP_M).to_variant()),
            (
                "elevations",
                (self
                    .race
                    .elevations()
                    .into_iter()
                    .collect::<PackedFloat64Array>())
                .to_variant(),
            ),
        ])
    }
}

/// Software-only mixed workout: no trainer connection or activity writer.
#[derive(GodotClass)]
#[class(base = RefCounted)]
pub struct MixedRaceController {
    race: crate::app::mixed::MixedRace,
}
#[godot_api]
impl IRefCounted for MixedRaceController {
    fn init(_base: Base<RefCounted>) -> Self {
        Self {
            race: crate::app::mixed::MixedRace::new(),
        }
    }
}
#[godot_api]
impl MixedRaceController {
    /// Restart all dynamic continuation and clear the audit.
    #[func]
    fn reset(&mut self) {
        self.race.reset();
    }
    /// Execute every fixed step at the selected playback rate.
    #[func]
    fn advance(&mut self, seconds: f64, rate: f64) {
        self.race.advance(seconds, rate);
    }
    /// Software rider input, never a trainer target.
    #[func]
    fn set_manual_power(&mut self, watts: f64) -> bool {
        self.race.set_power(Some(watts))
    }
    /// Resume nominal prescribed software input.
    #[func]
    fn use_reference(&mut self) {
        self.race.set_power(None);
    }
    /// Request a lateral slot.
    #[func]
    fn set_slot(&mut self, slot: i64) -> bool {
        self.race.set_slot(slot)
    }
    /// Request a known wheel, or empty for automatic selection.
    #[func]
    fn set_wheel(&mut self, name: GString) -> bool {
        self.race.set_wheel(&name.to_string())
    }
    /// Export only simulated data.
    #[func]
    fn export_trace(&mut self, path: GString) -> bool {
        self.race.export_trace(&path.to_string())
    }
    /// Validate and replay the saved control stream through the same session/kernel.
    #[func]
    fn load_trace(&mut self, path: GString) -> bool {
        self.race.load_trace(&path.to_string())
    }
    /// Course samples and public rider names for rendering.
    #[func]
    fn course(&self) -> VarDictionary {
        let k = session_director::mixed::knowledge();
        let names = dict(
            k.riders()
                .iter()
                .map(|r| (r.id(), gs(r.name()).to_variant()))
                .collect(),
        );
        dict(vec![
            ("names", names.to_variant()),
            ("elevation_step_m", ELEVATION_STEP_M.to_variant()),
            (
                "elevations",
                self.race
                    .elevations()
                    .into_iter()
                    .collect::<PackedFloat64Array>()
                    .to_variant(),
            ),
        ])
    }
    /// Prepared text and actual kernel rider positions.
    #[func]
    fn state(&self) -> VarDictionary {
        let view = self.race.view();
        let mut riders = VarArray::new();
        for r in view.riders {
            riders.push(
                &dict(vec![
                    ("id", gs(&r.id).to_variant()),
                    ("distance_m", r.distance_m.to_variant()),
                    ("speed_m_s", r.speed_ms.to_variant()),
                    ("slot", (r.slot as i64).to_variant()),
                ])
                .to_variant(),
            );
        }
        dict(vec![
            ("context", gs(&view.context).to_variant()),
            ("clock", gs(&view.clock).to_variant()),
            ("diagnostics", gs(&view.diagnostics).to_variant()),
            ("message", gs(&view.message).to_variant()),
            ("complete", view.complete.to_variant()),
            ("riders", riders.to_variant()),
        ])
    }
}
