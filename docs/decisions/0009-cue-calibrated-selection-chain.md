# ADR 0009: Separate motor execution and reject unsuitable situation fits

- Status: accepted for a focused headless experiment; no production generator claimed
- Date: 2026-10-21
- Extends ADR 0007 and ADR 0008; kernel and frozen regression contracts are unchanged

## Decision

Separate the public move reader from a reference motion executor. Motion goals contain only
observable speed, position error and lateral file. The executor produces capacity-limited power
and braking requests using the existing physics and drafting calculation; it never writes a
position, speed or capability. Its gains are synthetic engineering settings, not a human model.
Own capability is available; workout targets and other riders' private physiology are not policy
inputs. Observable lateral file is part of the public projection. A nearby rider in another file
is not automatically a useful wheel. A deliberately selected chase can request that rider's file.

A small selection/recovery/selection chain is the next experiment before expanding all zones.
It starts with a moving group, then runs continuously without a state reset. It has a frozen 1.5%
climb, fixed capabilities and bounded group roles: Morgan leads the selections, Jo takes the front
in the easier section, and Alex makes a bounded movement in another file. These roles must be
visible in actual motion; a role label alone is not evidence. This is a **stay-with-group exercise**,
not proof of a contest-finish threat classifier, tempo rotation lesson or natural full-race tactics.

The fixture compiler fits pace intentions offline against a target-independent cue-following
athlete, using the last 20 seconds of each block and at most eight adjustment passes. It can use
workout knowledge. Runtime local opponent controllers receive only fixed pace/leader/offset roles
and actual motion, without workout, target-error, author answer or fidelity inputs. Neither a local
controller nor the scheduler adjusts pace to repair athlete errors. Production planning stays pure;
this tool orchestrates the existing deterministic kernel and performs its own offline I/O.

## Acceptance and evidence

Freeze the plan, then test continuous control, one-second held controls, and alternating ±3%
requested-power variation every five seconds. Those are declared engineering execution bounds,
not proficiency classes or calibrated human reaction distributions. Report ±5% motor-output
variants, covering another-file movement and missing the second effort separately; they do not
silently change the declared acceptance bounds.

A plan passes the focused engineering gate only when **all** bounded executions meet:

- every step, including the initial group step: ≥80% of delivered samples within ±10% FTP;
- an actually available same-file wheel within 8 m when the second selection starts;
- an actual finish in 323–335 seconds, after the useful duration of the final selection;
- no more than 100 J of physiology clipping for either scheduled pace leader on this course.

These are explicit diagnostic settings. Rejected plans remain exportable as failure evidence;
passing does not establish human learning or empirical race-result validity. No automatic template
fallback, retry beyond the fixed fitting budget, live adaptation or winner selection is introduced.
The first 30-second group step is not excluded to conceal a controller startup failure: the common
initial group state is fitted before riding and its delivered effort is evaluated like every step.

The all-zone report also exposes a cue-fit rejection when its reference fails effort envelopes or
finishes outside the authored sprint. Report version 3 replaces the former >5 W requested-power
rise timing proxy with the onset of one second of actual closing (≥0.2 m/s) or contact (≤8 m) with
the first cued rider. That is physical response evidence, not recognition or proof of intent.

Climbing work, actual clipping and finish events support path review. An adversarial fixture with
unchanged identity/role but lower sustained capacity and finite reserve must lose the ability to
execute the same climbing pace. This checks course/capability consequences without a sprinter veto,
expected ranking or hidden capacity changes. It does not calibrate a 4,000 m stage or model a full
race. Human review/transfer, tempo pull-duty cues, other terrain and general workout synthesis are
still required. See the [verification record](../verification/2026-10-21-selection-chain.md).
