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
    #diagram-${name} { fill: #e0e0e0; }
    #diagram-${name} .node rect, #diagram-${name} .node circle, #diagram-${name} .node ellipse, #diagram-${name} .node polygon, #diagram-${name} .node path { fill: #1a1a1a; stroke: #888; }
    #diagram-${name} .label, #diagram-${name} .label text, #diagram-${name} span { fill: #e0e0e0 !important; color: #e0e0e0 !important; }
    #diagram-${name} .edgePath .path { stroke: #bbb; stroke-width: 2.5px; }
    #diagram-${name} .flowchart-link { stroke: #bbb; }
    #diagram-${name} .cluster rect { fill: #2a2a2a; stroke: #666; }
    #diagram-${name} .cluster text, #diagram-${name} .cluster span { fill: #e0e0e0 !important; color: #e0e0e0 !important; }
    #diagram-${name} .edgeLabel { background-color: rgba(26, 26, 26, 0.8) !important; }
    #diagram-${name} .edgeLabel rect { fill: rgba(26, 26, 26, 0.8) !important; background-color: rgba(26, 26, 26, 0.8) !important; }
    #diagram-${name} .edgeLabel p { background-color: rgba(26, 26, 26, 0.8) !important; }
    #diagram-${name} .labelBkg { background-color: rgba(26, 26, 26, 0.8) !important; }
    #diagram-${name} .arrowheadPath { fill: #bbb; }
    #diagram-${name} .marker { fill: #bbb; stroke: #bbb; }
}
`;

    svgContent = svgContent.replace('</style>', darkModeCSS + '</style>');
    svgContent = svgContent.replace('background-color: white;', 'background-color: transparent;');

    writeFileSync(svgPath, svgContent);
});
