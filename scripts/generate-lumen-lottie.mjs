import { writeFile } from 'node:fs/promises';
import { simulate } from './lumen-lottie/motion.mjs';
import { segments } from './lumen-lottie/storyboard.mjs';
import { assemble, stats } from './lumen-lottie/lottie.mjs';
import { verify } from './lumen-lottie/verify.mjs';

/**
 * Builds src/assets/lumen-expressions.json — Lumen's whole repertoire as one Lottie
 * composition with a marker per segment.
 *
 *   storyboard.mjs  what the character does, as poses on a timeline
 *   motion.mjs      springs turn those poses into 60 fps motion (follow-through, overshoot)
 *   geometry.mjs    channels → the artwork's contours and layer transforms
 *   lottie.mjs      fits compact eased keyframes to the baked motion
 *   verify.mjs      replays the file like a player and checks it against the motion
 *
 * The resting pose reproduces src/assets/lumen-face.svg.
 */

const MAX_ERROR_PX = 1;

const baked = segments().map(({ timeline, ...segment }) => {
	const { frames, report } = simulate(timeline);
	if (!segment.loop && report.residual > 0.05)
		throw new Error(`${segment.name} has not settled (off by ${report.residual.toFixed(3)})`);
	if (segment.loop && report.closure > 0.01)
		throw new Error(`${segment.name} does not loop (open by ${report.closure.toFixed(3)})`);
	// A player stops one frame short of the end of a non-looping segment, so repeat the
	// settled last frame: the pose it holds is then exactly the one that was designed.
	if (!segment.loop) frames.push(frames.at(-1));
	return { ...segment, frames };
});

const composition = assemble(baked, { tolerance: 0.07 });

const report = verify(composition, baked);
const [worstName, worst] = Object.entries(report).reduce((a, b) => (b[1].worst > a[1].worst ? b : a));
if (worst.worst > MAX_ERROR_PX)
	throw new Error(
		`${worstName} deviates by ${worst.worst.toFixed(2)} px at frame ${worst.at} (limit ${MAX_ERROR_PX} px)`,
	);

const text = JSON.stringify(composition);
await writeFile(new URL('../src/assets/lumen-expressions.json', import.meta.url), text);

const frames = baked.reduce((total, segment) => total + segment.frames.length, 0);
console.log(
	[
		`${baked.length} segments, ${frames} frames → ${stats.keyframes} keyframes in ${stats.animated} animated properties`,
		`${(text.length / 1024).toFixed(0)} KB; worst deviation from the motion: ${worst.worst.toFixed(2)} px (${worstName})`,
	].join('\n'),
);
