import { useMemo } from "react";
import { cx } from "../lib/format";

const TEXTURE = [
  "abacbbab",
  "bacabbca",
  "abbbacba",
  "cababbbc",
  "bbaabacb",
  "acbbabac",
  "bbacabba",
  "ababbcab",
];

const SHADES: Record<string, string> = {
  a: "#7db64b",
  b: "#8bc75a",
  c: "#6ba33f",
  d: "#9bd56b",
};

const FACE: Array<[number, number]> = [
  [2, 4],
  [3, 4],
  [4, 4],
  [5, 4],
  [2, 5],
  [3, 5],
  [4, 5],
  [5, 5],
  [10, 4],
  [11, 4],
  [12, 4],
  [13, 4],
  [10, 5],
  [11, 5],
  [12, 5],
  [13, 5],
  [6, 8],
  [7, 8],
  [8, 8],
  [9, 8],
  [4, 9],
  [5, 9],
  [6, 9],
  [7, 9],
  [8, 9],
  [9, 9],
  [10, 9],
  [11, 9],
  [4, 10],
  [5, 10],
  [6, 10],
  [7, 10],
  [8, 10],
  [9, 10],
  [10, 10],
  [11, 10],
  [4, 11],
  [5, 11],
  [6, 11],
  [7, 11],
  [8, 11],
  [9, 11],
  [10, 11],
  [11, 11],
  [4, 12],
  [5, 12],
  [10, 12],
  [11, 12],
  [4, 13],
  [5, 13],
  [10, 13],
  [11, 13],
];

interface CreeperMarkProps {
  size?: number;
  className?: string;
  title?: string;
  halo?: boolean;
  muted?: boolean;
}

export function CreeperMark({
  size = 48,
  className,
  title = "苦力怕",
  halo = false,
  muted = false,
}: CreeperMarkProps) {
  const cells = useMemo(() => {
    const output: Array<{
      x: number;
      y: number;
      fill: string;
      opacity: number;
      size: number;
    }> = [];
    for (let y = 0; y < 8; y++) {
      for (let x = 0; x < 8; x++) {
        const shade = TEXTURE[y][x];
        const opacity = x === 0 || y === 0 ? 0.86 : x === 7 || y === 7 ? 0.92 : 1;
        output.push({
          x: x * 2,
          y: y * 2,
          fill: SHADES[shade] ?? SHADES.a,
          opacity,
          size: 2,
        });
        if ((x + y * 3) % 17 === 0) {
          output.push({
            x: x * 2,
            y: y * 2,
            fill: SHADES.d,
            opacity: 0.2,
            size: 2,
          });
        }
      }
    }
    return output;
  }, []);

  return (
    <span
      className={cx("creeper-mark", halo && "creeper-halo", muted && "creeper-muted", className)}
      style={{ width: size, height: size }}
    >
      <svg
        viewBox="0 0 16 16"
        width={size}
        height={size}
        role="img"
        aria-label={title}
        shapeRendering="crispEdges"
      >
        <rect width="16" height="16" fill="#0a120c" />
        {cells.map((cell, index) => (
          <rect
            key={`t-${index}`}
            x={cell.x}
            y={cell.y}
            width={cell.size}
            height={cell.size}
            fill={cell.fill}
            opacity={cell.opacity}
          />
        ))}
        {/* Creeper face, scaled from the canonical 8×8 grid. */}
        {FACE.map(([x, y], index) => (
          <rect
            key={`f-${index}`}
            x={x}
            y={y}
            width="2"
            height="2"
            fill="#0b140d"
          />
        ))}
        <rect x="1" y="1" width="1" height="1" fill="#d4f5b0" opacity="0.5" />
        <rect x="14" y="1" width="1" height="1" fill="#e6ffc9" opacity="0.28" />
        <rect x="1" y="14" width="1" height="1" fill="#07100a" opacity="0.4" />
      </svg>
    </span>
  );
}


