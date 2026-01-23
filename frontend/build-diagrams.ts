import { readdirSync, readFileSync, writeFileSync } from 'fs';
import { execSync } from 'child_process';

const files = readdirSync('assets').filter((f) => f.endsWith('.mmd'));
files.forEach((f) => {
    const name = f.replace(/\.mmd$/, '');
    execSync(`npx mmdc -i assets/${f} -o assets/generated/${name}.svg --svgId diagram-${name}`, { stdio: 'inherit' });

    // Post-process SVG for dark mode
    const svgPath = `assets/generated/${name}.svg`;
    let svgContent = readFileSync(svgPath, 'utf8');

    const darkModeCSS = `
@media (prefers-color-scheme: dark) {
    #diagram-${name} { fill: #ccc; }
    #diagram-${name} .node rect, #diagram-${name} .node circle, #diagram-${name} .node ellipse, #diagram-${name} .node polygon, #diagram-${name} .node path { fill: #333; stroke: #666; }
    #diagram-${name} .label, #diagram-${name} .label text, #diagram-${name} span { fill: #ccc; color: #ccc; }
    #diagram-${name} .edgePath .path { stroke: #ccc; }
    #diagram-${name} .cluster rect { fill: #444; stroke: #777; }
    #diagram-${name} .cluster text, #diagram-${name} .cluster span { fill: #ccc; }
    #diagram-${name} .arrowheadPath { fill: #ccc; }
}
`;

    svgContent = svgContent.replace('</style>', darkModeCSS + '</style>');
    svgContent = svgContent.replace('background-color: white;', 'background-color: transparent;');

    writeFileSync(svgPath, svgContent);
});
