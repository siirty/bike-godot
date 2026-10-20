// Records golden vectors from the Dart domain packages for the Rust port in siirty/bike-godot.
//
// ```sh
// dart --enable-asserts run tool/export_golden_vectors.dart <output-dir>   # from packages/race_engine
// ```
//
// Every value written here is an *output of the Dart reference implementation*, never a hand-typed
// expectation. The Rust crates load these files and must reproduce them: bit-exactly for every
// double where both runtimes use the same IEEE-754 operations and libm, and byte-for-byte for the
// snapshot documents. Inputs are fixed literals or fixture files so a re-run produces identical
// files.
//
// Doubles are written by `jsonEncode`, whose output round-trips exactly; non-finite values cannot
// be represented in JSON and never appear in a vector.
// ignore_for_file: avoid_print
library;

import 'dart:convert';
import 'dart:io';
import 'dart:typed_data';

import 'package:race_engine/race_engine.dart';
import 'package:race_physics/race_physics.dart';
import 'package:race_physics/testing.dart';
import 'package:session_director/session_director.dart';
import 'package:workout_import/workout_import.dart';

late Directory _out;
late Directory _workoutFixtures;

void main(List<String> args) {
  if (args.length != 1) {
    stderr.writeln(
      'Usage: dart run tool/export_golden_vectors.dart <output-dir>',
    );
    exitCode = 64;
    return;
  }
  _out = Directory(args.single)..createSync(recursive: true);
  _workoutFixtures = Directory('../workout_import/test/fixtures');

  _write('workout_import.json', _workoutImportVectors());
  _write('race_physics.json', _racePhysicsVectors());
  _write('session_director.json', _sessionDirectorVectors());
  _write('race_engine.json', _raceEngineVectors());
}

void _write(String name, Object json) {
  final file = File('${_out.path}/$name');
  file.writeAsStringSync(
    '${const JsonEncoder.withIndent(' ').convert(json)}\n',
  );
  print('wrote ${file.path}');
}

Map<String, Object?> _error(Object e) => {
  'error': switch (e) {
    RacePhysicsSolverException() => 'solver',
    ArgumentError() => 'argument',
    WorkoutImportFormatException() => 'format',
    SimulationValidationException(:final code) => 'validation:${code.name}',
    SnapshotDecodeException(:final code) => 'decode:${code.name}',
    StateError() => 'state',
    _ => 'other:${e.runtimeType}',
  },
};

// -----------------------------------------------------------------------------------------------
// workout_import
// -----------------------------------------------------------------------------------------------

const _workoutFiles = [
  'IRONMAN_70_3_Power_Ups.zwo',
  'IRONMAN_70_3_Power_Ups.erg',
  'IRONMAN_70_3_Power_Ups.mrc',
  'Ramp_Z2_to_Z4.zwo',
  'Z5_spike_15s.zwo',
  'Zone_ramp_Z1-Z5.zwo',
];

/// Extra in-memory files that exercise grammar corners the fixtures do not.
const _inlineWorkouts = <String, String>{
  'intervals.zwo': '''
<workout_file><name>  Intervals  </name><author></author><description>d</description><workout>
  <Warmup Duration="300" PowerLow="0.4" PowerHigh="0.75" Cadence="85"/>
  <IntervalsT Repeat="4" OnDuration="60" OffDuration="30" OnPower="1.2" OffPower="0.5" Cadence="100" CadenceResting="80"/>
  <IntervalsT OnDuration="20" PowerOnLow="1.0" PowerOnHigh="1.3"/>
  <IntervalsT Repeat="3" OnDuration="45.5" OffDuration="15.25" PowerOnLow="0.9" PowerOnHigh="1.1" PowerOffLow="0.6" PowerOffHigh="0.4"/>
  <SteadyState Duration="120.5" Power="0.88"/>
  <FreeRide Duration="90" Cadence="90"/>
  <Freeride Duration="30"/>
  <Ramp Duration="200" PowerLow="0.5"/>
  <Cooldown Duration="240" PowerLow="0.3" PowerHigh="0.7"/>
</workout></workout_file>''',
  'crlf.erg':
      '[COURSE HEADER]\r\nVERSION = 2\r\nUNITS = ENGLISH\r\n'
      'DESCRIPTION = CRLF test with tabs\r\nMINUTES WATTS\r\n[END COURSE HEADER]\r\n'
      '[COURSE DATA]\r\n0.00\t100\r\n2.5\t100\r\n2.5 250\r\n4.25    250\r\n'
      '4.25\t120\r\n10\t180\r\n[END COURSE DATA]\r\n',
  'tabdesc.mrc':
      '[COURSE HEADER]\nDESCRIPTION\tTab separated\n[END COURSE HEADER]\n'
      '[COURSE DATA]\n0 50\n1.5 75\n3 105.5\n[END COURSE DATA]\n',
  'bad_root.zwo': '<foo/>',
  'missing_name.zwo': '<workout_file><workout/></workout_file>',
  'bad_number.zwo':
      '<workout_file><name>x</name><workout><SteadyState Duration="abc" Power="1"/></workout></workout_file>',
  'unknown_element.zwo':
      '<workout_file><name>x</name><workout><Teleport Duration="1"/></workout></workout_file>',
  'no_power.zwo':
      '<workout_file><name>x</name><workout><SteadyState Duration="10"/></workout></workout_file>',
  'no_data.erg': '[COURSE HEADER]\nDESCRIPTION = x\n[END COURSE HEADER]\n',
  'one_point.erg': '[COURSE DATA]\n0 100\n[END COURSE DATA]\n',
  'decreasing.mrc': '[COURSE DATA]\n0 50\n2 60\n1 70\n[END COURSE DATA]\n',
  'three_tokens.erg': '[COURSE DATA]\n0 100 5\n1 100\n[END COURSE DATA]\n',
  'unsupported.fit': 'whatever',
};

Map<String, Object?> _planJson(WorkoutPlan plan) => {
  'name': plan.name,
  'author': plan.author,
  'description': plan.description,
  'totalDurationMicros': plan.totalDuration.inMicroseconds,
  'segmentsJson': jsonDecode(WorkoutSegmentsCodec.encode(plan.segments)),
};

List<Map<String, Object?>> _timelineSamples(WorkoutPlan plan) {
  final timeline = WorkoutTimeline(plan);
  final total = timeline.totalDuration.inMicroseconds;
  final times = <int>{0, total};
  // Every segment boundary, one microsecond either side of it, and a regular grid.
  var elapsed = 0;
  for (final segment in plan.segments) {
    elapsed += segment.duration.inMicroseconds;
    for (final t in [elapsed - 1, elapsed, elapsed + 1]) {
      if (t >= 0 && t <= total) times.add(t);
    }
  }
  for (var t = 0; t <= total; t += 7300000) {
    times.add(t);
  }
  final sorted = times.toList()..sort();
  return [
    for (final t in sorted)
      {
        't': t,
        'index': timeline.indexAt(Duration(microseconds: t)),
        'remaining': timeline
            .timeRemainingAt(Duration(microseconds: t))
            .inMicroseconds,
        'elapsedInSegment': timeline
            .timeElapsedInSegmentAt(Duration(microseconds: t))
            .inMicroseconds,
        'power250': timeline.powerAt(Duration(microseconds: t), ftpWatts: 250),
        'power312': timeline.powerAt(
          Duration(microseconds: t),
          ftpWatts: 312.5,
        ),
      },
  ];
}

Map<String, Object?> _parseVector(String fileName, List<int> bytes) {
  try {
    final imported = WorkoutFormats.parseFile(
      fileName: fileName,
      bytes: Uint8List.fromList(bytes),
    );
    return {
      'fileName': fileName,
      'format': imported.formatTag,
      'plan': _planJson(imported.plan),
      'timeline': imported.plan.segments.isEmpty
          ? null
          : _timelineSamples(imported.plan),
    };
  } on WorkoutImportFormatException catch (e) {
    return {
      'fileName': fileName,
      ..._error(e),
      'message': e.message,
      'errorFormat': e.format,
    };
  }
}

Object _workoutImportVectors() {
  final parses = <Map<String, Object?>>[
    for (final name in _workoutFiles)
      _parseVector(
        name,
        File('${_workoutFixtures.path}/$name').readAsBytesSync(),
      ),
    for (final entry in _inlineWorkouts.entries)
      _parseVector(entry.key, utf8.encode(entry.value)),
  ];

  final legacy = <Map<String, Object?>>[
    for (final format in ['zwo', 'erg', 'mrc'])
      () {
        final source = File(
          '${_workoutFixtures.path}/legacy_segments_json/'
          'IRONMAN_70_3_Power_Ups.$format.segments.json',
        ).readAsStringSync();
        final decoded = WorkoutSegmentsCodec.decode(source);
        return {
          'file': 'IRONMAN_70_3_Power_Ups.$format.segments.json',
          'readable': decoded is ReadableWorkoutSegments,
          'version': switch (decoded) {
            ReadableWorkoutSegments(:final version) => version,
            UnreadableWorkoutSegments(:final version) => version,
          },
          'reencoded': decoded is ReadableWorkoutSegments
              ? jsonDecode(WorkoutSegmentsCodec.encode(decoded.segments))
              : null,
        };
      }(),
  ];

  final decodeCases = [
    '',
    'null',
    '42',
    '"a string"',
    '{}',
    '[]',
    '[{"type":null}]',
    '{"version":"one","segments":[]}',
    '{"version":1,"segments":"not a list"}',
    '{"version":1,"segments":[7]}',
    '{"version":9999,"segments":[]}',
    '{"version":1,"segments":[{"type":"teleport"}]}',
    '{"version":1,"segments":[{"type":"freeRide","durationMs":60000,"cadence":null}]}',
    '{"version":0,"segments":[{"type":"steadyState","durationMs":1000,"targetPower":{"type":"absoluteWatts","watts":200}}]}',
    '{"version":1,"segments":[{"type":"intervalRepeat","repeat":2,"onDurationMs":60000,'
        '"onStartPower":{"type":"percentFtp","fraction":1.0},"onEndPower":{"type":"percentFtp","fraction":1.0},'
        '"offDurationMs":60000,"offStartPower":null,"offEndPower":null,"onCadence":null,"offCadence":null}]}',
  ];
  final decodes = [
    for (final source in decodeCases)
      switch (WorkoutSegmentsCodec.decode(source)) {
        ReadableWorkoutSegments(:final segments, :final version) => {
          'source': source,
          'readable': true,
          'version': version,
          'reencoded': jsonDecode(WorkoutSegmentsCodec.encode(segments)),
        },
        UnreadableWorkoutSegments(:final kind, :final version) => {
          'source': source,
          'readable': false,
          'kind': kind.name,
          'version': version,
        },
      },
  ];

  return {'parses': parses, 'legacy': legacy, 'decodes': decodes};
}

// -----------------------------------------------------------------------------------------------
// race_physics
// -----------------------------------------------------------------------------------------------

const _riders = [
  RiderPhysicalParams(totalMassKg: 78),
  RiderPhysicalParams(totalMassKg: 62.5, crr: 0.0041, cdaM2: 0.245),
  RiderPhysicalParams(
    totalMassKg: 95,
    crr: 0.0068,
    cdaM2: 0.38,
    driveTrainEfficiency: 0.96,
  ),
];

const _grades = [-0.12, -0.08, -0.03, -0.0001, 0.0, 0.002, 0.045, 0.08, 0.15];
const _speeds = [0.0, 0.3, 1.5, 4.2, 8.333, 12.5, 17.9, 29.9];
const _winds = [-14.0, -3.5, 0.0, 2.25, 8.0];

Map<String, Object?> _riderJson(RiderPhysicalParams r) => r.toJson();

Object _racePhysicsVectors() {
  final forces = <Map<String, Object?>>[];
  for (final mass in [50.0, 78.0, 112.4]) {
    for (final grade in _grades) {
      forces.add({
        'massKg': mass,
        'grade': grade,
        'gravityN': gravityForceN(massKg: mass, gradeFraction: grade),
        'rollingN': rollingResistanceForceN(
          massKg: mass,
          gradeFraction: grade,
          crr: 0.0053,
        ),
      });
    }
  }
  final aero = <Map<String, Object?>>[];
  for (final speed in _speeds) {
    for (final wind in _winds) {
      aero.add({
        'speedMs': speed,
        'headwindMs': wind,
        'rho': 1.18,
        'cda': 0.29,
        'aeroN': aeroDragForceN(
          airDensityKgM3: 1.18,
          cdaM2: 0.29,
          speedMs: speed,
          headwindMs: wind,
        ),
      });
    }
  }

  final power = <Map<String, Object?>>[];
  final speedSolve = <Map<String, Object?>>[];
  final gradeSolve = <Map<String, Object?>>[];
  for (final rider in _riders) {
    for (final grade in _grades) {
      for (final wind in [0.0, 4.0, -6.0]) {
        final env = EnvironmentParams(gradeFraction: grade, headwindMs: wind);
        for (final speed in _speeds) {
          power.add({
            'rider': _riderJson(rider),
            'grade': grade,
            'headwindMs': wind,
            'speedMs': speed,
            'powerW': powerForSpeed(speedMs: speed, rider: rider, env: env),
          });
        }
        for (final watts in [0.0, 35.0, 150.0, 250.0, 412.5, 900.0, 4000.0]) {
          Map<String, Object?> result;
          try {
            result = {
              'speedMs': solveSpeedForPower(
                powerWatts: watts,
                rider: rider,
                env: env,
              ),
            };
          } on Object catch (e) {
            result = _error(e);
          }
          speedSolve.add({
            'rider': _riderJson(rider),
            'grade': grade,
            'headwindMs': wind,
            'powerW': watts,
            ...result,
          });
        }
      }
    }
    for (final speed in [0.0, 2.5, 5.0, 9.7, 14.0]) {
      for (final watts in [0.0, 80.0, 220.0, 380.0, 1500.0]) {
        Map<String, Object?> result;
        try {
          result = {
            'grade': solveGradeForPower(
              powerWatts: watts,
              speedMs: speed,
              rider: rider,
              env: const EnvironmentParams(),
            ),
          };
        } on Object catch (e) {
          result = _error(e);
        }
        gradeSolve.add({
          'rider': _riderJson(rider),
          'speedMs': speed,
          'powerW': watts,
          ...result,
        });
      }
    }
  }

  final steps = <Map<String, Object?>>[];
  for (final rider in _riders) {
    for (final speed in [0.0, 0.05, 2.0, 9.5, 21.0]) {
      for (final watts in [0.0, 120.0, 320.0, 1450.0]) {
        for (final grade in [-0.07, 0.0, 0.06]) {
          for (final (braking, drag, wind, stepMicros, cap) in [
            (0.0, 1.0, 0.0, 50000, 500.0),
            (35.0, 0.72, -9.5, 50000, 500.0),
            (400.0, 0.5, 3.0, 200000, 275.0),
            (0.0, 0.9, 0.0, 2000, 800.0),
          ]) {
            final env = EnvironmentParams(
              gradeFraction: grade,
              headwindMs: wind,
            );
            final r = stepLongitudinalDynamics(
              speedMs: speed,
              powerWatts: watts,
              rider: rider,
              environment: env,
              timestep: Duration(microseconds: stepMicros),
              config: LongitudinalDynamicsConfig(maxPropulsiveForceN: cap),
              brakingForceN: braking,
              aerodynamicDragMultiplier: drag,
            );
            steps.add({
              'rider': _riderJson(rider),
              'speedMs': speed,
              'powerW': watts,
              'grade': grade,
              'headwindMs': wind,
              'brakingN': braking,
              'dragMultiplier': drag,
              'stepMicros': stepMicros,
              'maxPropulsiveForceN': cap,
              'nextSpeedMs': r.nextSpeedMs,
              'distanceDeltaM': r.distanceDeltaM,
              'propulsiveForceN': r.propulsiveForceN,
              'gravityForceN': r.gravityForceN,
              'rollingResistanceForceN': r.rollingResistanceForceN,
              'rawAeroDragForceN': r.rawAeroDragForceN,
              'appliedAeroDragForceN': r.appliedAeroDragForceN,
              'netForceN': r.netForceN,
              'rawAccelerationMs2': r.rawAccelerationMs2,
              'zeroSpeedClampApplied': r.zeroSpeedClampApplied,
            });
          }
        }
      }
    }
  }

  final config = LongitudinalDynamicsConfig();
  final trajectories = <Map<String, Object?>>[];
  for (final scenario in convergenceCorpus) {
    for (final track in scenario.tracks) {
      final t = runTrack(
        track,
        step: SimulationTimestep.recommended,
        config: config,
      );
      // One sample per simulated second keeps the file small while still pinning the trajectory.
      final perSecond = 1000000 ~/ t.sampleIntervalMicros;
      trajectories.add({
        'scenario': scenario.name,
        'track': track.name,
        'sampleCount': t.timesMicros.length,
        'samples': [
          for (var i = 0; i < t.timesMicros.length; i += perSecond)
            [t.timesMicros[i], t.speedMs[i], t.distanceM[i]],
        ],
        'finalSpeedMs': t.finalSpeedMs,
        'finalDistanceM': t.finalDistanceM,
        'stoppedAtMicros': t.stoppedAt?.inMicroseconds,
        'stoppedAtDistanceM': t.stoppedAtDistanceM,
      });
    }
  }

  return {
    'forces': forces,
    'aero': aero,
    'powerForSpeed': power,
    'solveSpeedForPower': speedSolve,
    'solveGradeForPower': gradeSolve,
    'steps': steps,
    'trajectories50ms': trajectories,
    'convergenceReport': renderConvergenceReport(),
  };
}

// -----------------------------------------------------------------------------------------------
// session_director
// -----------------------------------------------------------------------------------------------

Object _sessionDirectorVectors() {
  const profiles = [
    RiderProfile(ftpWatts: 250, physical: RiderPhysicalParams(totalMassKg: 78)),
    RiderProfile(
      ftpWatts: 315,
      physical: RiderPhysicalParams(totalMassKg: 68, cdaM2: 0.27),
    ),
  ];
  final syntheses = <Map<String, Object?>>[];
  final plans = <String, WorkoutPlan>{
    for (final name in _workoutFiles)
      name: WorkoutFormats.parseFile(
        fileName: name,
        bytes: File('${_workoutFixtures.path}/$name').readAsBytesSync(),
      ).plan,
    'intervals.zwo': WorkoutFormats.parseFile(
      fileName: 'intervals.zwo',
      bytes: utf8.encode(_inlineWorkouts['intervals.zwo']!),
    ).plan,
  };
  for (final entry in plans.entries) {
    for (var p = 0; p < profiles.length; p++) {
      final terrain = GradeForPowerSynthesis().synthesize(
        plan: entry.value,
        rider: profiles[p],
        seed: 7,
      );
      final probes = <List<double>>[];
      final length = terrain.lengthM ?? 0;
      for (var i = -1; i <= 400; i++) {
        final d = length * i / 397;
        probes.add([d, terrain.gradeAtDistance(d)]);
      }
      syntheses.add({
        'workout': entry.key,
        'profile': p,
        'samples': [
          for (final s in terrain.samples) [s.distanceM, s.gradeFraction],
        ],
        'ranges': [
          for (final r in terrain.segmentRanges)
            [r.startDistanceM, r.endDistanceM],
        ],
        'lengthM': terrain.lengthM,
        'probes': probes,
      });
    }
  }
  final efforts = [
    for (var i = -10; i <= 200; i++)
      {
        'effort': i / 100,
        'grade': GradeForPowerSynthesis.gradeForEffort(i / 100),
        'zone': zoneForEffort(i / 100).name,
      },
  ];
  return {'syntheses': syntheses, 'gradeForEffort': efforts};
}

// -----------------------------------------------------------------------------------------------
// race_engine
// -----------------------------------------------------------------------------------------------

RiderId _id(String s) => RiderId(s);

Object _raceEngineVectors() {
  // PCG32: raw sequences, doubles and bounded draws.
  final pcg = <Map<String, Object?>>[];
  for (final (seed, stream) in [
    (42, 54),
    (0, 0),
    (20260812, 1),
    (99, 3),
    (-1, 7),
    (0x7FFFFFFFFFFFFFFF, -2),
  ]) {
    final a = Pcg32.seeded(seed: seed, stream: stream);
    final start = a.state;
    final u32 = [for (var i = 0; i < 12; i++) a.nextUint32()];
    final doubles = [for (var i = 0; i < 6; i++) a.nextDouble()];
    final bounded = [
      for (final bound in [1, 2, 3, 7, 100, 1000003, 0x100000000])
        a.nextIntBelow(bound),
    ];
    pcg.add({
      'seed': seed.toString(),
      'stream': stream.toString(),
      'startState': hex64(start.state),
      'startStream': hex64(start.stream),
      'u32': u32,
      'doubles': doubles,
      'bounded': bounded,
      'endState': hex64(a.state.state),
    });
  }

  // Terrain lookups.
  final terrainCases = <Map<String, Object?>>[];
  for (final interpolation in TerrainInterpolation.values) {
    for (final beyondEnd in BeyondEndBehavior.values) {
      for (final lengthM in [null, 2500.0]) {
        final terrain = SimulationTerrain(
          samples: [
            TerrainSample(distanceM: 0, gradeFraction: 0.01),
            TerrainSample(distanceM: 400, gradeFraction: 0.065),
            TerrainSample(distanceM: 1150.5, gradeFraction: -0.04),
            TerrainSample(distanceM: 2000, gradeFraction: 0.02),
          ],
          interpolation: interpolation,
          beyondEnd: beyondEnd,
          lengthM: lengthM,
        );
        terrainCases.add({
          'interpolation': interpolation.name,
          'beyondEnd': beyondEnd.name,
          'lengthM': lengthM,
          'probes': [
            for (final d in [
              -5.0,
              0.0,
              1.0,
              399.999,
              400.0,
              777.7,
              1150.5,
              1999.0,
              2000.0,
              2200.0,
              2500.0,
              2500.001,
              9000.0,
            ])
              [d, terrain.gradeAtDistance(d)],
          ],
        });
      }
    }
  }

  final races = <Map<String, Object?>>[];
  for (final scenario in _raceScenarios()) {
    final sim = RaceSimulation(config: scenario.config);
    var state = scenario.initial;
    final checkpoints = <Map<String, Object?>>[];
    for (var step = 0; step < scenario.steps; step++) {
      state = sim
          .step(state, scenario.inputs(step), scenario.config.fixedStep)
          .nextState;
      if ((step + 1) % scenario.every == 0 || step == scenario.steps - 1) {
        checkpoints.add({
          'step': step + 1,
          'riders': [
            for (final r in state.riders)
              {
                'id': r.id.value,
                'distanceM': r.distanceM,
                'speedMs': r.speedMs,
                'lateralSlot': r.lateralSlot,
                'draftTargetId': r.draftTargetId?.value,
              },
          ],
        });
      }
    }
    races.add({
      'name': scenario.name,
      'initialSnapshot': SimulationSnapshotCodec.encode(
        scenario.config,
        scenario.initial,
      ),
      'steps': scenario.steps,
      'inputScript': scenario.script,
      'checkpoints': checkpoints,
      'finalSnapshot': SimulationSnapshotCodec.encode(scenario.config, state),
    });
  }

  final decodeErrors = <Map<String, Object?>>[];
  final golden = races.first['initialSnapshot']! as String;
  for (final (label, source) in [
    ('not json', '{'),
    ('not object', '[]'),
    ('wrong format', golden.replaceFirst('bike-training-race-snapshot', 'x')),
    ('v1', golden.replaceFirst('"schemaVersion":2', '"schemaVersion":1')),
    (
      'missing config',
      '{"format":"bike-training-race-snapshot","schemaVersion":2}',
    ),
    (
      'bad interpolation',
      golden.replaceFirst(
        '"interpolation":"linear"',
        '"interpolation":"cubic"',
      ),
    ),
    ('negative speed', golden.replaceFirst('"speedMs":8.0', '"speedMs":-8.0')),
    (
      'upper hex',
      golden.replaceFirst(RegExp(r'"state":"[0-9a-f]{4}'), '"state":"ABCD'),
    ),
    (
      'slot past count',
      golden.replaceFirst('"lateralSlot":1', '"lateralSlot":9'),
    ),
  ]) {
    try {
      SimulationSnapshotCodec.decode(source);
      decodeErrors.add({'label': label, 'source': source, 'error': null});
    } on RaceSimulationException catch (e) {
      decodeErrors.add({
        'label': label,
        'source': source,
        ..._error(e),
        'path': e.path,
        'message': e.message,
      });
    }
  }

  return {
    'pcg32': pcg,
    'terrain': terrainCases,
    'races': races,
    'decodeErrors': decodeErrors,
  };
}

final class _RaceScenario {
  _RaceScenario({
    required this.name,
    required this.config,
    required this.initial,
    required this.steps,
    required this.every,
    required this.script,
  });

  final String name;
  final RaceSimulationConfig config;
  final RaceState initial;
  final int steps;
  final int every;

  /// Per rider, a list of `[fromStep, powerW, brakingN, slot|null, target|null]` changes.
  final Map<String, List<List<Object?>>> script;

  RaceInputFrame inputs(int step) => RaceInputFrame(
    inputs: [
      for (final entry in script.entries)
        () {
          List<Object?> current = entry.value.first;
          for (final change in entry.value) {
            if ((change[0]! as int) <= step) current = change;
          }
          return RiderControlInput(
            id: _id(entry.key),
            powerWatts: (current[1]! as num).toDouble(),
            brakingForceN: (current[2]! as num).toDouble(),
            lateralSlot: current[3] as int?,
            draftTargetId: current[4] == null
                ? null
                : _id(current[4]! as String),
          );
        }(),
    ],
  );
}

List<_RaceScenario> _raceScenarios() {
  final drafting = BasicDraftingConfig(
    maxGapM: 6,
    minimumDragMultiplierBySlotOffset: [0.62, 0.8, 0.95],
  );
  final rolling = SimulationTerrain(
    samples: [
      TerrainSample(distanceM: 0, gradeFraction: 0.005),
      TerrainSample(distanceM: 180, gradeFraction: 0.025),
      TerrainSample(distanceM: 430, gradeFraction: -0.01),
      TerrainSample(distanceM: 760, gradeFraction: 0.03),
      TerrainSample(distanceM: 1120, gradeFraction: -0.015),
    ],
    interpolation: TerrainInterpolation.linear,
    lengthM: 2000,
  );
  return [
    _RaceScenario(
      name: 'replay-three-riders-no-drafting',
      config: RaceSimulationConfig(
        fixedStep: SimulationTimestep.recommended,
        riders: [
          RiderDefinition(
            id: _id('human'),
            physical: const RiderPhysicalParams(totalMassKg: 78),
          ),
          RiderDefinition(
            id: _id('rival'),
            physical: const RiderPhysicalParams(totalMassKg: 77),
          ),
          RiderDefinition(
            id: _id('leader'),
            physical: const RiderPhysicalParams(totalMassKg: 75),
          ),
        ],
        terrain: rolling,
        dynamics: LongitudinalDynamicsConfig(),
        lateralSlotCount: 3,
      ),
      initial: RaceState(
        simulationTime: Duration.zero,
        randomState: Pcg32.seeded(seed: 20260926, stream: 1).state,
        riders: [
          RiderState(
            id: _id('human'),
            distanceM: 30,
            speedMs: 8,
            lateralSlot: 1,
          ),
          RiderState(
            id: _id('rival'),
            distanceM: 35,
            speedMs: 8,
            lateralSlot: 2,
          ),
          RiderState(id: _id('leader'), distanceM: 42, speedMs: 8),
        ],
      ),
      steps: 1800,
      every: 100,
      script: {
        'human': [
          [0, 250, 0, null, null],
          [650, 330, 0, null, null],
        ],
        'leader': [
          [0, 265, 0, null, null],
        ],
        'rival': [
          [0, 275, 0, null, null],
          [900, 245, 0, null, null],
        ],
      },
    ),
    _RaceScenario(
      name: 'pack-with-drafting',
      config: RaceSimulationConfig(
        fixedStep: SimulationTimestep.recommended,
        riders: [
          for (final (id, mass, cda) in [
            ('a1', 72.0, 0.30),
            ('b2', 81.0, 0.34),
            ('c3', 66.0, 0.27),
            ('d4', 90.0, 0.36),
            ('e5', 75.5, 0.31),
          ])
            RiderDefinition(
              id: _id(id),
              physical: RiderPhysicalParams(totalMassKg: mass, cdaM2: cda),
            ),
        ],
        terrain: rolling,
        dynamics: LongitudinalDynamicsConfig(),
        drafting: drafting,
        lateralSlotCount: 3,
        headwindMs: 1.5,
      ),
      initial: RaceState(
        simulationTime: Duration.zero,
        randomState: Pcg32.seeded(seed: 5, stream: 11).state,
        riders: [
          RiderState(id: _id('a1'), distanceM: 20, speedMs: 9),
          RiderState(id: _id('b2'), distanceM: 17.5, speedMs: 9),
          RiderState(id: _id('c3'), distanceM: 15, speedMs: 9, lateralSlot: 1),
          RiderState(
            id: _id('d4'),
            distanceM: 12,
            speedMs: 9,
            draftTargetId: _id('b2'),
          ),
          RiderState(id: _id('e5'), distanceM: 9.5, speedMs: 9, lateralSlot: 2),
        ],
      ),
      steps: 2400,
      every: 60,
      script: {
        'a1': [
          [0, 280, 0, null, null],
          [700, 180, 0, null, null],
          [1500, 340, 0, 1, null],
        ],
        'b2': [
          [0, 230, 0, null, 'a1'],
          [1200, 400, 0, 2, null],
          [1300, 260, 0, null, 'a1'],
        ],
        'c3': [
          [0, 210, 0, 0, 'b2'],
          [900, 0, 25, null, null],
          [1000, 300, 0, null, null],
        ],
        'd4': [
          [0, 260, 0, null, null],
          [400, 260, 0, 1, 'c3'],
        ],
        'e5': [
          [0, 250, 0, 0, null],
          [1800, 600, 0, 2, null],
        ],
      },
    ),
    _RaceScenario(
      name: 'standing-start-braking-beyond-end',
      config: RaceSimulationConfig(
        fixedStep: const Duration(milliseconds: 100),
        riders: [
          RiderDefinition(
            id: _id('x'),
            physical: const RiderPhysicalParams(
              totalMassKg: 66.25,
              crr: 0.0062,
              cdaM2: 0.2405,
              driveTrainEfficiency: 0.9685,
            ),
          ),
          RiderDefinition(
            id: _id('y'),
            physical: const RiderPhysicalParams(totalMassKg: 81.5),
          ),
        ],
        terrain: SimulationTerrain(
          samples: [
            TerrainSample(distanceM: 0, gradeFraction: 0.04),
            TerrainSample(distanceM: 150, gradeFraction: -0.06),
          ],
          beyondEnd: BeyondEndBehavior.flat,
          lengthM: 400,
        ),
        dynamics: LongitudinalDynamicsConfig(maxPropulsiveForceN: 420),
        drafting: BasicDraftingConfig(
          maxGapM: 3.5,
          minimumDragMultiplierBySlotOffset: [0.7],
        ),
        airDensityKgM3: 1.1,
        headwindMs: -3,
      ),
      initial: RaceState(
        simulationTime: Duration.zero,
        randomState: Pcg32.seeded(seed: 1, stream: 1).state,
        riders: [
          RiderState(id: _id('x'), distanceM: 0, speedMs: 0),
          RiderState(id: _id('y'), distanceM: 1, speedMs: 0),
        ],
      ),
      steps: 900,
      every: 30,
      script: {
        'x': [
          [0, 900, 0, null, null],
          [100, 300, 0, null, 'y'],
          [500, 0, 300, null, null],
          [560, 200, 0, null, null],
        ],
        'y': [
          [0, 600, 0, null, null],
          [300, 250, 0, null, null],
        ],
      },
    ),
  ];
}
