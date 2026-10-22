# Samples

Demo files made with Lumetrope (MIT, regenerate with
`cargo test -p lumetrope-core --test demos -- --ignored write_demo_samples`):

- `demo-loader.json` — plain Lottie: a trimmed arc sweeping round (trim paths + rotation).
- `demo-button.lottie` — dotLottie 2.0: a button that pops in and spins on click (event input).
- `demo-toggle.lottie` — dotLottie 2.0: a switch whose knob slides, driven by a boolean input.

Other files:

- `../crates/core/tests/golden/*.json` — hand-authored Lottie files used by the renderer's golden tests (MIT, part of this repo).
- `lottiefiles/` — 25 interactive dotLottie v2 files (each with state machines) downloaded from
  https://lottiefiles.com/free-animations/interactivity for **local testing only**. They are free
  animations under the LottieFiles licence and are git-ignored, so they are not redistributed with
  Lumetrope. Re-download them with the URLs listed on that page if needed.
