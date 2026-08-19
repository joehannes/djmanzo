/**
 * A scene, drawn with Canvas 2D.
 *
 * The baseline tier, and the one that is always honest: it needs no GPU, so it
 * cannot *silently* fall back to software — it already is software. That
 * property is why it was built before the WebGL one, per
 * [ADR-0009](../../../docs/adr/0009-the-living-interface.md).
 *
 * Nothing here knows what a river is. It draws bands, bars, marks, whirls and
 * streams, and the scene decided which of those a river becomes.
 */
import type { Paint, Primitive, Scene } from "./scene";

const css = (paint: Paint): string =>
  `hsl(${paint.hue.toFixed(1)} ${Math.round(paint.saturation * 100)}% ${Math.round(
    paint.lightness * 100,
  )}% / ${paint.alpha})`;

/**
 * Gradients are far more expensive than a flat fill — the rendering benchmark
 * measured flat discs and so under-counted them badly. Cached against the
 * colours and the two y coordinates, none of which change on most frames.
 */
const gradients = new Map<string, CanvasGradient>();

export function draw(ctx: CanvasRenderingContext2D, scene: Scene): void {
  ctx.clearRect(0, 0, scene.width, ctx.canvas.height);
  for (const primitive of scene.primitives) paint(ctx, primitive);
}

function paint(ctx: CanvasRenderingContext2D, p: Primitive): void {
  switch (p.kind) {
    case "band": {
      if (p.h <= 0) return;
      if (p.bottom) {
        const key = `${p.x}|${p.y}|${p.w}|${p.h}|${p.horizontal}|${css(p.top)}|${css(p.bottom)}`;
        let gradient = gradients.get(key);
        if (!gradient) {
          gradient = p.horizontal
            ? ctx.createLinearGradient(p.x, 0, p.x + p.w, 0)
            : ctx.createLinearGradient(0, p.y, 0, p.y + p.h);
          gradient.addColorStop(0, css(p.top));
          gradient.addColorStop(1, css(p.bottom));
          // Bounded, or a sweeping filter would grow it without limit.
          if (gradients.size > 64) gradients.clear();
          gradients.set(key, gradient);
        }
        ctx.fillStyle = gradient;
      } else {
        ctx.fillStyle = css(p.top);
      }
      ctx.fillRect(p.x, p.y, p.w, p.h);
      return;
    }
    case "bar": {
      if (p.dashed) {
        ctx.save();
        ctx.setLineDash([3, 4]);
        ctx.strokeStyle = css(p.paint);
        ctx.lineWidth = Math.max(1, p.h);
        ctx.beginPath();
        ctx.moveTo(p.x, p.y);
        ctx.lineTo(p.x + p.w, p.y);
        ctx.stroke();
        ctx.restore();
        return;
      }
      ctx.fillStyle = css(p.paint);
      ctx.fillRect(p.x, p.y, p.w, Math.max(1, p.h));
      return;
    }
    case "mark": {
      ctx.fillStyle = css(p.paint);
      ctx.beginPath();
      if (p.shape === "disc") {
        ctx.arc(p.x + p.w / 2, p.y + p.h / 2, p.w / 2, 0, Math.PI * 2);
      } else {
        ctx.moveTo(p.x + p.w / 2, p.y);
        ctx.lineTo(p.x + p.w, p.y + p.h);
        ctx.lineTo(p.x, p.y + p.h);
        ctx.closePath();
      }
      ctx.fill();
      return;
    }
    case "whirl": {
      ctx.beginPath();
      ctx.arc(p.x + p.w / 2, p.y + p.h / 2, p.w / 2, p.from, p.from + p.sweep);
      ctx.strokeStyle = css(p.paint);
      ctx.lineWidth = p.thickness;
      ctx.stroke();
      return;
    }
    case "stream": {
      ctx.beginPath();
      ctx.moveTo(p.x, p.y);
      ctx.bezierCurveTo(p.toX * 0.5, p.y, p.toX * 0.7, p.toY, p.toX, p.toY);
      ctx.lineWidth = p.thickness;
      ctx.lineCap = "round";
      ctx.strokeStyle = css(p.paint);
      ctx.stroke();
    }
  }
}
