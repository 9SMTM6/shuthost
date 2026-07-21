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

const svg = `<svg height="${size}" viewBox="0 0 ${size} ${size}" width="${size}" xmlns="http://www.w3.org/2000/svg">
  <defs>
    <filter id="dropShadow" x="-30%" y="-30%" width="160%" height="160%">
      <feDropShadow dx="0" dy="4" stdDeviation="6" flood-color="var(--shadow-color)" flood-opacity="var(--shadow-opacity)"/>
    </filter>
    <style><![CDATA[
      :root{--shadow-color:#fff;--shadow-opacity:0.65}
      .m{fill:none;stroke:${color};stroke-width:${stroke};stroke-linecap:round;stroke-linejoin:round;filter:url(#dropShadow)}
      .f{fill:${color}}
      @media(prefers-color-scheme:dark){:root{--shadow-color:#000;--shadow-opacity:0.35}.m{stroke:${darkColor}}.f{fill:${darkColor}}}
    ]]></style>
  </defs>
  <g class="m" transform="translate(0, ${yOffset})">
    <!-- left quarter-arc: from the left side up to arcEndX (trimmed from notch edge) -->
    <path d="M ${screenLeft} ${rectTop} A ${arcR} ${arcR} 0 0 1 ${arcEndX} ${arcMeetY}"/>
    <!-- right quarter-arc: from arcStartX (trimmed from notch edge) down to the right side -->
    <path d="M ${arcStartX} ${arcMeetY} A ${arcR} ${arcR} 0 0 1 ${screenRight} ${rectTop}"/>
    <!-- power-button stem (bottom is stemAboveRect above rectTop, height is stemHeight) -->
    <line x1="${cx}" y1="${powerTop}" x2="${cx}" y2="${powerBottom}"/>
    <!-- lower frame — left side, bottom edge, right side -->
    <path d="M ${screenLeft} ${rectTop} V ${screenBottom} H ${screenRight} V ${rectTop}"/>
    <!-- stand base (filled trapezoid): top-left → top-right → bottom-right → bottom-left -->
    <polygon points="${baseTopLeft},${neckBottom} ${baseTopRight},${neckBottom} ${baseBottomRight},${neckBottom + baseH} ${baseBottomLeft},${neckBottom + baseH}" class="f"/>
    <!-- stand neck -->
    <line x1="${cx}" y1="${screenBottom}" x2="${cx}" y2="${neckBottom}"/>
  </g>
</svg>
`;

writeFileSync('src/generated/favicon.svg', svg);
console.info('Generated src/generated/favicon.svg');
