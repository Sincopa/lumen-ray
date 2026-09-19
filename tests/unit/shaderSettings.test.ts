import { describe, it, expect, beforeEach } from 'vitest';
import {
	shaderSettings,
	SHADER_PRESETS,
} from '../../src/lib/stores/shaderSettings.svelte';

describe('shaderSettings', () => {
	beforeEach(() => {
		shaderSettings.reset();
	});

	it('has aurora as the default preset with dimmed monochrome idle colors', () => {
		expect(shaderSettings.selectedPreset).toBe('aurora');
		expect(shaderSettings.idleColorA).toBe('#4d4d4d');
		expect(shaderSettings.idleColorB).toBe('#a8a8a8');
		expect(shaderSettings.colorA).toBe('#10b981');
		expect(shaderSettings.colorB).toBe('#06b6d4');
	});

	it('includes dawn, ocean, and sakura in presets', () => {
		const presetIds = SHADER_PRESETS.map((p) => p.id);
		expect(presetIds).toContain('dawn');
		expect(presetIds).toContain('ocean');
		expect(presetIds).toContain('sakura');
		expect(presetIds).toContain('aurora');
		expect(presetIds).toContain('monochrome');
	});

	it('switching presets only alters active colors, preserving idle colors', () => {
		shaderSettings.applyPreset('dawn');
		expect(shaderSettings.selectedPreset).toBe('dawn');
		expect(shaderSettings.colorA).toBe('#ff3888');
		expect(shaderSettings.colorB).toBe('#ffa033');
		// Idle colors must remain the dimmed monochrome
		expect(shaderSettings.idleColorA).toBe('#4d4d4d');
		expect(shaderSettings.idleColorB).toBe('#a8a8a8');

		shaderSettings.applyPreset('ocean');
		expect(shaderSettings.selectedPreset).toBe('ocean');
		expect(shaderSettings.colorA).toBe('#2563eb');
		expect(shaderSettings.colorB).toBe('#00f5d4');
		expect(shaderSettings.idleColorA).toBe('#4d4d4d');
		expect(shaderSettings.idleColorB).toBe('#a8a8a8');
	});

	it('reset restores default aurora and dimmed monochrome idle colors', () => {
		shaderSettings.applyPreset('cyberpunk');
		shaderSettings.setIdleColorA('#112233');
		shaderSettings.setIdleColorB('#445566');
		shaderSettings.setExposure(2.2);

		shaderSettings.reset();
		expect(shaderSettings.selectedPreset).toBe('aurora');
		expect(shaderSettings.colorA).toBe('#10b981');
		expect(shaderSettings.colorB).toBe('#06b6d4');
		expect(shaderSettings.idleColorA).toBe('#4d4d4d');
		expect(shaderSettings.idleColorB).toBe('#a8a8a8');
		expect(shaderSettings.exposure).toBe(1.6);
	});
});
