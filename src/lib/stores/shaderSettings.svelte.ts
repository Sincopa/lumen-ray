export interface ShaderPreset {
	id: string;
	nameKey: string;
	colorA: string;
	colorB: string;
}

export const SHADER_PRESETS: ShaderPreset[] = [
	{
		id: 'aurora',
		nameKey: 'settings.presetAurora',
		colorA: '#10b981',
		colorB: '#06b6d4',
	},
	{
		id: 'dawn',
		nameKey: 'settings.presetDawn',
		colorA: '#ff3888',
		colorB: '#ffa033',
	},
	{
		id: 'ocean',
		nameKey: 'settings.presetOcean',
		colorA: '#2563eb',
		colorB: '#00f5d4',
	},
	{
		id: 'sakura',
		nameKey: 'settings.presetSakura',
		colorA: '#f472b6',
		colorB: '#818cf8',
	},
	{
		id: 'cyberpunk',
		nameKey: 'settings.presetCyberpunk',
		colorA: '#00e5ff',
		colorB: '#ff007f',
	},
	{
		id: 'sunset',
		nameKey: 'settings.presetSunset',
		colorA: '#ef4444',
		colorB: '#f97316',
	},
	{
		id: 'lumen',
		nameKey: 'settings.presetLumen',
		colorA: '#6841ee',
		colorB: '#8d68fa',
	},
	{
		id: 'monochrome',
		nameKey: 'settings.presetMonochrome',
		colorA: '#666666',
		colorB: '#e0e0e0',
	},
];

const STORAGE_KEY = 'lumen_shader_settings';

class ShaderSettingsManager {
	enabled = $state(true);
	tooltipsEnabled = $state(true);
	// Default idle state is the monochrome preset's active state (#666666 and #e0e0e0) dimmed by 25%
	idleColorA = $state<string>('#4d4d4d');
	idleColorB = $state<string>('#a8a8a8');
	// Default active preset is 'aurora'
	colorA = $state<string>('#10b981');
	colorB = $state<string>('#06b6d4');
	exposure = $state<number>(1.6);
	selectedPreset = $state<string>('aurora');

	// Compatibility getter/setter for idleColor
	get idleColor(): string {
		return this.idleColorA;
	}
	set idleColor(val: string) {
		this.setIdleColorA(val);
	}

	constructor() {
		if (typeof window !== 'undefined') {
			try {
				const saved =
					localStorage.getItem(STORAGE_KEY) ??
					localStorage.getItem('misc_shader_settings');
				if (saved) {
					const parsed = JSON.parse(saved);
					if (typeof parsed.enabled === 'boolean')
						this.enabled = parsed.enabled;
					if (typeof parsed.tooltipsEnabled === 'boolean')
						this.tooltipsEnabled = parsed.tooltipsEnabled;
					if (
						typeof parsed.idleColorA === 'string' &&
						/^#[0-9a-f]{6}$/i.test(parsed.idleColorA)
					)
						this.idleColorA = parsed.idleColorA;
					if (
						typeof parsed.idleColorB === 'string' &&
						/^#[0-9a-f]{6}$/i.test(parsed.idleColorB)
					)
						this.idleColorB = parsed.idleColorB;
					if (
						typeof parsed.colorA === 'string' &&
						/^#[0-9a-f]{6}$/i.test(parsed.colorA)
					)
						this.colorA = parsed.colorA;
					if (
						typeof parsed.colorB === 'string' &&
						/^#[0-9a-f]{6}$/i.test(parsed.colorB)
					)
						this.colorB = parsed.colorB;
					if (
						typeof parsed.exposure === 'number' &&
						Number.isFinite(parsed.exposure) &&
						parsed.exposure >= 0.8 &&
						parsed.exposure <= 2.5
					)
						this.exposure = parsed.exposure;

					// Migrate previous default 'lumen' preset to 'aurora'
					if (
						parsed.selectedPreset === 'lumen' &&
						parsed.colorA === '#6841ee' &&
						parsed.colorB === '#8d68fa'
					) {
						this.applyPreset('aurora');
					} else if (
						parsed.selectedPreset === 'custom' ||
						SHADER_PRESETS.some((p) => p.id === parsed.selectedPreset)
					) {
						this.selectedPreset = parsed.selectedPreset;
					}
				}
			} catch (e) {
				console.error('Failed to load shader settings from localStorage', e);
			}
		}
	}

	private persist() {
		if (typeof window !== 'undefined') {
			try {
				localStorage.setItem(
					STORAGE_KEY,
					JSON.stringify({
						enabled: this.enabled,
						tooltipsEnabled: this.tooltipsEnabled,
						idleColorA: this.idleColorA,
						idleColorB: this.idleColorB,
						colorA: this.colorA,
						colorB: this.colorB,
						exposure: this.exposure,
						selectedPreset: this.selectedPreset,
					}),
				);
			} catch (e) {
				console.error('Failed to save shader settings', e);
			}
		}
	}

	setEnabled(value: boolean) {
		this.enabled = value;
		this.persist();
	}

	setTooltipsEnabled(value: boolean) {
		this.tooltipsEnabled = value;
		this.persist();
	}

	setIdleColorA(hex: string) {
		if (!/^#[0-9a-f]{6}$/i.test(hex)) return;
		this.idleColorA = hex;
		this.persist();
	}

	setIdleColorB(hex: string) {
		if (!/^#[0-9a-f]{6}$/i.test(hex)) return;
		this.idleColorB = hex;
		this.persist();
	}

	setIdleColor(hex: string) {
		this.setIdleColorA(hex);
	}

	setColorA(hex: string) {
		if (!/^#[0-9a-f]{6}$/i.test(hex)) return;
		this.colorA = hex;
		this.selectedPreset = 'custom';
		this.persist();
	}

	setColorB(hex: string) {
		if (!/^#[0-9a-f]{6}$/i.test(hex)) return;
		this.colorB = hex;
		this.selectedPreset = 'custom';
		this.persist();
	}

	setExposure(val: number) {
		if (!Number.isFinite(val)) return;
		this.exposure = Math.max(0.8, Math.min(2.5, val));
		this.persist();
	}

	// Changing presets ONLY changes the active state (when VPN is on)
	applyPreset(presetId: string) {
		const preset = SHADER_PRESETS.find((p) => p.id === presetId);
		if (preset) {
			this.colorA = preset.colorA;
			this.colorB = preset.colorB;
			this.selectedPreset = preset.id;
			this.persist();
		}
	}

	reset() {
		this.enabled = true;
		this.idleColorA = '#4d4d4d';
		this.idleColorB = '#a8a8a8';
		this.applyPreset('aurora');
		this.exposure = 1.6;
		this.persist();
	}
}

export const shaderSettings = new ShaderSettingsManager();
