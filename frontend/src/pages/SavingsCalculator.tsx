import { Title } from '@solidjs/meta';
import { createSignal, type JSX } from 'solid-js';
import type { AnyComponent } from '../helpers/utils/solid';
import { Footer } from '../sharedComponents/Footer';
import { SimpleHeader } from '../sharedComponents/Header';

const HOURS_PER_WEEK = 7 * 24;
const W_PER_KW = 1_000;
const G_PER_KG = 1_000;
const DAYS_PER_MONTH = 365.25 / 12;
const DAYS_PER_YEAR = 365.25;

export const SavingsCalculatorPage = (() => {
    const [idlePowerW, setIdlePowerW] = createSignal(60);
    const [shutdownPowerW, setShutdownPowerW] = createSignal(1.7);
    const [energyCostPerKwhEur, setEnergyCostPerKwhEur] = createSignal(0.37);
    const [co2PerKwhG, setCo2PerKwhG] = createSignal(344);
    const [activeHoursPerWeek, setActiveHoursPerWeek] = createSignal(4);

    const idleHoursPerDay = () => 24 - activeHoursPerWeek() / HOURS_PER_WEEK;
    const savedPowerW = () => idlePowerW() - shutdownPowerW();
    const energyKwhPerDay = () => (idleHoursPerDay() * savedPowerW()) / W_PER_KW;

    const costPerDayEur = () => energyKwhPerDay() * energyCostPerKwhEur();
    const costPerMonthEur = () => costPerDayEur() * DAYS_PER_MONTH;
    const costPerYearEur = () => costPerDayEur() * DAYS_PER_YEAR;

    const co2PerDayG = () => energyKwhPerDay() * co2PerKwhG();
    const co2PerMonthKg = () => (co2PerDayG() * DAYS_PER_MONTH) / G_PER_KG;
    const co2PerYearKg = () => (co2PerDayG() * DAYS_PER_YEAR) / G_PER_KG;

    const handleInput =
        (setter: (v: number) => void): JSX.EventHandler<HTMLInputElement, InputEvent> =>
        (e) => {
            const val = parseFloat(e.currentTarget.value);
            if (!Number.isNaN(val)) setter(val);
        };

    return (
        <>
            <Title>WoL Savings Calculator - ShutHost Coordinator</Title>
            <SimpleHeader />
            <main
                id="main-content"
                class="main px-4 sm:px-6 lg:px-8 max-w-7xl mx-auto w-full"
                tabindex="-1"
            >
                <section
                    class="py-4 sm:py-6"
                    aria-labelledby="calc-title"
                >
                    <div class="section-container">
                        <h1
                            id="calc-title"
                            class="section-title px-4 pt-4 text-xl sm:text-2xl"
                        >
                            WoL Savings Calculator
                        </h1>
                        <p class="px-4 text-sm text-[#616161] dark:text-[#9d9d9d]">
                            Estimate cost and CO₂ savings from shutting down hosts
                            instead of idling. Defaults based on Germany, 2026
                            (€0.37/kWh, 344 g CO₂/kWh), a typical old home PC used as server,
                            and uptime expected for daily incremental backups.
                            The saving accounts for the standby power still drawn while shut down with active WoL.
                        </p>

                        <div class="m-4 grid grid-cols-1 sm:grid-cols-2 md:grid-cols-3 lg:grid-cols-5 gap-4">
                            <label class="flex flex-col gap-1 text-sm text-[#616161] dark:text-[#9d9d9d]">
                                Idle Power (W)
                                <input
                                    type="number"
                                    value={idlePowerW()}
                                    onInput={handleInput(setIdlePowerW)}
                                    min="0"
                                    step="1"
                                    class="rounded border border-[#e5e5e5] dark:border-[#3e3e42] bg-white dark:bg-[#252526] px-3 py-2 text-black dark:text-[#cccccc]"
                                />
                            </label>
                            <label class="flex flex-col gap-1 text-sm text-[#616161] dark:text-[#9d9d9d]">
                                Shutdown Power - active WoL (W)
                                <input
                                    type="number"
                                    value={shutdownPowerW()}
                                    onInput={handleInput(setShutdownPowerW)}
                                    min="0"
                                    step="0.1"
                                    class="rounded border border-[#e5e5e5] dark:border-[#3e3e42] bg-white dark:bg-[#252526] px-3 py-2 text-black dark:text-[#cccccc]"
                                />
                            </label>
                            <label class="flex flex-col gap-1 text-sm text-[#616161] dark:text-[#9d9d9d]">
                                Energy Cost (€/kWh)
                                <input
                                    type="number"
                                    value={energyCostPerKwhEur()}
                                    onInput={handleInput(setEnergyCostPerKwhEur)}
                                    min="0"
                                    step="0.01"
                                    class="rounded border border-[#e5e5e5] dark:border-[#3e3e42] bg-white dark:bg-[#252526] px-3 py-2 text-black dark:text-[#cccccc]"
                                />
                            </label>
                            <label class="flex flex-col gap-1 text-sm text-[#616161] dark:text-[#9d9d9d]">
                                CO₂ Factor (g/kWh)
                                <input
                                    type="number"
                                    value={co2PerKwhG()}
                                    onInput={handleInput(setCo2PerKwhG)}
                                    min="0"
                                    step="1"
                                    class="rounded border border-[#e5e5e5] dark:border-[#3e3e42] bg-white dark:bg-[#252526] px-3 py-2 text-black dark:text-[#cccccc]"
                                />
                            </label>
                            <label class="flex flex-col gap-1 text-sm text-[#616161] dark:text-[#9d9d9d]">
                                Active Hours / Week
                                <input
                                    type="number"
                                    value={activeHoursPerWeek()}
                                    onInput={handleInput(setActiveHoursPerWeek)}
                                    min="0"
                                    step="0.5"
                                    class="rounded border border-[#e5e5e5] dark:border-[#3e3e42] bg-white dark:bg-[#252526] px-3 py-2 text-black dark:text-[#cccccc]"
                                />
                            </label>
                        </div>

                        <div class="m-4 mt-6 grid grid-cols-1 sm:grid-cols-3 gap-4">
                            <div class="rounded-lg border border-[#e5e5e5] dark:border-[#3e3e42] p-4 bg-[#f9f9f9] dark:bg-[#252526]">
                                <p class="text-xs font-semibold uppercase tracking-wide text-[#616161] dark:text-[#9d9d9d]">
                                    Per Day
                                </p>
                                <p class="mt-2 text-lg font-semibold text-black dark:text-[#cccccc]">
                                    {costPerDayEur().toFixed(4)} €
                                </p>
                                <p class="text-sm text-[#616161] dark:text-[#9d9d9d]">
                                    {co2PerDayG().toFixed(2)} g CO₂
                                </p>
                            </div>
                            <div class="rounded-lg border border-[#e5e5e5] dark:border-[#3e3e42] p-4 bg-[#f9f9f9] dark:bg-[#252526]">
                                <p class="text-xs font-semibold uppercase tracking-wide text-[#616161] dark:text-[#9d9d9d]">
                                    Per Month (30.44 d)
                                </p>
                                <p class="mt-2 text-lg font-semibold text-black dark:text-[#cccccc]">
                                    {costPerMonthEur().toFixed(2)} €
                                </p>
                                <p class="text-sm text-[#616161] dark:text-[#9d9d9d]">
                                    {co2PerMonthKg().toFixed(2)} kg CO₂
                                </p>
                            </div>
                            <div class="rounded-lg border border-[#e5e5e5] dark:border-[#3e3e42] p-4 bg-[#f9f9f9] dark:bg-[#252526]">
                                <p class="text-xs font-semibold uppercase tracking-wide text-[#616161] dark:text-[#9d9d9d]">
                                    Per Year (365.25 d)
                                </p>
                                <p class="mt-2 text-lg font-semibold text-black dark:text-[#cccccc]">
                                    {costPerYearEur().toFixed(2)} €
                                </p>
                                <p class="text-sm text-[#616161] dark:text-[#9d9d9d]">
                                    {co2PerYearKg().toFixed(2)} kg CO₂
                                </p>
                            </div>
                        </div>

                        <details class="m-4 mt-6">
                            <summary class="collapsible-header rounded-lg border border-[#e5e5e5] dark:border-[#3e3e42] text-sm font-semibold text-[#616161] dark:text-[#9d9d9d]">
                                <span>Formulas</span>
                                <span class="collapsible-icon" />
                            </summary>
                            <div class="collapsible-content text-sm font-mono text-black dark:text-[#cccccc] space-y-1">
                                <p>
                                    idle_h = 24 − hours_week / ({7}×24)
                                    {' '}= 24 − {activeHoursPerWeek()} / {HOURS_PER_WEEK}
                                    {' '}= {idleHoursPerDay().toFixed(6)} h
                                </p>
                                <p>
                                    P_saved = P_idle − P_shutdown
                                    {' '}= {idlePowerW()} − {shutdownPowerW()}
                                    {' '}= {savedPowerW()} W
                                </p>
                                <p>
                                    E_day = idle_h × P_saved / 1000
                                    {' '}= {idleHoursPerDay().toFixed(6)} × {savedPowerW()} / {W_PER_KW}
                                    {' '}= {energyKwhPerDay().toFixed(6)} kWh
                                </p>
                                <p>
                                    €_day = E_day × €/kWh
                                    {' '}= {energyKwhPerDay().toFixed(6)} × {energyCostPerKwhEur()}
                                    {' '}= {costPerDayEur().toFixed(6)} €
                                </p>
                                <p>
                                    CO₂_day = E_day × g/kWh
                                    {' '}= {energyKwhPerDay().toFixed(6)} × {co2PerKwhG()}
                                    {' '}= {co2PerDayG().toFixed(4)} g
                                </p>
                                <p class="pt-2">
                                    €_month = €_day × {DAYS_PER_MONTH}
                                    {' '}= {costPerDayEur().toFixed(6)} × {DAYS_PER_MONTH}
                                    {' '}= {costPerMonthEur().toFixed(4)} €
                                </p>
                                <p>
                                    CO₂_month = CO₂_day × {DAYS_PER_MONTH} / 1000
                                    {' '}= {co2PerDayG().toFixed(4)} × {DAYS_PER_MONTH} / {G_PER_KG}
                                    {' '}= {co2PerMonthKg().toFixed(4)} kg
                                </p>
                                <p class="pt-2">
                                    €_year = €_day × {DAYS_PER_YEAR}
                                    {' '}= {costPerDayEur().toFixed(6)} × {DAYS_PER_YEAR}
                                    {' '}= {costPerYearEur().toFixed(4)} €
                                </p>
                                <p>
                                    CO₂_year = CO₂_day × {DAYS_PER_YEAR} / 1000
                                    {' '}= {co2PerDayG().toFixed(4)} × {DAYS_PER_YEAR} / {G_PER_KG}
                                    {' '}= {co2PerYearKg().toFixed(4)} kg
                                </p>
                            </div>
                        </details>
                    </div>
                </section>
            </main>
            <Footer />
        </>
    );
}) satisfies AnyComponent;
