# ADR 0011: Explicit game session phases and provisional team objective

- Status: accepted for the game prototype
- Date: 2026-10-22
- Supersedes ADR 0010's app phase handling and replay policy; preserves its kernel boundaries

## Decision and scope

Treat the designed race as a game until the world model can be independently verified.
Current acceptance concerns explicit rules, readable decisions, workout alignment and consistent
consequences. Passing software tests is not evidence of elite-race prediction or tactical transfer.
TT-01–TT-10 remain requirements before claiming validated tactical training. The director should
make the intended racing action and workout action agree; a separate workout score does not excuse
a situation in which ignoring the required contribution is equally good for the game objective.

Use one pure GameSession for Godot and headless comparison:

| Workout time | Phase | Authority |
|---|---|---|
| 0–600 s | WarmUp, trainer beside bus | Player physiology only; no active competitive field |
| 600–3300 s | Race | Existing 50 ms kernel and frozen competitive-only opponent plan |
| 3300–3600 s | CoolDown, trainer beside bus | Player physiology only; field/result stay frozen |
| After completion/finish | Finished | No further work or competitive advancement |

The workout and capability values stay unchanged. The field is initialised once at competitive
start with the actual player's preparation reserve. NPCs explicitly start fresh at the authored
grid; their warm-up is outside this game. Grid placement is pre-race initialisation, not a mid-race
rejoin. The competitive plan is fitted from that grid, with no runtime target-error correction.
The legacy MixedSession experiment remains available unchanged in duration through its original
constructor; the app uses the new competitive-only template through GameSession.

## Objective and controls

The director assigns the existing SupportTeam role. The provisional rule is: **at least one Ember
rider, including the player, must be in the leading group at the 45-minute competitive deadline**.
Leading-group membership uses the observer's actual 12 m longitudinal neighbour connections.
This is an explicit game threshold, not an empirical cooperation or race-result model. No finish
distance or winner is fabricated. Capture group members, team representation and player deficit
as immutable outcome evidence at workout time 3300 s.

Pause holds workout/active clocks, reserve, field and policies. It does not simulate real-world
recovery during the wall-time pause. Finish ends immediately. Skip to cooldown explicitly moves
the workout cursor to 3300 s while preserving executed active time; pause remains active if it was
already set. A skipped or early-ended race is Incomplete, never a successful race. A previously
resolved result stays intact when finishing during cooldown.

Workout status is independent: InProgress, Completed (all time executed), Skipped or Abandoned.
Completed does not mean that every target watt was achieved. Samples and adherence remain separate.
The UI exposes the objective and result without revealing coaching/private reserve. It labels the
world model provisional. Primitive bus/stand art is presentation only; no role selector is added.

## Audit and replay

Active time excludes pauses and skipped duration. Workout time is the source cursor and can jump
only on explicit skip. Schema 2 / policy mixed-game-v2 records ordered Input, Pause,
SkipToCooldown and Finish actions on active time, plus samples and lifecycle events. Multiple
actions can share a timestamp. Validate the entire stream before resetting: correct policy/initial
template, bounded aligned times, finite input, identities, ordering, pause/skip/finish semantics and
no advancement after stopping. Old mixed-v1 traces reject explicitly. Replay executes the same
lifecycle from the beginning and recomputes outcomes; stored outcome labels are not trusted.
Playback pause is an external viewing control and does not modify recorded game actions.

No kernel snapshot/event contract changes. The new lifecycle audit is outside RaceState/RaceEvent.
No simulated sample becomes measured activity. Hardware integration and SIM-only trainer controls
are unchanged; this screen still accepts software input only.

## Balance evidence, not an automatic approval

The comparison tool runs the nominal hour; missed, 30 s delayed and 15% excessive contributions
for each hard block; 5% easier endurance; and ignoring all contributions. It reports block power,
shelter, gaps, reserve, frozen outcome and review checkpoints. Unnecessary contributions,
excess-effort wins/losses and sensitivity to easier endurance remain visible findings.
These software input policies are not an independent cue reader or human recognition test.
The reference succeeds, but current easier-endurance and excess-effort failures identify tuning
work before calling this fixture balanced. They must not be hidden with automatic regrouping.
