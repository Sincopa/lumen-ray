<script lang="ts">
	import { onMount } from 'svelte';
	import { shaderSettings } from '../../lib/stores/shaderSettings.svelte';

	interface Props {
		active?: boolean;
	}

	let { active = false }: Props = $props();

	let canvas: HTMLCanvasElement;
	let gl: WebGL2RenderingContext | null = null;
	let running = true;
	let reducedMotion = false;
	let mounted = false;
	let lastFrame = 0;
	let frameId = 0;
	let program: WebGLProgram | null = null;
	let vao: WebGLVertexArrayObject | null = null;
	let uniforms: Record<string, WebGLUniformLocation> = {};
	let lost = false;

	const vsSource = `#version 300 es
    precision highp float;
    void main(){vec2 p=vec2(float((gl_VertexID<<1)&2),float(gl_VertexID&2));gl_Position=vec4(p*2.0-1.0,0.0,1.0);}`;

	const fsSource = `#version 300 es
precision highp float;
precision highp int;
out vec4 fragColor;
uniform vec2 uResolution;
uniform float uTime,uPlaying;
uniform int uNucleiCount,uRayCount,uDebug;
uniform vec4 uNuclei[6],uShape[6];
uniform vec4 uRays[64],uRayMeta[64],uRayRoots[64];
uniform float uDeformation,uContourThickness,uContourContrast,uArcCoverage;
uniform float uRayThickness,uRayCurvature,uRaySharpness,uRayGlow;
uniform float uRefraction,uDomainWarp,uDistortionScale;
uniform float uGlobalGlow,uAmbientGlow,uExposure,uContrast,uDarkLevel;
uniform vec3 uIdleColorA,uIdleColorB,uColorA,uColorB;
mat2 rotate(float a){float c=cos(a),s=sin(a);return mat2(c,-s,s,c);}
float gauss(float d,float w){float x=d/max(w,.0001);return exp2(-1.442695*x*x);}
float smoothUnion(float a,float b,float k){float h=max(k-abs(a-b),0.0)/k;return min(a,b)-h*h*k*.25;}
vec3 sourceColor(int i){
    float b=smoothstep(.10,.90,.5+.50*sin(uShape[i].z+uTime*.047));
    vec3 idleC=mix(uIdleColorA,uIdleColorB,b);
    vec3 activeC=mix(uColorA,uColorB,b);
    return mix(idleC,activeC,uPlaying);
}
// All membrane and scattering coordinates pass through the same flowing medium.
// Curl-like cross derivatives keep the folds coherent over several seconds.
vec2 warp(vec2 p){
    vec2 x=(p-vec2(0,.88))*uDistortionScale;
    float t=uTime;
    vec2 w=vec2(sin(x.y*5.1+t*.18)+.48*cos(x.x*3.7+x.y*2.4-t*.11),
                cos(x.x*4.8-t*.14)-.48*cos(x.x*3.7+x.y*2.4-t*.11));
    vec2 f=vec2(sin(x.y*10.1+w.x*.8-t*.13),cos(x.x*9.7+w.y*.6+t*.10));
    return p+uDomainWarp*uDeformation*(w*.043+f*.016)/(1.0+dot(x,x)*.8);
}
// Kept equivalent to surfaceDistance() in JS: ray roots are solved on this surface.
float surface(vec2 q,int i){
    vec2 v=rotate(uShape[i].x)*(q-uNuclei[i].xy);
    float r=uNuclei[i].z;
    v*=vec2(1.0/uShape[i].y,uShape[i].y);
    float phase=uShape[i].z;
    v.x+=r*.27*uDeformation*sin(v.y/max(r,.02)*2.5+phase+uTime*.13);
    float fold=sin(v.x*13.0+phase+uTime*.19)*cos(v.y*11.0-phase*.7-uTime*.14);
    float d=length(v)-r*(1.0+.22*uDeformation*fold);
    // Neighbour potentials bend each membrane while preserving its identity.
    float pressure=0.0;
    for(int j=0;j<6;j++){
        if(j>=uNucleiCount)break;
        if(j==i)continue;
        vec2 dv=q-uNuclei[j].xy;
        float rr=uNuclei[j].z;
        pressure+=rr*exp(-dot(dv,dv)/(rr*rr*.85+.0001))*uNuclei[j].w;
    }
    return d+-pressure*.14*uRefraction+pow(1.0-uNuclei[i].w,2.0)*.085;
}
float arcLight(vec2 q,int i){
    vec2 v=q-uNuclei[i].xy;
    float phase=uShape[i].z;
    float a=dot(v,vec2(cos(phase+uTime*.10),sin(phase+uTime*.10)))/max(uNuclei[i].z,.02);
    float modulation=.51+.37*a+.16*sin(q.x*13.0-q.y*9.0+phase-uTime*.15);
    return smoothstep(1.0-uArcCoverage-.24,1.0-uArcCoverage+.20,modulation);
}
void main(){
    float px=1.0/uResolution.x;
    vec2 p=vec2(gl_FragCoord.x/uResolution.x-.5,(uResolution.y-gl_FragCoord.y)/uResolution.x);
    float frameHeight=uResolution.y/uResolution.x;
    vec2 q=warp(p);
    float ds[6],aa[6],field=4.0;
    vec3 cs[6];
    vec2 center=vec2(0);float total=0.0;
    vec3 pigment=vec3(0);float pigmentWeight=0.0;
    for(int i=0;i<6;i++){
        if(i>=uNucleiCount)break;
        ds[i]=surface(q,i);
        aa[i]=max(fwidth(ds[i])*.65,px*.55);
        field=smoothUnion(field,ds[i],.050);
        cs[i]=sourceColor(i);
        float weight=uNuclei[i].w*uNuclei[i].z;
        center+=uNuclei[i].xy*weight;total+=weight;
        vec2 v=q-uNuclei[i].xy;
        float cw=uNuclei[i].w*exp(-dot(v,v)*7.0);
        pigment+=cs[i]*cw;pigmentWeight+=cw;
    }
    center/=max(total,.001);pigment/=max(pigmentWeight,.001);
    vec3 contours=vec3(0),glow=vec3(0),rays=vec3(0),volume=vec3(0);
    for(int i=0;i<6;i++){
        if(i>=uNucleiCount)break;
        float d=ds[i],life=uNuclei[i].w;
        float other=4.0;
        for(int j=0;j<6;j++){if(j>=uNucleiCount)break;if(i!=j)other=min(other,ds[j]);}
        float owner=exp(-max(d-field,0.0)*52.0);
        float front=.25+.75*smoothstep(-.085,.045,other);
        float arc=arcLight(q,i);
        float opacity=life*(.018+.982*arc);
        // Subpixel core + small halo: the diffuse light cannot erase the membrane.
        float width=max(.0018*uContourThickness,aa[i]);
        vec3 highlight=mix(cs[i],vec3(1.0,.91,.77),.38);
        float outerLine=owner*gauss(field,width);
        float innerLine=(1.0-owner)*gauss(d,width*1.4)*.46;
        float core=outerLine+innerLine;
        float fringe=owner*gauss(field+.0025,.010+width)+(1.0-owner)*gauss(d,.009)*.32;
        contours+=(highlight*core*.64+cs[i]*fringe*.31)*opacity*uContourContrast;
        glow+=cs[i]*(gauss(d,.020)*.19+gauss(d,.075)*.20)*life*(.24+.76*arc)*front;
        // Absorption inside each lens leaves room for internal membranes and folds.
        float inner=1.0-smoothstep(-.035,.025,d);
        float folds=.62+.24*sin(q.x*8.0+q.y*9.0+uShape[i].z+uTime*.11);
        volume+=cs[i]*(gauss(d+.018,.11)*.21+inner*.075*folds)*life;
    }
    // Each sheet originates at an actual membrane point, following its outward
    // normal. A shared domain bends both the emitting surface and the resulting light.
    // Unequal companion folds produce broad fans with a fine refracted edge.
    for(int j=0;j<64;j++){
        if(j>=uRayCount)break;
        vec4 ray=uRays[j],meta=uRayMeta[j],root=uRayRoots[j];
        if(ray.w<.001)continue;
        
        vec2 delta=q-root.xy;
        vec2 direction=root.zw,side=vec2(-direction.y,direction.x);
        float along=dot(delta,direction),len=ray.y;
        float s=clamp(along/max(len,.02),0.0,1.0);
        float tail=1.0-smoothstep(.10,1.0,s);
        float head=smoothstep(-.045,.020,along);
        float curl=uRayCurvature*meta.y*.12*along*along/(.22+abs(along));
        float across=dot(delta,side)-curl;
        float rootWidth=(.006+.045*ray.z)*uRayThickness;
        float width=max(rootWidth*pow(1.0-s,.78)*(1.0+.28*sin(s*3.0+meta.z)),px*.65);
        float spectralSide=meta.w>.5?1.0:-1.0;
        float edge=across-spectralSide*width*.58;
        float edgeWidth=max(width*(.10+.10/uRaySharpness),px*.7);
        float ribbon=gauss(across,width);
        float ridge=gauss(edge,edgeWidth);
        float split=gauss(across+spectralSide*width*.6,width*.35);
        float energy=ray.w*head*tail*(.16+.84*smoothstep(-.045,.045,field));
        vec3 idleC=mix(uIdleColorA,uIdleColorB,meta.x);
        vec3 activeC=mix(uColorA,uColorB,meta.x);
        vec3 c=mix(idleC,activeC,uPlaying);
        rays+=(c*(ribbon*.39+split*.065)+mix(c,vec3(1.0,.9,.80),.20)*ridge*.27)*energy;
        glow+=c*(gauss(across,width*2.3)*.18+gauss(across,width*4.5)*.025)*energy*uRayGlow;
    }
    float h=uResolution.y/uResolution.x;
    float lowerFade=1.0;
    vec2 air=(p-center)*vec2(.65,0.42);
    float falloff=exp(-dot(air,air)*0.65);
    float activeFill=mix(0.18,0.35,uPlaying);
    // Wide screen-fill: a very soft base tint that covers the full canvas
    // so the bottom doesn't appear dead black.
    vec2 airWide=(p-vec2(0.0,h*0.5))*vec2(0.30,0.18);
    float wideFill=exp(-dot(airWide,airWide)*0.45)*0.55;
    vec3 atmosphere=pigment*(falloff*0.75+wideFill+activeFill)*(mix(uAmbientGlow*0.75,uAmbientGlow,uPlaying))*(.35+.65*smoothstep(-.48,.33,p.x));
    vec3 color=(rays+(glow+volume+atmosphere)*uGlobalGlow)*lowerFade;
    if(uDebug==1)color=vec3(1.0-smoothstep(-.025,.025,field))*vec3(.55,.36,.95);
    if(uDebug==2)color=contours;
    if(uDebug==3)color=rays;
    if(uDebug==4)color=(glow+volume+atmosphere)*uGlobalGlow;
    if(uDebug==5){
        vec2 grid=abs(fract(q*12.0-.5)-.5)/max(fwidth(q*12.0),vec2(.0001));
        color=vec3(.04,.02,.1)+vec3(.40,.22,.8)*(1.0-min(min(grid.x,grid.y),1.0));
    }
    vec3 radiance=max(color,vec3(0))*uExposure*mix(0.75,1.16,uPlaying);
    float peak=max(radiance.r,max(radiance.g,radiance.b));
    color=radiance*((1.0-exp(-peak))/max(peak,.0001));
    if(uDebug==0)color=1.0-(1.0-color)*exp(-contours*uExposure*lowerFade);
    color=pow(max(color,vec3(0)),vec3(uContrast));
    color=max(vec3(0),color-vec3(uDarkLevel*.018*(1.0-uPlaying*0.5)));
    float dither=fract(52.9829189*fract(dot(gl_FragCoord.xy,vec2(.06711056,.00583715))))-.5;
    fragColor=vec4(clamp(color+dither/255.0,0.0,1.0),1.0);
}`;

	const defaults = {
		nucleiCount: 4,
		nucleiSize: 1.22,
		nucleiSpread: 1.0,
		deformation: 1.15,
		nucleiSpeed: 1.0,
		contourThickness: 1.15,
		contourContrast: 1.25,
		arcCoverage: 0.51,
		rayCount: 24,
		rayMinLength: 0.3,
		rayMaxLength: 1.55,
		rayThickness: 1.0,
		rayCurvature: 0.55,
		raySharpness: 1.1,
		rayGlow: 1.25,
		rayFadeSpeed: 0.85,
		rayMovementSpeed: 1.0,
		refraction: 1.0,
		domainWarp: 1.1,
		distortionScale: 1.15,
		globalGlow: 1.25,
		ambientGlow: 0.65,
		exposure: 1.6,
		contrast: 1.07,
		darkLevel: 0.32,
		animationSpeed: 0.8,
		idleColorA: '#4d4d4d',
		idleColorB: '#a8a8a8',
		colorA: '#10b981',
		colorB: '#06b6d4',
		transition: 0.9,
		seed: 731,
		debug: 0,
	};

	const config = { ...defaults };
	let lastTime = 0,
		time = 0,
		centerTime = 0,
		rayTime = 0,
		fadeTime = 0,
		colorMix = 0;
	let transitionFrom = 0,
		transitionElapsed = 0,
		transitionTarget = 0;
	let nuclei: ReturnType<typeof makeScene>['nuclei'] = [],
		filaments: ReturnType<typeof makeScene>['filaments'] = [];
	// Upload only active sources and rays; GLSL ignores the unused array slots.
	const srcData = new Float32Array(config.nucleiCount * 4),
		shapeData = new Float32Array(config.nucleiCount * 4),
		rayData = new Float32Array(config.rayCount * 4),
		metaData = new Float32Array(config.rayCount * 4),
		rootData = new Float32Array(config.rayCount * 4);
	const pressureScratch = new Float64Array(6),
		rotationData = new Float64Array(12);
	const centerDistances = new Float64Array(6);
	const sourceTints = new Float64Array(6);
	const arcDirections = new Float64Array(12);

	// These settings stay constant for the lifetime of a WebGL program.
	const staticUniformKeys = [
		'deformation',
		'contourThickness',
		'contourContrast',
		'arcCoverage',
		'rayThickness',
		'rayCurvature',
		'raySharpness',
		'rayGlow',
		'refraction',
		'domainWarp',
		'distortionScale',
		'globalGlow',
		'ambientGlow',
		'contrast',
		'darkLevel',
	] as const;

	const colorCurrent = {
		idleColorA: hex(shaderSettings.idleColorA),
		idleColorB: hex(shaderSettings.idleColorB),
		colorA: hex(shaderSettings.colorA),
		colorB: hex(shaderSettings.colorB),
	};

	const colorUniforms = {
		idleColorA: 'uIdleColorA',
		idleColorB: 'uIdleColorB',
		colorA: 'uColorA',
		colorB: 'uColorB',
	} as const;
	const colorKeys = Object.keys(colorUniforms) as Array<
		keyof typeof colorUniforms
	>;
	const colorHex = {
		idleColorA: shaderSettings.idleColorA,
		idleColorB: shaderSettings.idleColorB,
		colorA: shaderSettings.colorA,
		colorB: shaderSettings.colorB,
	};
	const colorTargets = {
		idleColorA: [...colorCurrent.idleColorA],
		idleColorB: [...colorCurrent.idleColorB],
		colorA: [...colorCurrent.colorA],
		colorB: [...colorCurrent.colorB],
	};

	function hex(c: string) {
		return [
			parseInt(c.slice(1, 3), 16) / 255,
			parseInt(c.slice(3, 5), 16) / 255,
			parseInt(c.slice(5, 7), 16) / 255,
		];
	}
	function smooth(a: number, b: number, x: number) {
		const t = Math.max(0, Math.min(1, (x - a) / (b - a)));
		return t * t * (3 - 2 * t);
	}
	function lerp(a: number, b: number, t: number) {
		return a + (b - a) * t;
	}
	function rng(seed: number) {
		let x = seed >>> 0;
		return () => {
			x += 0x6d2b79f5;
			let t = x;
			t = Math.imul(t ^ (t >>> 15), t | 1);
			t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
			return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
		};
	}

	function makeScene(seed: number) {
		const random = rng(seed),
			r = (a: number, b: number) => lerp(a, b, random());
		const anchors = [
			[-0.135, -0.09],
			[0.11, 0.07],
			[-0.07, 0.19],
			[0.16, -0.15],
			[-0.19, 0.08],
			[0.015, -0.23],
		];
		const radii = [0.172, 0.177, 0.112, 0.117, 0.1, 0.11];
		const ns = anchors.map((a, i) => ({
			x: a[0] + r(-0.012, 0.012),
			y: a[1] + r(-0.012, 0.012),
			radius: radii[i] * r(0.93, 1.07),
			phase: r(0, Math.PI * 2),
			phase2: r(0, Math.PI * 2),
			speed: r(0.7, 1.3),
			stretch: r(0.84, 1.16),
			angle: r(-1, 1),
			life: r(0.1, 2.8),
		}));
		// Stratified, unequal emission sites, not a uniformly spaced sunburst.
		const rs = Array.from({ length: 64 }, (_, j) => ({
			angle: ((j * 2.39996323 + r(-0.38, 0.38)) % (2 * Math.PI)) - Math.PI,
			length: Math.pow(r(0, 1), 1.1),
			width: j % 4 === 0 ? r(0.1, 0.3) : r(0.65, 1.45),
			brightness: r(0.6, 1.25),
			bend: r(-0.65, 0.65),
			phase: r(0, Math.PI * 2),
			speed: r(0.55, 1.25),
			spread: r(0, 1),
		}));
		return { nuclei: ns, filaments: rs };
	}

	function compileShader(type: number, source: string) {
		if (!gl) return null;
		const s = gl.createShader(type)!;
		gl.shaderSource(s, source.trim());
		gl.compileShader(s);
		if (!gl.getShaderParameter(s, gl.COMPILE_STATUS)) {
			console.error(gl.getShaderInfoLog(s));
			gl.deleteShader(s);
			return null;
		}
		return s;
	}

	function initGL() {
		if (!canvas) return false;
		gl = canvas.getContext('webgl2', {
			alpha: false,
			antialias: false,
			depth: false,
			stencil: false,
			powerPreference: 'low-power',
		});
		if (!gl) return false;

		const vs = compileShader(gl.VERTEX_SHADER, vsSource);
		const fs = compileShader(gl.FRAGMENT_SHADER, fsSource);
		if (!vs || !fs) return false;

		program = gl.createProgram()!;
		gl.attachShader(program, vs);
		gl.attachShader(program, fs);
		gl.linkProgram(program);
		gl.deleteShader(vs);
		gl.deleteShader(fs);

		if (!gl.getProgramParameter(program, gl.LINK_STATUS)) {
			console.error(gl.getProgramInfoLog(program));
			return false;
		}

		gl.useProgram(program);
		vao = gl.createVertexArray();
		gl.bindVertexArray(vao);

		uniforms = {};
		const count = gl.getProgramParameter(program, gl.ACTIVE_UNIFORMS);
		for (let i = 0; i < count; i++) {
			const info = gl.getActiveUniform(program, i)!;
			uniforms[info.name.replace('[0]', '')] = gl.getUniformLocation(
				program,
				info.name,
			)!;
		}

		gl.uniform1i(uniforms.uNucleiCount, config.nucleiCount);
		gl.uniform1i(uniforms.uRayCount, config.rayCount);
		gl.uniform1i(uniforms.uDebug, config.debug);
		for (const key of staticUniformKeys) {
			setFloat('u' + key[0].toUpperCase() + key.slice(1), config[key]);
		}

		gl.disable(gl.DEPTH_TEST);
		gl.disable(gl.BLEND);
		resize();
		return true;
	}

	function resize() {
		if (!gl || !canvas || lost) return;
		const dpr = Math.min(window.devicePixelRatio || 1, 1.5);
		const w = Math.round(canvas.clientWidth * dpr);
		const h = Math.round(canvas.clientHeight * dpr);
		if (canvas.width !== w || canvas.height !== h) {
			canvas.width = w;
			canvas.height = h;
			gl.viewport(0, 0, w, h);
		}
	}

	// Analytic optical surface in the same coordinates as GLSL surface().
	function surfaceDistance(x: number, y: number, i: number) {
		const k = i * 4,
			cs = rotationData[i * 2],
			sn = rotationData[i * 2 + 1];
		const dx = x - srcData[k],
			dy = y - srcData[k + 1],
			r = srcData[k + 2];
		let vx = (cs * dx + sn * dy) / shapeData[k + 1];
		const vy = (-sn * dx + cs * dy) * shapeData[k + 1],
			phase = shapeData[k + 2];
		vx +=
			r *
			0.27 *
			config.deformation *
			Math.sin((vy / Math.max(r, 0.02)) * 2.5 + phase + time * 0.13);
		const fold =
			Math.sin(vx * 13 + phase + time * 0.19) *
			Math.cos(vy * 11 - phase * 0.7 - time * 0.14);
		let pressure = 0;
		for (let j = 0; j < config.nucleiCount; j++) {
			if (i === j) continue;
			const o = j * 4,
				ex = x - srcData[o],
				ey = y - srcData[o + 1],
				rr = srcData[o + 2];
			pressure +=
				rr *
				Math.exp(-(ex * ex + ey * ey) / (rr * rr * 0.85 + 0.0001)) *
				srcData[o + 3];
		}
		return (
			Math.hypot(vx, vy) -
			r * (1 + 0.22 * config.deformation * fold) -
			pressure * 0.14 * config.refraction +
			(1 - srcData[k + 3]) ** 2 * 0.085
		);
	}
	function arcLight(x: number, y: number, i: number) {
		const k = i * 4,
			phase = shapeData[k + 2];
		const direction =
			((x - srcData[k]) * arcDirections[i * 2] +
				(y - srcData[k + 1]) * arcDirections[i * 2 + 1]) /
			Math.max(srcData[k + 2], 0.02);
		const light =
			0.51 +
			0.37 * direction +
			0.16 * Math.sin(x * 13 - y * 9 + phase - time * 0.15);
		return smooth(
			1 - config.arcCoverage - 0.24,
			1 - config.arcCoverage + 0.2,
			light,
		);
	}
	function fieldDistance(x: number, y: number) {
		// Evaluate the neighbour potential once per point, shared by all surfaces.
		let pressure = 0;
		for (let i = 0; i < config.nucleiCount; i++) {
			const k = i * 4,
				x0 = x - srcData[k],
				y0 = y - srcData[k + 1],
				r = srcData[k + 2];
			pressureScratch[i] =
				r *
				Math.exp(-(x0 * x0 + y0 * y0) / (r * r * 0.85 + 0.0001)) *
				srcData[k + 3];
			pressure += pressureScratch[i];
		}
		let field = 4;
		for (let i = 0; i < config.nucleiCount; i++) {
			const k = i * 4,
				cs = rotationData[i * 2],
				sn = rotationData[i * 2 + 1];
			const dx = x - srcData[k],
				dy = y - srcData[k + 1],
				r = srcData[k + 2];
			let vx = (cs * dx + sn * dy) / shapeData[k + 1];
			const vy = (-sn * dx + cs * dy) * shapeData[k + 1],
				phase = shapeData[k + 2];
			vx +=
				r *
				0.27 *
				config.deformation *
				Math.sin((vy / Math.max(r, 0.02)) * 2.5 + phase + time * 0.13);
			const fold =
				Math.sin(vx * 13 + phase + time * 0.19) *
				Math.cos(vy * 11 - phase * 0.7 - time * 0.14);
			const d =
				Math.hypot(vx, vy) -
				r * (1 + 0.22 * config.deformation * fold) -
				(pressure - pressureScratch[i]) * 0.14 * config.refraction +
				(1 - srcData[k + 3]) ** 2 * 0.085;
			const h = Math.max(0.05 - Math.abs(field - d), 0) / 0.05;
			field = Math.min(field, d) - h * h * 0.0125;
		}
		return field;
	}
	function updateSources() {
		const t = centerTime,
			rt = rayTime,
			ft = fadeTime;
		const cx = 0.025 * Math.sin(t * 0.17) + 0.02 * Math.sin(t * 0.083),
			cy = 0.88 + 0.045 * Math.sin(t * 0.14 + 0.3);
		const turn = 0.24 * Math.sin(t * 0.12),
			cs = Math.cos(turn),
			sn = Math.sin(turn);
		for (let i = 0; i < config.nucleiCount; i++) {
			const n = nuclei[i],
				p = n.phase,
				v = t * n.speed;
			const life =
				i < 2 ? 1 : smooth(-0.98, -0.16, Math.sin(v * 0.21 + n.life));
			const radius =
				n.radius *
				config.nucleiSize *
				(0.93 + 0.07 * Math.sin(v * 0.38 + p)) *
				(0.62 + 0.38 * life);
			const ox =
				(n.x +
					0.038 * Math.sin(v * 0.29 + p) +
					0.016 * Math.sin(v * 0.47 + n.phase2)) *
				config.nucleiSpread;
			const oy =
				(n.y + 0.034 * Math.cos(v * 0.26 + n.phase2)) * config.nucleiSpread;
			const offset = i * 4;
			srcData[offset] = cx + cs * ox - sn * oy;
			srcData[offset + 1] = cy + sn * ox + cs * oy;
			srcData[offset + 2] = radius;
			srcData[offset + 3] = life;
			shapeData[offset] = n.angle + 0.38 * Math.sin(v * 0.24 + p);
			shapeData[offset + 1] = n.stretch + 0.13 * Math.sin(v * 0.27 + p);
			shapeData[offset + 2] = p + 0.16 * Math.sin(v * 0.17);
			shapeData[offset + 3] = i * 0.7;
		}
		for (let i = 0; i < config.nucleiCount; i++) {
			rotationData[i * 2] = Math.cos(shapeData[i * 4]);
			rotationData[i * 2 + 1] = Math.sin(shapeData[i * 4]);
		}
		// Each source is shared by several rays. Cache frame-invariant values.
		for (let i = 0; i < config.nucleiCount; i++) {
			const offset = i * 4;
			const phase = shapeData[offset + 2];
			centerDistances[i] = surfaceDistance(
				srcData[offset],
				srcData[offset + 1],
				i,
			);
			sourceTints[i] = smooth(
				0.1,
				0.9,
				0.5 + 0.5 * Math.sin(phase + time * 0.047),
			);
			arcDirections[i * 2] = Math.cos(phase + time * 0.1);
			arcDirections[i * 2 + 1] = Math.sin(phase + time * 0.1);
		}
		for (let j = 0; j < config.rayCount; j++) {
			const r = filaments[j],
				i = j % config.nucleiCount,
				k = i * 4;
			const life = smooth(-0.86, 0.3, Math.sin(ft * 0.28 * r.speed + r.phase));
			let angle =
				r.angle +
				0.24 * Math.sin(rt * 0.25 * r.speed + r.phase) +
				0.07 * Math.sin(rt * 0.41 + r.phase * 2);
			let len =
					lerp(config.rayMinLength, config.rayMaxLength, r.length) *
					(0.65 + 0.35 * life),
				width = r.width;
			let strength = r.brightness * life * srcData[k + 3];

			const dx = Math.cos(angle),
				dy = Math.sin(angle);
			let lo = 0,
				hi = srcData[k + 2] * 2.8 + 0.06;
			strength *= smooth(0, 0.025, -centerDistances[i]);
			// Follow one labelled membrane through merges/splits. 16 iterations
			// keep root quantization well below a display pixel even at DPR 2.
			for (let n = 0; n < 16; n++) {
				const mid = (lo + hi) * 0.5;
				if (
					surfaceDistance(srcData[k] + dx * mid, srcData[k + 1] + dy * mid, i) >
					0
				)
					hi = mid;
				else lo = mid;
			}
			const distance = (lo + hi) * 0.5,
				x = srcData[k] + dx * distance,
				y = srcData[k + 1] + dy * distance;
			const e = 0.0005,
				gx = surfaceDistance(x + e, y, i) - surfaceDistance(x - e, y, i),
				gy = surfaceDistance(x, y + e, i) - surfaceDistance(x, y - e, i);
			const norm = Math.hypot(gx, gy) || 1;
			strength *= smooth(0.06, 0.32, norm / (2 * e));
			let light = 0,
				tint = 0,
				weight = 0;
			const field = fieldDistance(x, y);
			for (let n = 0; n < config.nucleiCount; n++) {
				const w =
					Math.exp(-Math.max(surfaceDistance(x, y, n) - field, 0) * 32) *
					srcData[n * 4 + 3];
				light += arcLight(x, y, n) * w;
				tint += sourceTints[n] * w;
				weight += w;
			}
			const emission = 0.18 + (0.82 * light) / Math.max(weight, 0.001);
			const energy = strength * emission;
			const offset = j * 4;
			rayData[offset] = angle;
			rayData[offset + 1] = len;
			rayData[offset + 2] = width;
			rayData[offset + 3] = energy;
			metaData[offset] = tint / Math.max(weight, 0.001);
			metaData[offset + 1] = r.bend + 0.16 * Math.sin(rt * 0.19 + r.phase);
			metaData[offset + 2] = r.phase;
			metaData[offset + 3] = r.spread;
			rootData[offset] = x;
			rootData[offset + 1] = y;
			rootData[offset + 2] = gx / norm;
			rootData[offset + 3] = gy / norm;
		}
	}

	function setFloat(name: string, val: number) {
		if (gl && uniforms[name] !== undefined) gl.uniform1f(uniforms[name], val);
	}

	function render(now: number) {
		frameId = 0;
		if (
			!running ||
			lost ||
			!gl ||
			!program ||
			document.hidden ||
			!shaderSettings.enabled
		)
			return;
		if (now - lastFrame < 1000 / 30 && !reducedMotion) {
			frameId = requestAnimationFrame(render);
			return;
		}
		lastFrame = now;
		const dt = lastTime ? Math.min((now - lastTime) / 1000, 0.06) : 0;
		lastTime = now;

		const step = dt * config.animationSpeed * (1 + colorMix * 0.25);
		time += step;
		centerTime += step * config.nucleiSpeed;
		rayTime += step * config.rayMovementSpeed;
		fadeTime += step * config.rayFadeSpeed;

		if (transitionElapsed < config.transition) {
			transitionElapsed = Math.min(config.transition, transitionElapsed + dt);
			colorMix = lerp(
				transitionFrom,
				transitionTarget,
				smooth(0, config.transition, transitionElapsed),
			);
		}

		updateSources();

		gl.useProgram(program);
		gl.bindVertexArray(vao);
		gl.uniform2f(uniforms.uResolution, canvas.width, canvas.height);
		setFloat('uTime', time);
		setFloat('uPlaying', colorMix);
		gl.uniform4fv(uniforms.uNuclei, srcData);
		gl.uniform4fv(uniforms.uShape, shapeData);
		gl.uniform4fv(uniforms.uRays, rayData);
		gl.uniform4fv(uniforms.uRayMeta, metaData);
		gl.uniform4fv(uniforms.uRayRoots, rootData);

		// Apply dynamically configured exposure from shaderSettings
		setFloat('uExposure', config.exposure * (shaderSettings.exposure / 1.6));

		// Parse a target only when its setting changes; keep the same easing.
		const mixVal = reducedMotion ? 1 : 1 - Math.exp(-dt * 6);
		for (const key of colorKeys) {
			const hexValue = shaderSettings[key];
			if (colorHex[key] !== hexValue) {
				colorHex[key] = hexValue;
				colorTargets[key] = hex(hexValue);
			}
			const current = colorCurrent[key];
			const target = colorTargets[key];
			for (let i = 0; i < 3; i++) {
				current[i] = lerp(current[i], target[i], mixVal);
			}
			gl.uniform3fv(uniforms[colorUniforms[key]], current);
		}

		gl.drawArrays(gl.TRIANGLES, 0, 3);
		if (!reducedMotion) frameId = requestAnimationFrame(render);
	}

	function resume() {
		cancelAnimationFrame(frameId);
		lastTime = 0;
		lastFrame = 0;
		if (mounted && !document.hidden && shaderSettings.enabled && !lost)
			frameId = requestAnimationFrame(render);
	}

	$effect(() => {
		transitionFrom = colorMix;
		transitionTarget = active ? 1.0 : 0.0;
		transitionElapsed = 0;
		if (reducedMotion) {
			colorMix = transitionTarget;
			transitionElapsed = config.transition;
		}
		resume();
	});
	$effect(() => {
		void shaderSettings.enabled;
		void shaderSettings.exposure;
		void shaderSettings.idleColorA;
		void shaderSettings.idleColorB;
		void shaderSettings.colorA;
		void shaderSettings.colorB;
		resume();
	});

	onMount(() => {
		mounted = true;
		const media = window.matchMedia('(prefers-reduced-motion: reduce)');
		reducedMotion = media.matches;
		({ nuclei, filaments } = makeScene(config.seed));
		if (initGL()) resume();
		const handleResize = () => {
			resize();
			resume();
		};
		const motionChange = () => {
			reducedMotion = media.matches;
			resume();
		};
		const contextLost = (event: Event) => {
			event.preventDefault();
			lost = true;
			cancelAnimationFrame(frameId);
		};
		const contextRestored = () => {
			lost = false;
			if (initGL()) resume();
		};
		window.addEventListener('resize', handleResize);
		document.addEventListener('visibilitychange', resume);
		media.addEventListener('change', motionChange);
		canvas.addEventListener('webglcontextlost', contextLost);
		canvas.addEventListener('webglcontextrestored', contextRestored);
		return () => {
			mounted = false;
			running = false;
			cancelAnimationFrame(frameId);
			window.removeEventListener('resize', handleResize);
			document.removeEventListener('visibilitychange', resume);
			media.removeEventListener('change', motionChange);
			canvas.removeEventListener('webglcontextlost', contextLost);
			canvas.removeEventListener('webglcontextrestored', contextRestored);
			if (gl) {
				gl.deleteVertexArray(vao);
				gl.deleteProgram(program);
			}
		};
	});
</script>

<canvas
	bind:this={canvas}
	class="shader-canvas"
	class:disabled={!shaderSettings.enabled}
	aria-hidden="true"
></canvas>

<style>
	.shader-canvas.disabled {
		visibility: hidden;
	}
	.shader-canvas {
		position: absolute;
		inset: 0;
		width: 100%;
		height: 100%;
		pointer-events: none;
		z-index: 0;
		background: #08080b;
		display: block;
		border-radius: var(--radius-window);
	}
</style>
