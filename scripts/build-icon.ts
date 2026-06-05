// Renders the "A · Sonnentaler" variant from the standalone design HTML
// into a square SVG (and PNG once rasterized).

function sunburst(
  cx: number,
  cy: number,
  rIn: number,
  rOut: number,
  count: number,
  halfDeg: number,
  offset = 0,
): string {
  let d = "";
  const aw = (halfDeg * Math.PI) / 180;
  for (let i = 0; i < count; i++) {
    const a = (i / count) * 2 * Math.PI - Math.PI / 2 + offset;
    const x1 = cx + rIn * Math.cos(a - aw);
    const y1 = cy + rIn * Math.sin(a - aw);
    const x2 = cx + rOut * Math.cos(a);
    const y2 = cy + rOut * Math.sin(a);
    const x3 = cx + rIn * Math.cos(a + aw);
    const y3 = cy + rIn * Math.sin(a + aw);
    d += `M${x1.toFixed(1)} ${y1.toFixed(1)} L${x2.toFixed(1)} ${y2.toFixed(1)} L${x3.toFixed(1)} ${y3.toFixed(1)} Z `;
  }
  return d.trim();
}

const rays = sunburst(256, 256, 150, 212, 16, 7.2);

const svg = `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 512 512" width="1024" height="1024">
  <defs>
    <linearGradient id="bg" x1="0" y1="0" x2="1" y2="1">
      <stop offset="0" stop-color="#FFC65A" />
      <stop offset="1" stop-color="#F5872A" />
    </linearGradient>
    <linearGradient id="coin" x1="0" y1="0" x2="0.4" y2="1">
      <stop offset="0" stop-color="#FFF0C2" />
      <stop offset="1" stop-color="#F4A92C" />
    </linearGradient>
  </defs>
  <rect width="512" height="512" fill="url(#bg)" />
  <path d="${rays}" fill="#FFF4D6" opacity="0.92" />
  <circle cx="256" cy="256" r="146" fill="url(#coin)" />
  <circle cx="256" cy="256" r="146" fill="none" stroke="#ffffff" stroke-opacity="0.55" stroke-width="6" />
  <circle cx="256" cy="256" r="120" fill="none" stroke="#C7791A" stroke-opacity="0.30" stroke-width="4" />
  <text x="256" y="258" text-anchor="middle" dominant-baseline="central"
        font-family="Manrope, 'DejaVu Sans', sans-serif" font-weight="800" font-size="184" fill="#B5560E">€</text>
</svg>
`;

await Bun.write("app-icon.svg", svg);
console.log("Wrote app-icon.svg (1024×1024)");
