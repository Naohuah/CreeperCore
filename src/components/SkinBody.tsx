import { useEffect, useRef } from "react";
import { CreeperMark } from "./CreeperMark";

interface SkinBodyProps {
  skin: string | null;
  label: string;
  slim?: boolean;
  size?: number;
}

/** Flat front-facing full-body preview drawn directly from a skin PNG. */
export function SkinBody({ skin, label, slim = false, size = 58 }: SkinBodyProps) {
  const canvasRef = useRef<HTMLCanvasElement>(null);

  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas || !skin) return;
    const context = canvas.getContext("2d");
    if (!context) return;
    context.imageSmoothingEnabled = false;
    context.clearRect(0, 0, 64, 128);
    const image = new Image();
    image.onload = () => {
      const scale = Math.max(1, Math.floor(image.width / 64));
      const px = 4;
      const arm = slim ? 3 : 4;
      const draw = (
        sourceX: number,
        sourceY: number,
        sourceW: number,
        sourceH: number,
        x: number,
        y: number,
        width: number,
        height: number,
      ) => {
        context.drawImage(
          image,
          sourceX * scale,
          sourceY * scale,
          sourceW * scale,
          sourceH * scale,
          x,
          y,
          width,
          height,
        );
      };
      // Legs, body, arms, then head. Old 64×32 skins only have right limbs.
      draw(4, 20, 4, 12, 8 * px, 20 * px, 4 * px, 12 * px);
      if (image.height >= 64) {
        draw(20, 52, 4, 12, 4 * px, 20 * px, 4 * px, 12 * px);
      } else {
        draw(4, 20, 4, 12, 4 * px, 20 * px, 4 * px, 12 * px);
      }
      draw(20, 20, 8, 12, 4 * px, 8 * px, 8 * px, 12 * px);
      draw(44, 20, arm, 12, 12 * px, 8 * px, arm * px, 12 * px);
      if (image.height >= 64) {
        draw(36, 52, arm, 12, 0, 8 * px, arm * px, 12 * px);
      } else {
        draw(44, 20, arm, 12, 0, 8 * px, arm * px, 12 * px);
      }
      draw(8, 8, 8, 8, 4 * px, 0, 8 * px, 8 * px);
      if (image.height >= 64) {
        draw(20, 36, 8, 12, 4 * px, 8 * px, 8 * px, 12 * px);
        draw(44, 36, arm, 12, 12 * px, 8 * px, arm * px, 12 * px);
        draw(52, 52, arm, 12, 0, 8 * px, arm * px, 12 * px);
        draw(40, 8, 8, 8, 4 * px, 0, 8 * px, 8 * px);
      }
    };
    image.src = skin;
    return () => {
      image.onload = null;
    };
  }, [skin, slim]);

  if (!skin) {
    return <CreeperMark size={Math.round(size * 0.7)} muted />;
  }

  return (
    <canvas
      ref={canvasRef}
      className="skin-body"
      width={64}
      height={128}
      style={{ width: size, height: size * 2 }}
      role="img"
      aria-label={`${label} 的全身皮肤预览`}
    />
  );
}
