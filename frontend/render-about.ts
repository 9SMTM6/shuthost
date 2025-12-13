import fs from 'fs';
import path from 'path';
import handlebars from 'handlebars';

type LicenseOverview = {
    name: string;
    id: string;
    count: number;
    text: string;
}

type UsedBy = {
    crate: {
        name: string;
        version: string;
        repository?: string;
    };
}

type LicenseDetail = {
    name: string;
    id: string;
    text: string;
    used_by: UsedBy[];
}

type CargoAboutData = {
    overview: LicenseOverview[];
    licenses: LicenseDetail[];
    crates?: any[]; // optional
}

type NpmLicenseInfo = {
    licenses: string;
    repository?: string;
    path: string;
    licenseFile: string;
    publisher?: string;
    email?: string;
}

type NpmLicensesData = Record<string, NpmLicenseInfo>;

type AdditionalData = {
    overview?: LicenseOverview[];
    licenses?: LicenseDetail[];
    [key: string]: any;
}

// Mapping from SPDX license IDs to full names for unification
const licenseNameMap: Record<string, string> = {
    "MIT": "MIT License",
    "Apache-2.0": "Apache License 2.0",
    "ISC": "ISC License",
    "BSD-3-Clause": "BSD 3-Clause \"New\" or \"Revised\" License",
    "BSD-2-Clause": "BSD 2-Clause \"Simplified\" License",
    "CC0-1.0": "Creative Commons Zero v1.0 Universal",
    "MPL-2.0": "Mozilla Public License 2.0",
    "Zlib": "zlib License",
    "OpenSSL": "OpenSSL License",
    "Unicode-3.0": "Unicode License v3",
    "CDLA-Permissive-2.0": "Community Data License Agreement Permissive 2.0",
    "CC-BY-3.0": "Creative Commons Attribution 3.0",
    // Add more as needed
};

// Load the cargo about JSON
const cargoAboutPath = path.join(process.cwd(), 'assets', 'generated', 'cargo_about.json');
const cargoAbout: CargoAboutData = JSON.parse(fs.readFileSync(cargoAboutPath, 'utf8'));

// Load additional data if exists (optional)
const additionalDataPath = path.join(process.cwd(), 'additional-data.json');
let additional: AdditionalData = {};
if (fs.existsSync(additionalDataPath)) {
    additional = JSON.parse(fs.readFileSync(additionalDataPath, 'utf8'));
}

// Load npm licenses if exists
const npmLicensesPath = path.join(process.cwd(), 'assets', 'generated', 'npm-licenses.json');
let npmOverview: LicenseOverview[] = [];
let npmLicenses: LicenseDetail[] = [];
if (fs.existsSync(npmLicensesPath)) {
    const npmData: NpmLicensesData = JSON.parse(fs.readFileSync(npmLicensesPath, 'utf8'));
    const licenseMap: Record<string, LicenseOverview> = {};
    const usedByMap: Record<string, UsedBy[]> = {};

    for (const [pkg, info] of Object.entries(npmData)) {
        const license = info.licenses;
        const fullName = licenseNameMap[license] || license;
        if (!licenseMap[fullName] || !usedByMap[fullName]) {
            let text = '';
            try {
                text = fs.readFileSync(info.licenseFile, 'utf8');
            } catch (e) {
                // ignore if file not found
            }
            licenseMap[fullName] = { name: fullName, id: license, count: 0, text };
            usedByMap[fullName] = [];
        }
        licenseMap[fullName].count++;
        const [name, version] = pkg.split('@');
        usedByMap[fullName].push({
            crate: {
                name: name!,
                version: version!,
                repository: info.repository || `https://www.npmjs.com/package/${name}`
            }
        });
    }

    npmOverview = Object.values(licenseMap);
    npmLicenses = npmOverview.map(lic => ({
        name: lic.name,
        id: lic.id,
        text: lic.text,
        used_by: usedByMap[lic.name]!
    }));
}

// Merge data
const overviewMap = new Map<string, LicenseOverview>();
const licensesMap = new Map<string, LicenseDetail>();

// Helper to add overview
const addOverview = (items: LicenseOverview[]) => {
    for (const item of items) {
        const existing = overviewMap.get(item.name);
        if (existing) {
            existing.count += item.count;
        } else {
            overviewMap.set(item.name, { ...item });
        }
    }
};

// Helper to add licenses
const addLicenses = (items: LicenseDetail[]) => {
    for (const item of items) {
        const existing = licensesMap.get(item.name);
        if (existing) {
            existing.used_by.push(...item.used_by);
        } else {
            licensesMap.set(item.name, { ...item, used_by: [...item.used_by] });
        }
    }
};

// Add from cargo
addOverview(cargoAbout.overview || []);
addLicenses(cargoAbout.licenses || []);

// Add from npm
addOverview(npmOverview);
addLicenses(npmLicenses);

// Add from additional
addOverview(additional.overview || []);
addLicenses(additional.licenses || []);

const data: CargoAboutData & AdditionalData = {
    overview: Array.from(overviewMap.values()),
    licenses: Array.from(licensesMap.values()),
    ...additional
};

// Load the template
const templatePath = path.join(process.cwd(), 'about.hbs');
const template = fs.readFileSync(templatePath, 'utf8');

// Compile and render
const compiled = handlebars.compile(template);
const html = compiled(data);

// Write to output
const outputPath = 'assets/generated/about.html';
fs.writeFileSync(outputPath, html);

console.log('About page generated at', outputPath);