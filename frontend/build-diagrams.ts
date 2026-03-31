import { readdirSync, readFileSync, writeFileSync } from 'fs';
import { execSync } from 'child_process';

const files = readdirSync('assets').filter((f) => f.endsWith('.mmd'));
files.forEach((f) => {
    const name = f.replace(/\.mmd$/, '');
    execSync(`npx mmdc -i assets/${f} -o assets/generated/${name}.svg --svgId diagram-${name} --backgroundColor transparent`, { stdio: 'inherit' });

    // Post-process SVG for dark mode
    const svgPath = `assets/generated/${name}.svg`;
    let svgContent = readFileSync(svgPath, 'utf8');

    const darkModeCSS = `
#diagram-${name} { 
    @media (prefers-color-scheme: dark) {
        fill: #e0e0e0;
        .node rect, .node circle, .node ellipse, .node polygon, .node path { fill: #1a1a1a; stroke: #888; }
        .label, .label text, span { fill: #e0e0e0 !important; color: #e0e0e0 !important; }
        .edgePath .path { stroke: #bbb; stroke-width: 2.5px; }
        .flowchart-link { stroke: #bbb; }
        .cluster rect { fill: #2a2a2a; stroke: #666; }
        .cluster text, .cluster span { fill: #e0e0e0 !important; color: #e0e0e0 !important; }
        .edgeLabel { background-color: rgba(26, 26, 26, 0.8) !important; }
        .edgeLabel rect { fill: rgba(26, 26, 26, 0.8) !important; background-color: rgba(26, 26, 26, 0.8) !important; }
        .edgeLabel p { background-color: rgba(26, 26, 26, 0.8) !important; }
        .labelBkg { background-color: rgba(26, 26, 26, 0.8) !important; }
        .arrowheadPath { fill: #bbb; }
        .marker { fill: #bbb; stroke: #bbb; }
    }
}
`;

    if (!svgContent.includes('</style>')) {
        throw new Error(`Missing </style> in SVG for diagram ${name}`);
    };
    svgContent = svgContent.replace('</style>', darkModeCSS + '</style>');

    writeFileSync(svgPath, svgContent);
});
