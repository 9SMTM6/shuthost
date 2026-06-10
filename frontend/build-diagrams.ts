import { mkdirSync, readdirSync, readFileSync, writeFileSync } from 'node:fs';
import { D2 } from '@terrastruct/d2';

// Theme colors derived from the frontend's VS Code-like light/dark palette.
// Prepended to every diagram source so the theme is defined once here.
//
// N1 (fg/text) -> N7 (page background). In dark mode, N1 is light, N7 is dark.
// B1-B6: base/accent colors (containers, primary accents).
const D2_THEME_HEADER = `\
vars: {
  d2-config: {
    layout-engine: elk
    pad: 20
    theme-overrides: {
      N1: "#111111"
      N2: "#616161"
      N3: "#858585"
      N4: "#a0a0a0"
      N5: "#e5e5e5"
      N6: "#f3f3f3"
      N7: "#ffffff"
      B1: "#005fb8"
      B2: "#0078d4"
      B3: "#cce4f7"
      B4: "#e8f4fb"
      B5: "#f3f3f3"
      B6: "#ffffff"
      AA2: "#005fb8"
      AA4: "#e5e5e5"
      AA5: "#f3f3f3"
      AB4: "#e5e5e5"
      AB5: "#f3f3f3"
    }
    dark-theme-overrides: {
      N1: "#cccccc"
      N2: "#9d9d9d"
      N3: "#707075"
      N4: "#4e4e52"
      N5: "#3e3e42"
      N6: "#252526"
      N7: "#1e1e1e"
      B1: "#3794ff"
      B2: "#0078d4"
      B3: "#094771"
      B4: "#2d2d30"
      B5: "#252526"
      B6: "#1e1e1e"
      AA2: "#4fc3f7"
      AA4: "#2d2d30"
      AA5: "#252526"
      AB4: "#2d2d30"
      AB5: "#1e1e1e"
    }
  }
}
***: {style.font-size: 18}
(*** -> ***)[*]: {style.font-size: 19}
`;

const getFontFamilyFromTailwind = () => {
    const css = readFileSync('src/styles.tailwind.css', 'utf8');
    const match = css.match(/--font-sans:\s*([^;]+);/s);
    // biome-ignore lint/style/noNonNullAssertion: We expect this to always be present in the generated Tailwind CSS.
    return match![1]!.trim().replace(/\s+/g, ' ');
};

const FONT_FAMILY = getFontFamilyFromTailwind();

const replaceEmbeddedFonts = (svg: string, fontFamily: string) => {
    let stripped = svg
        // Remove embedded @font-face rules that carry base64 blobs.
        .replace(/@font-face\s*{[\s\S]*?}/g, '')
        // Remove any src URL references (including data URIs) if left behind.
        .replace(/src:\s*url\([^)]*\)\s*;?/g, '');

    const replacementCss = `<![CDATA[
[class^="d2-"] .text {
  font-family: ${fontFamily};
}
[class^="d2-"] .text-bold {
  font-family: ${fontFamily};
  font-weight: bold;
}
[class^="d2-"] .text-italic {
  font-family: ${fontFamily};
  font-style: italic;
}
]]>`;

    let replaced = false;
    stripped = stripped.replace(
        /<style([^>]*)>([\s\S]*?)<\/style>/g,
        (_match, attrs, content) => {
            if (!replaced && /\.text\s*\{/.test(content)) {
                replaced = true;
                return `<style${attrs}>${replacementCss}</style>`;
            }

            return `<style${attrs}>${content}</style>`;
        },
    );

    if (!replaced) {
        stripped = stripped.replace(
            /(<svg[^>]*>)/,
            `$1<style type="text/css">${replacementCss}</style>`,
        );
    }

    return stripped;
};

const d2 = new D2();

try {
    mkdirSync('src/generated', { recursive: true });
    const files = readdirSync('src').filter((f) => f.endsWith('.d2'));
    for (const f of files) {
        const name = f.replace(/\.d2$/, '');
        const source = readFileSync(`src/${f}`, 'utf8');

        const result = await d2.compile(`${D2_THEME_HEADER}\n${source}`);
        const svg = await d2.render(result.diagram, {
            ...result.renderOptions,
            // Theme 0 = Default (light), Theme 200 = Dark Mauve as base for dark mode.
            // Colors are fully overridden by dark-theme-overrides above.
            darkThemeID: 200,
            // Omit XML declaration so SVGs can be embedded directly via innerHTML
            noXMLTag: true,
        });

        let optimizedSvg = replaceEmbeddedFonts(svg, FONT_FAMILY);
        // override the connection font size, since D2s default font seems to have less horizontal width
        optimizedSvg = optimizedSvg.replace(
            /(<text[^>]+class="text-italic fill-N2"[^>]*style="[^"]*?)font-size:19px/gs,
            '$1font-size:16px',
        );

        writeFileSync(`src/generated/${name}.svg`, optimizedSvg);
        console.info(`Generated src/generated/${name}.svg`);
    }
} catch (error) {
    console.error('Error building diagrams:', error);
    process.exit(1);
}

// The D2 WASM runtime keeps a WebWorker alive after use; explicitly exit so
// Node doesn't hang waiting for it to idle.
process.exit(0);
