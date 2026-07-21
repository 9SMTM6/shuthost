import { writeFileSync } from 'node:fs';

/** Viewbox size in px */
const size = 400;
/** Line weight in px */
const stroke = 26;
/** Monitor screen width */
const screenW = 256;
/** Height of the rectangular portion of the monitor (below the circular top arc) */
const rectH = 30;
/** How far above rectTop the stem bottom sits (stem stays above the straight frame) */
const stemAboveRect = 90;
/** Total height of the power stem */
const stemHeight = 55;

/** Width of the stand base at its top edge */
const baseW = 128;
/** Height of the stand base */
const baseH = 26;
/** Extra width per side at the base bottom for the trapezoid flare */
const baseF = 16;

/** Radius of the circular arc forming the monitor top */
const arcR = screenW / 2;
/** How far each quarter-arc is trimmed back from the notch edge (widens gap) */
const arcTrim = stroke * 1.5;
const cx = size / 2;
const screenLeft = (size - screenW) / 2;
const screenRight = screenLeft + screenW;
const screenBottom = (size + rectH + arcR) / 2;
const rectTop = screenBottom - rectH;

const arcEndX = cx - arcTrim;
const arcStartX = cx + arcTrim;
/** y-coordinate where each quarter-arc stops (on the circle at x = arcEndX / arcStartX) */
const arcMeetY = rectTop - Math.sqrt(arcR * arcR - (cx - arcEndX) ** 2);

const powerBottom = rectTop - stemAboveRect;
const powerTop = powerBottom - stemHeight;

const neck = stroke * 1.5;
const neckBottom = screenBottom + neck;
const baseTopLeft = cx - baseW / 2;
const baseTopRight = cx + baseW / 2;
const baseBottomLeft = cx - (baseW + 2 * baseF) / 2;
const baseBottomRight = cx + (baseW + 2 * baseF) / 2;

const contentMinY = powerTop;
const contentMaxY = neckBottom + baseH;
const contentCenterY = (contentMinY + contentMaxY) / 2;
const yOffset = size / 2 - contentCenterY;

const color = '#2b2b2b';
const darkColor = '#e6e6e6';

const paths = `
      <path d="M ${screenLeft} ${rectTop} A ${arcR} ${arcR} 0 0 1 ${arcEndX} ${arcMeetY}"/>
      <path d="M ${arcStartX} ${arcMeetY} A ${arcR} ${arcR} 0 0 1 ${screenRight} ${rectTop}"/>
      <line x1="${cx}" y1="${powerTop}" x2="${cx}" y2="${powerBottom}"/>
      <path d="M ${screenLeft} ${rectTop} V ${screenBottom} H ${screenRight} V ${rectTop}"/>
      <polygon class="f" points="${baseTopLeft},${neckBottom} ${baseTopRight},${neckBottom} ${baseBottomRight},${neckBottom + baseH} ${baseBottomLeft},${neckBottom + baseH}"/>
      <line x1="${cx}" y1="${screenBottom}" x2="${cx}" y2="${neckBottom}"/>`;

// note: usvg does NOT support CSS var() and ignores @media, and it fails silently.
const svg = `<svg height="${size}" viewBox="0 0 ${size} ${size}" width="${size}" xmlns="http://www.w3.org/2000/svg">
  <defs>
    <style><![CDATA[
      .s{fill:none;stroke-linecap:round;stroke-linejoin:round}
      .g{stroke:#fff;stroke-width:${stroke + 6}}
      .m{stroke:${color};stroke-width:${stroke}}
      .f{fill:${color}}
      @media(prefers-color-scheme:dark){.g{stroke:#000;opacity:0.6}.m{stroke:${darkColor}}.f{fill:${darkColor}}}
    ]]></style>
  </defs>
  <g class="s" transform="translate(0, ${yOffset})">
    <g class="g">${paths}</g>
    <g class="m">${paths}</g>
  </g>
</svg>
`;

writeFileSync('src/generated/favicon.svg', svg);
console.info('Generated src/generated/favicon.svg');
