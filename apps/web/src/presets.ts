export const PRESET_GROUPS: { title: string; items: { id: string; label: string }[] }[] = [
  {
    title: "Enter",
    items: [
      { id: "fade_in", label: "Fade in" },
      { id: "scale_in", label: "Scale in" },
      { id: "pop", label: "Pop" },
      { id: "slide_in_left", label: "Slide in ←" },
      { id: "slide_in_right", label: "Slide in →" },
      { id: "slide_in_up", label: "Slide in ↑" },
      { id: "slide_in_down", label: "Slide in ↓" },
    ],
  },
  {
    title: "Exit",
    items: [
      { id: "fade_out", label: "Fade out" },
      { id: "scale_out", label: "Scale out" },
      { id: "slide_out_left", label: "Slide out ←" },
      { id: "slide_out_right", label: "Slide out →" },
      { id: "slide_out_up", label: "Slide out ↑" },
      { id: "slide_out_down", label: "Slide out ↓" },
    ],
  },
  {
    title: "Emphasis",
    items: [
      { id: "spin", label: "Spin" },
      { id: "bounce", label: "Bounce" },
      { id: "pulse", label: "Pulse" },
      { id: "shake", label: "Shake" },
      { id: "wiggle", label: "Wiggle" },
      { id: "float", label: "Float" },
      { id: "blink", label: "Blink" },
    ],
  },
];
