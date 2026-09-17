export interface ShaderPreset {
  id: string;
  nameKey: string;
  idleColor: string;
  colorA: string;
  colorB: string;
}

export const SHADER_PRESETS: ShaderPreset[] = [
  {
    id: 'lumen',
    nameKey: 'settings.presetLumen',
    idleColor: '#363048',
    colorA: '#6841ee',
    colorB: '#8d68fa',
  },
  {
    id: 'cyberpunk',
    nameKey: 'settings.presetCyberpunk',
    idleColor: '#081426',
    colorA: '#00e5ff',
    colorB: '#ff007f',
  },
  {
    id: 'aurora',
    nameKey: 'settings.presetAurora',
    idleColor: '#061814',
    colorA: '#10b981',
    colorB: '#06b6d4',
  },
  {
    id: 'sunset',
    nameKey: 'settings.presetSunset',
    idleColor: '#220812',
    colorA: '#ef4444',
    colorB: '#f97316',
  },
  {
    id: 'monochrome',
    nameKey: 'settings.presetMonochrome',
    idleColor: '#121212',
    colorA: '#666666',
    colorB: '#e0e0e0',
  },
];

const STORAGE_KEY = 'lumen_shader_settings';

class ShaderSettingsManager {
  enabled = $state(true);
  idleColor = $state<string>('#363048');
  colorA = $state<string>('#6841ee');
  colorB = $state<string>('#8d68fa');
  exposure = $state<number>(1.6);
  selectedPreset = $state<string>('lumen');

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
          if (
            typeof parsed.idleColor === 'string' &&
            /^#[0-9a-f]{6}$/i.test(parsed.idleColor)
          )
            this.idleColor = parsed.idleColor;
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
          if (
            parsed.selectedPreset === 'custom' ||
            SHADER_PRESETS.some((p) => p.id === parsed.selectedPreset)
          )
            this.selectedPreset = parsed.selectedPreset;
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
            idleColor: this.idleColor,
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

  setIdleColor(hex: string) {
    if (!/^#[0-9a-f]{6}$/i.test(hex)) return;
    this.idleColor = hex;
    this.selectedPreset = 'custom';
    this.persist();
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

  applyPreset(presetId: string) {
    const preset = SHADER_PRESETS.find((p) => p.id === presetId);
    if (preset) {
      this.idleColor = preset.idleColor;
      this.colorA = preset.colorA;
      this.colorB = preset.colorB;
      this.selectedPreset = preset.id;
      this.persist();
    }
  }

  reset() {
    this.enabled = true;
    this.applyPreset('lumen');
    this.exposure = 1.6;
    this.persist();
  }
}

export const shaderSettings = new ShaderSettingsManager();
