# Platform rollout

**Linux desktop first.** It is the ride device (see [product-alignment.md](product-alignment.md)),
it needs no Apple hardware or paid developer account, and it matches the development environment.
It is the only target that is built and tested today. CI builds the GDExtension and runs the
headless Godot smoke test on Linux.

Each further platform needs two things:
- the `godot_adapter` library cross-compiled for that target, and listed in
  `apps/rider_godot/bike_core.gdextension`;
- a Godot export preset.

None of these exist yet. In order of reach versus cost:

1. **Windows desktop.** It needs a WinRT BLE spike, because that backend is the least proven; see
   [risks.md](risks.md).
2. **Android.** The `btleplug` hybrid build and the R8 keep rules are the main cost; see
   [risks.md](risks.md).
3. **macOS and iOS.** Xcode needs a real Mac, which is a hard external constraint. With no Mac
   available, these could at most be built in CI on a `macos-latest` runner, with no local device
   testing.
