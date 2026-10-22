import { useEditor } from "../store";
import { formatTime } from "../lib/time";
import { Icon } from "./Icon";

export function PlayerBar() {
  const { info, frame, playing, loop, speed, setFrame, setPlaying, setLoop, setSpeed, mode, smSnap } = useEditor();
  if (!info) return null;
  if (mode !== "animate")
    return smSnap ? (
      <div className="player sm-player">
        <span className="sm-state-badge">{smSnap.state}</span>
        <span className="timecode num">{formatTime(frame - info.ip, info.fr)}</span>
      </div>
    ) : null;
  const last = Math.max(info.ip, Math.ceil(info.op) - 1);
  return (
    <div className="player">
      <span className="timecode num">{formatTime(frame - info.ip, info.fr)}</span>
      <button className="icon-btn" data-tip="First frame" onClick={() => setFrame(info.ip)}>
        <Icon name="first" />
      </button>
      <button className="icon-btn" data-tip="Previous frame (,)" onClick={() => setFrame(frame - 1)}>
        <Icon name="prev" />
      </button>
      <button className="play" aria-label={playing ? "Pause" : "Play"} onClick={() => setPlaying(!playing)}>
        <Icon name={playing ? "pause" : "play"} fill />
      </button>
      <button className="icon-btn" data-tip="Next frame (.)" onClick={() => setFrame(frame + 1)}>
        <Icon name="next" />
      </button>
      <button className="icon-btn" data-tip="Last frame" onClick={() => setFrame(last)}>
        <Icon name="last" />
      </button>
      <button className={`icon-btn ${loop ? "active" : ""}`} data-tip="Loop" onClick={() => setLoop(!loop)}>
        <Icon name="loop" />
      </button>
      <select value={speed} onChange={(e) => setSpeed(Number(e.target.value))} aria-label="Playback speed">
        {[0.25, 0.5, 1, 1.5, 2].map((s) => (
          <option key={s} value={s}>
            {s}×
          </option>
        ))}
      </select>
      <button className="icon-btn" data-tip="Play full screen (F)" aria-label="Play full screen" onClick={() => useEditor.setState({ fullscreen: true })}>
        <Icon name="expand" />
      </button>
    </div>
  );
}
