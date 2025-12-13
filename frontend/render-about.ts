import fs from 'fs';
import path from 'path';
import handlebars from 'handlebars';

interface LicenseOverview {
    name: string;
    id: string;
    count: number;
    text: string;
}

interface UsedBy {
    name: string;
    version: string;
    repository?: string;
    crate?: {
        name: string;
        version: string;
        repository?: string;
    };
}

interface LicenseDetail {
    name: string;
    id: string;
    text: string;
    used_by: UsedBy[];
}

interface CargoAboutData {
    overview: LicenseOverview[];
    licenses: LicenseDetail[];
    crates?: any[]; // optional
}

interface NpmLicenseInfo {
    licenses: string;
    repository?: string;
    path: string;
    licenseFile: string;
    publisher?: string;
    email?: string;
}

type NpmLicensesData = Record<string, NpmLicenseInfo>;

interface AdditionalData {
    overview?: LicenseOverview[];
    licenses?: LicenseDetail[];
    [key: string]: any;
}

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
const npmLicensesPath = path.join(process.cwd(), 'npm-licenses.json');
let npmOverview: LicenseOverview[] = [];
let npmLicenses: LicenseDetail[] = [];
if (fs.existsSync(npmLicensesPath)) {
    const npmData: NpmLicensesData = JSON.parse(fs.readFileSync(npmLicensesPath, 'utf8'));
    const licenseMap: Record<string, LicenseOverview> = {};
    const usedByMap: Record<string, UsedBy[]> = {};

    for (const [pkg, info] of Object.entries(npmData)) {
        const license = info.licenses;
        if (!licenseMap[license] || !usedByMap[license]) {
            licenseMap[license] = { name: license, id: license, count: 0, text: '' }; // text empty for now
            usedByMap[license] = [];
        }
        licenseMap[license].count++;
        const [name, version] = pkg.split('@');
        usedByMap[license].push({
            name: name!,
            version: version!,
            repository: info.repository!
        });
    }

    npmOverview = Object.values(licenseMap);
    npmLicenses = npmOverview.map(lic => ({
        name: lic.name,
        id: lic.id,
        text: lic.text,
        used_by: usedByMap[lic.id]!
    }));
}

// Merge data
const data: CargoAboutData & AdditionalData = {
    overview: [...(cargoAbout.overview || []), ...npmOverview, ...(additional.overview || [])],
    licenses: [...(cargoAbout.licenses || []), ...npmLicenses, ...(additional.licenses || [])],
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