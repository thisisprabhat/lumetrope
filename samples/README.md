# Samples

Demo files made with Lumetrope (MIT). Each one was built with the same tool calls an AI agent makes
through the MCP server (shapes, gradients, repeaters, effects, keyframes, themes, state machines) and
is regenerated with
`cargo test -p lumetrope-core --test demos -- --ignored write_demo_samples`:

- `demo-bloom.lottie` — a generative mandala. Tap the flower and it bursts open; tap a swatch to switch
  its colour theme (slots + themes, set from a state-machine action).
- `demo-day-night.lottie` — a landscape that changes time of day. Tap the sun or moon: sky cross-fades
  through dusk, stars twinkle, the cottage lights up, a shooting star crosses (boolean input, four states).
- `demo-buddy.lottie` — a character with a personality. Idles and blinks, perks up when you hover,
  looks around, hops with hearts when petted (pointer enter/exit, a global state, parented rig).
- `demo-groove.lottie` — a music player. The record spins up, the tonearm drops, the equalizer dances,
  notes drift up; tap again and it winds down (boolean input, looping and one-shot states).

To eyeball them without the app:
`PREVIEW_DIR=/some/dir cargo test -p lumetrope-core --test demos -- --ignored preview_demo_samples`
writes a contact sheet PNG per demo.

Other files:

- `../crates/core/tests/golden/*.json` — hand-authored Lottie files used by the renderer's golden tests (MIT, part of this repo).
- `lottiefiles/` — 25 interactive dotLottie v2 files (each with state machines) downloaded from
  https://lottiefiles.com/free-animations/interactivity for **local testing only**. They are free
  animations under the LottieFiles licence and are git-ignored, so they are not redistributed with
  Lumetrope. Re-download them with the URLs listed on that page if needed.
