import { writeFileSync } from 'node:fs';

/** Viewbox size in px */
const size = 400;
/** Line weight in px */
const stroke = 18;
/** Monitor screen width */
const screenW = 256;
/** Height of the rectangular portion of the monitor (below the circular top arc) */
const rectH = 30;
/** Radius of the circular arc forming the monitor top */
const arcR = screenW / 2;
/** Width of the power-notch cutout at the top centre */
const notchW = 20;
/** How far each quarter-arc is trimmed back from the notch edge (widens gap) */
const arcTrim = 20;
/** How far above rectTop the stem bottom sits (stem stays above the straight frame) */
const stemAboveRect = 90;
/** Total height of the power stem */
const stemHeight = 55;
/** Height of the stand stem (neck) */
const neck = 20;
/** Gap between the monitor bottom and the neck top */
const neckGap = 25;
/** Width of the stand base at its top edge */
const baseW = 128;
/** Height of the stand base */
const baseH = 26;
/** Extra width per side at the base bottom for the trapezoid flare */
const baseF = 16;

const cx = size / 2;
const screenLeft = (size - screenW) / 2;
const screenRight = screenLeft + screenW;
const screenBottom = (size + rectH + arcR) / 2;
const rectTop = screenBottom - rectH;

const arcEndX = cx - notchW / 2 - arcTrim;
const arcStartX = cx + notchW / 2 + arcTrim;
/** y-coordinate where each quarter-arc stops (on the circle at x = arcEndX / arcStartX) */
const arcMeetY = rectTop - Math.sqrt(arcR * arcR - (cx - arcEndX) ** 2);

const powerBottom = rectTop - stemAboveRect;
const powerTop = powerBottom - stemHeight;

const neckTop = screenBottom + neckGap;
const neckBottom = neckTop + neck;
const baseTopLeft = cx - baseW / 2;
const baseTopRight = cx + baseW / 2;
const baseBottomLeft = cx - (baseW + 2 * baseF) / 2;
const baseBottomRight = cx + (baseW + 2 * baseF) / 2;

const color = "#2b2b2b";
const darkColor = "#e6e6e6";

const svg = `<svg height="${size}" viewBox="0 0 ${size} ${size}" width="${size}" xmlns="http://www.w3.org/2000/svg">
  <defs>
    <filter id="dropShadow" x="-30%" y="-30%" width="160%" height="160%">
      <feDropShadow dx="0" dy="3" stdDeviation="3" flood-color="#000" flood-opacity="0.28"/>
    </filter>
    <style><![CDATA[
      .m{fill:none;stroke:${color};stroke-width:${stroke};stroke-linecap:round;stroke-linejoin:round;filter:url(#dropShadow)}
      .f{fill:${color}}
      @media(prefers-color-scheme:dark){.m{stroke:${darkColor}}.f{fill:${darkColor}}}
    ]]></style>
  </defs>
  <g class="m">
    <!-- left quarter-arc: from the left side up to arcEndX (trimmed from notch edge) -->
    <path d="M ${screenLeft} ${rectTop} A ${arcR} ${arcR} 0 0 1 ${arcEndX} ${arcMeetY}"/>
    <!-- right quarter-arc: from arcStartX (trimmed from notch edge) down to the right side -->
    <path d="M ${arcStartX} ${arcMeetY} A ${arcR} ${arcR} 0 0 1 ${screenRight} ${rectTop}"/>
    <!-- power-button stem (bottom is stemAboveRect above rectTop, height is stemHeight) -->
    <line x1="${cx}" y1="${powerTop}" x2="${cx}" y2="${powerBottom}"/>
    <!-- lower frame — left side, bottom edge, right side -->
    <path d="M ${screenLeft} ${rectTop} V ${screenBottom} H ${screenRight} V ${rectTop}"/>
    <!-- stand neck -->
    <line x1="${cx}" y1="${neckTop}" x2="${cx}" y2="${neckBottom}"/>
    <!-- stand base (filled trapezoid): top-left → top-right → bottom-right → bottom-left -->
    <polygon points="${baseTopLeft},${neckBottom} ${baseTopRight},${neckBottom} ${baseBottomRight},${neckBottom+baseH} ${baseBottomLeft},${neckBottom+baseH}" class="f"/>
  </g>
</svg>
`;

writeFileSync('src/generated/favicon.svg', svg);
console.info('Generated src/generated/favicon.svg');
