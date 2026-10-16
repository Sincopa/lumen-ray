<script lang="ts">
	/**
	 * Digit strips, not NumberFlow. NumberFlow kept composited CSS animations
	 * that WebView2 never freed, and the connection clock leaked several MB a second.
	 * Each digit is a 0-9 strip moved with a transform. Nothing is allocated per tick.
	 */
	let { value, minDigits = 1 }: { value: number; minDigits?: number } =
		$props();

	let text = $derived(
		String(Math.max(0, Math.round(value))).padStart(minDigits, '0'),
	);
</script>

<span class="rolling-number">
	<span class="ghost">{text}</span>
	<span class="columns" aria-hidden="true">
		{#each text as digit, index (text.length - index)}
			<span class="column">
				<span
					class="strip"
					style:transform="translateY({-Number(digit) * 10}%)"
				>
					{#each { length: 10 }, n}
						<span>{n}</span>
					{/each}
				</span>
			</span>
		{/each}
	</span>
</span>

<style>
	.rolling-number {
		position: relative;
		display: inline-block;
		font-variant-numeric: tabular-nums;
		white-space: nowrap;
	}

	/* Real text gives the width, the baseline and the accessible value. */
	.ghost {
		color: transparent;
	}

	.columns {
		position: absolute;
		inset: 0;
		display: flex;
	}

	.column {
		flex: 1;
		height: 100%;
		overflow: hidden;
	}

	.strip {
		display: flex;
		flex-direction: column;
		height: 1000%;
		transition: transform 0.5s var(--ease-spring);
	}

	.strip > span {
		flex: 0 0 10%;
		text-align: center;
	}

	@media (prefers-reduced-motion: reduce) {
		.strip {
			transition: none;
		}
	}
</style>
