import { useEffect, useRef } from "react";
import { CreeperMark } from "./CreeperMark";

interface SkinHeadProps {
  skin: string | null;
  size?: number;
  label: string;
}

/** Crop the 8×8 face (and hat layer) out of a standard Minecraft skin. */
export function SkinHead({ skin, size = 56, label }: SkinHeadProps) {
  const canvasRef = useRef<HTMLCanvasElement>(null);

  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas || !skin) return;
    const context = canvas.getContext("2d");
    if (!context) return;
    context.imageSmoothingEnabled = false;
    context.clearRect(0, 0, 64, 64);
    const image = new Image();
    image.onload = () => {
      const scale = Math.max(1, Math.floor(image.width / 64));
      context.clearRect(0, 0, 64, 64);
      context.drawImage(image, 8 * scale, 8 * scale, 8 * scale, 8 * scale, 0, 0, 64, 64);
      if (image.height >= 64) {
        context.drawImage(
          image,
          40 * scale,
          8 * scale,
          8 * scale,
          8 * scale,
          0,
          0,
          64,
          64,
        );
      }
    };
    image.src = skin;
    return () => {
      image.onload = null;
    };
  }, [skin]);

  if (!skin) {
    return (
      <span
        className="skin-head-fallback"
        style={{ width: size, height: size }}
        aria-hidden="true"
      >
        <CreeperMark size={Math.round(size * 0.64)} muted />
      </span>
    );
  }

  return (
    <canvas
      ref={canvasRef}
      className="skin-head"
      width={64}
      height={64}
      style={{ width: size, height: size }}
      role="img"
      aria-label={`${label} 的头像`}
    />
  );
}
