<script lang="ts">
	import fi from 'flag-icons/flags/4x3/fi.svg';
	import nl from 'flag-icons/flags/4x3/nl.svg';
	import de from 'flag-icons/flags/4x3/de.svg';
	import se from 'flag-icons/flags/4x3/se.svg';
	import us from 'flag-icons/flags/4x3/us.svg';
	import gb from 'flag-icons/flags/4x3/gb.svg';
	import jp from 'flag-icons/flags/4x3/jp.svg';
	import ru from 'flag-icons/flags/4x3/ru.svg';
	import pl from 'flag-icons/flags/4x3/pl.svg';
	import ro from 'flag-icons/flags/4x3/ro.svg';
	import al from 'flag-icons/flags/4x3/al.svg';
	import ch from 'flag-icons/flags/4x3/ch.svg';
	import mk from 'flag-icons/flags/4x3/mk.svg';
	import my from 'flag-icons/flags/4x3/my.svg';
	import at from 'flag-icons/flags/4x3/at.svg';
	import lt from 'flag-icons/flags/4x3/lt.svg';
	import hk from 'flag-icons/flags/4x3/hk.svg';
	import fr from 'flag-icons/flags/4x3/fr.svg';
	import ca from 'flag-icons/flags/4x3/ca.svg';
	import au from 'flag-icons/flags/4x3/au.svg';
	import sg from 'flag-icons/flags/4x3/sg.svg';
	import kr from 'flag-icons/flags/4x3/kr.svg';
	import tr from 'flag-icons/flags/4x3/tr.svg';
	import ua from 'flag-icons/flags/4x3/ua.svg';
	import it from 'flag-icons/flags/4x3/it.svg';
	import es from 'flag-icons/flags/4x3/es.svg';
	import br from 'flag-icons/flags/4x3/br.svg';
	import in_ from 'flag-icons/flags/4x3/in.svg';
	import ie from 'flag-icons/flags/4x3/ie.svg';
	import cz from 'flag-icons/flags/4x3/cz.svg';
	import bg from 'flag-icons/flags/4x3/bg.svg';
	import hu from 'flag-icons/flags/4x3/hu.svg';
	import pt from 'flag-icons/flags/4x3/pt.svg';
	import ee from 'flag-icons/flags/4x3/ee.svg';
	import lv from 'flag-icons/flags/4x3/lv.svg';
	import no from 'flag-icons/flags/4x3/no.svg';
	import dk from 'flag-icons/flags/4x3/dk.svg';
	import il from 'flag-icons/flags/4x3/il.svg';
	import za from 'flag-icons/flags/4x3/za.svg';
	import ar from 'flag-icons/flags/4x3/ar.svg';
	import mx from 'flag-icons/flags/4x3/mx.svg';
	import cn from 'flag-icons/flags/4x3/cn.svg';
	import kz from 'flag-icons/flags/4x3/kz.svg';
	import ge from 'flag-icons/flags/4x3/ge.svg';
	import rs from 'flag-icons/flags/4x3/rs.svg';
	import sk from 'flag-icons/flags/4x3/sk.svg';
	import hr from 'flag-icons/flags/4x3/hr.svg';
	import md from 'flag-icons/flags/4x3/md.svg';
	import by from 'flag-icons/flags/4x3/by.svg';
	import am from 'flag-icons/flags/4x3/am.svg';
	import az from 'flag-icons/flags/4x3/az.svg';
	import tw from 'flag-icons/flags/4x3/tw.svg';
	import eu from 'flag-icons/flags/4x3/eu.svg';
	import fm from 'flag-icons/flags/4x3/fm.svg';

	const flags: Record<string, string> = {
		fi,
		nl,
		de,
		se,
		us,
		gb,
		jp,
		ru,
		pl,
		ro,
		al,
		ch,
		mk,
		my,
		at,
		lt,
		hk,
		fr,
		ca,
		au,
		sg,
		kr,
		tr,
		ua,
		it,
		es,
		br,
		in: in_,
		ie,
		cz,
		bg,
		hu,
		pt,
		ee,
		lv,
		no,
		dk,
		il,
		za,
		ar,
		mx,
		cn,
		kz,
		ge,
		rs,
		sk,
		hr,
		md,
		by,
		am,
		az,
		tw,
		eu,
		fm,
	};

	interface Props {
		countryCode?: string; // 'fi', 'nl', 'de', 'us', etc.
		size?: number;
	}

	let { countryCode = 'fi', size = 32 }: Props = $props();
</script>

<div class="flag-circle" style="width: {size}px; height: {size}px;">
	{#if countryCode && flags[countryCode.toLowerCase()]}
		<img class="flag-icon" src={flags[countryCode.toLowerCase()]} alt="" />
	{:else}
		<span aria-hidden="true">—</span>
	{/if}
</div>

<style>
	.flag-circle {
		border-radius: 50%;
		overflow: hidden;
		display: inline-flex;
		align-items: center;
		justify-content: center;
		flex-shrink: 0;
		background: rgba(255, 255, 255, 0.1);
		box-shadow: 0 1px 4px rgba(0, 0, 0, 0.25);
	}

	.flag-icon {
		width: 100% !important;
		height: 100% !important;
		line-height: 1em;
		object-fit: cover;
		background-position: center !important;
		border-radius: 50%;
		display: block;
	}
</style>
