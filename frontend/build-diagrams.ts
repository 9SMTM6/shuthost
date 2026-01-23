import { readdirSync } from 'fs';
import { execSync } from 'child_process';

const files = readdirSync('assets').filter((f: string) => f.endsWith('.mmd'));
files.forEach((f: string) => {
    const name = f.replace(/\.mmd$/, '');
    execSync(`npx mmdc -i assets/${f} -o assets/generated/${name}.svg --svgId diagram-${name}`, { stdio: 'inherit' });
});
