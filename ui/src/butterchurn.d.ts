/**
 * §122: the part of Butterchurn (MIT, 3.0.0-beta.5) djmanzo uses. The
 * library ships no types; this is what `./milkdrop.ts` calls, and nothing
 * more. See `docs/VISUALS.md` step 4.
 */
declare module "butterchurn" {
  export interface AudioLevels {
    timeByteArray: Uint8Array;
    timeByteArrayL: Uint8Array;
    timeByteArrayR: Uint8Array;
  }

  export interface Visualizer {
    loadPreset(preset: object, blendTime: number): Promise<void>;
    render(options: { audioLevels: AudioLevels; elapsedTime: number }): void;
    setRendererSize(width: number, height: number): void;
  }

  const butterchurn: {
    createVisualizer(
      context: AudioContext | null,
      canvas: HTMLCanvasElement,
      options: { width: number; height: number; pixelRatio?: number; textureRatio?: number; onlyUseWASM?: boolean },
    ): Visualizer;
  };
  export default butterchurn;
}
