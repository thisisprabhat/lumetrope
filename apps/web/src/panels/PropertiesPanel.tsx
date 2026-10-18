import { useSceneInfo } from "../doc";
import { useEditor } from "../store";
import { EffectsSection, MasksSection, TextSection } from "./FxPanels";
import { AlignBar, LayerHeader, LayerSettingsSection, TransformSection } from "./LayerPanel";
import { ScenePanel } from "./ScenePanel";
import { ShapeSections } from "./ShapePanel";

export function PropertiesPanel() {
  const selection = useEditor((s) => s.selection);
  const scene = useSceneInfo();
  const layers = scene.layers.filter((l) => selection.includes(l.id));
  if (layers.length === 0) {
    return (
      <div className="panel-scroll">
        <ScenePanel />
      </div>
    );
  }
  if (layers.length > 1) {
    return (
      <div className="panel-scroll">
        <div className="layer-header">
          <span className="type-badge">{layers.length} layers</span>
        </div>
        <AlignBar ids={layers.map((l) => l.id)} />
      </div>
    );
  }
  const l = layers[0];
  return (
    <div className="panel-scroll" key={l.id}>
      <LayerHeader layer={l} />
      <AlignBar ids={[l.id]} />
      <TransformSection id={l.id} />
      {l.type === "shape" && <ShapeSections id={l.id} />}
      {l.type === "text" && <TextSection id={l.id} />}
      <LayerSettingsSection layer={l} />
      <MasksSection id={l.id} />
      <EffectsSection id={l.id} />
    </div>
  );
}
