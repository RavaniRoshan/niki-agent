import React, { useEffect, useRef } from 'react';
import * as THREE from 'three';

interface PixelatedShaderCanvasProps {
  isDark: boolean;
  className?: string;
}

const VERTEX_SHADER = `
  varying vec2 vUv;
  void main() {
    vUv = uv;
    gl_Position = vec4(position.xy, 0.0, 1.0);
  }
`;

/**
 * Pixelated Atmospheric Plume Fragment Shader
 * Echoes the centered vertical+horizontal luminous plume composition of the hero reference,
 * quantized into crisp shader pixels with 4x4 Bayer ordered dithering and strictly mapped
 * to our OKLCH dark slate-navy and icy-white light palette.
 */
const FRAGMENT_SHADER = `
  precision highp float;

  uniform float uTime;
  uniform vec2 uResolution;
  uniform vec2 uPointer;
  uniform float uIsDark;

  varying vec2 vUv;

  float hash21(vec2 p) {
    p = fract(p * vec2(123.34, 456.21));
    p += dot(p, p + 45.32);
    return fract(p.x * p.y);
  }

  float vnoise(vec2 p) {
    vec2 i = floor(p);
    vec2 f = fract(p);
    vec2 u = f * f * (3.0 - 2.0 * f);
    float a = hash21(i);
    float b = hash21(i + vec2(1.0, 0.0));
    float c = hash21(i + vec2(0.0, 1.0));
    float d = hash21(i + vec2(1.0, 1.0));
    return mix(mix(a, b, u.x), mix(c, d, u.x), u.y);
  }

  float fbm(vec2 p) {
    float val = 0.0;
    float amp = 0.5;
    mat2 rot = mat2(0.8, 0.6, -0.6, 0.8);
    for (int i = 0; i < 4; i++) {
      val += amp * vnoise(p);
      p = rot * p * 2.02 + vec2(0.35, -0.25);
      amp *= 0.5;
    }
    return val;
  }

  // 4x4 Bayer ordered dither threshold in [0, 1]
  float bayer4(vec2 cell) {
    vec2 p = mod(floor(cell), 4.0);
    int x = int(p.x);
    int y = int(p.y);
    int idx = y * 4 + x;
    if (idx == 0) return 0.0 / 16.0;
    if (idx == 1) return 8.0 / 16.0;
    if (idx == 2) return 2.0 / 16.0;
    if (idx == 3) return 10.0 / 16.0;
    if (idx == 4) return 12.0 / 16.0;
    if (idx == 5) return 4.0 / 16.0;
    if (idx == 6) return 14.0 / 16.0;
    if (idx == 7) return 6.0 / 16.0;
    if (idx == 8) return 3.0 / 16.0;
    if (idx == 9) return 11.0 / 16.0;
    if (idx == 10) return 1.0 / 16.0;
    if (idx == 11) return 9.0 / 16.0;
    if (idx == 12) return 15.0 / 16.0;
    if (idx == 13) return 7.0 / 16.0;
    if (idx == 14) return 13.0 / 16.0;
    return 5.0 / 16.0;
  }

  void main() {
    float pixelSize = 8.0;
    vec2 fragCoord = vUv * uResolution;
    vec2 cell = floor(fragCoord / pixelSize);
    vec2 localUv = fract(fragCoord / pixelSize);
    vec2 qUv = (cell * pixelSize + pixelSize * 0.5) / uResolution;

    float aspect = uResolution.x / max(uResolution.y, 1.0);
    vec2 p = vec2((qUv.x - 0.5) * aspect, qUv.y - 0.52);

    // Subtle pointer drift
    vec2 ptr = (uPointer - 0.5) * vec2(0.18, 0.12);
    vec2 sp = p - ptr;

    float t = uTime * 0.22;

    // Domain-warped organic flow
    vec2 warp = vec2(
      fbm(sp * 2.1 + vec2(t * 0.7, -t * 0.45)),
      fbm(sp * 2.1 + vec2(-t * 0.5, t * 0.65) + vec2(3.7, 1.9))
    );
    vec2 wp = sp + (warp - 0.5) * 0.42;

    // Horizontal atmospheric band + vertical rising plume silhouette
    float horizBand = exp(-pow(wp.x * 1.08, 2.0) - pow((wp.y + 0.03) * 3.1, 2.0));
    float vertPlume = exp(-pow((wp.x + 0.04) * 2.85, 2.0) - pow((wp.y - 0.08) * 1.42, 2.0));
    float lowerGlow = exp(-pow(wp.x * 1.55, 2.0) - pow((wp.y + 0.22) * 2.6, 2.0));

    float ridge = fbm(wp * 3.4 - vec2(t * 0.4, t * 0.85));
    float field = (horizBand * 0.78 + vertPlume * 0.72 + lowerGlow * 0.26) * (0.72 + 0.52 * ridge);

    // Soft radial vignette so edges settle cleanly into page background
    float edgeMask = 1.0 - smoothstep(0.32, 0.92, length(vec2(p.x * 0.68, p.y * 1.15)));
    field *= edgeMask;

    // Ordered dithering + stepped tonal quantization for unmistakable pixel-shader character
    float dither = (bayer4(cell) - 0.5) * 0.095;
    float stepped = clamp(floor((field + dither) * 7.0) / 7.0, 0.0, 1.0);

    // Subtle per-pixel cell bevel (darkens 1px cell seam softly in active region)
    float cellGrid = step(0.12, localUv.x) * step(0.12, localUv.y);
    float gridAtten = mix(1.0, 0.88 + 0.12 * cellGrid, smoothstep(0.06, 0.45, stepped));

    // Exact OKLCH-matched sRGB palette stops
    // Dark mode: pure black #000000 -> obsidian indigo -> primary royal cobalt & violet -> secondary sky-cyan
    vec3 darkBg   = vec3(0.0, 0.0, 0.0);
    vec3 darkLow  = vec3(0.075, 0.078, 0.210);
    vec3 darkMid  = vec3(0.220, 0.295, 0.940);
    vec3 darkHigh = vec3(0.305, 0.675, 0.960);

    vec3 darkCol = mix(darkBg, darkLow, smoothstep(0.02, 0.30, stepped));
    darkCol = mix(darkCol, darkMid, smoothstep(0.26, 0.66, stepped));
    darkCol = mix(darkCol, darkHigh, smoothstep(0.64, 1.0, stepped) * 0.68);

    // Light mode: crisp off-white #fbfbfd -> soft sky-periwinkle -> secondary sky-cyan -> primary cobalt
    vec3 lightBg   = vec3(0.988, 0.988, 0.994);
    vec3 lightLow  = vec3(0.890, 0.935, 0.995);
    vec3 lightMid  = vec3(0.460, 0.755, 0.980);
    vec3 lightHigh = vec3(0.275, 0.365, 0.935);

    vec3 lightCol = mix(lightBg, lightLow, smoothstep(0.04, 0.34, stepped));
    lightCol = mix(lightCol, lightMid, smoothstep(0.32, 0.72, stepped));
    lightCol = mix(lightCol, lightHigh, smoothstep(0.70, 1.0, stepped) * 0.42);

    vec3 finalCol = mix(lightCol, darkCol, uIsDark) * gridAtten;

    // Gentle top/bottom fade to seamlessly blend with navbar & next section
    float verticalFade = smoothstep(0.0, 0.14, vUv.y) * smoothstep(1.0, 0.84, vUv.y);
    vec3 baseBg = mix(lightBg, darkBg, uIsDark);
    finalCol = mix(baseBg, finalCol, verticalFade);

    gl_FragColor = vec4(finalCol, 1.0);
  }
`;

export function PixelatedShaderCanvas({ isDark, className = '' }: PixelatedShaderCanvasProps) {
  const mountRef = useRef<HTMLDivElement | null>(null);
  const isDarkRef = useRef(isDark);
  const uniformsRef = useRef<{
    uTime: { value: number };
    uResolution: { value: THREE.Vector2 };
    uPointer: { value: THREE.Vector2 };
    uIsDark: { value: number };
  } | null>(null);
  const renderImmediateRef = useRef<(() => void) | null>(null);

  useEffect(() => {
    isDarkRef.current = isDark;
    if (uniformsRef.current) {
      uniformsRef.current.uIsDark.value = isDark ? 1.0 : 0.0;
    }
    renderImmediateRef.current?.();
  }, [isDark]);

  useEffect(() => {
    const container = mountRef.current;
    if (!container) return;

    // Probe software WebGL (SwiftShader / llvmpipe) per threejs skill directive
    const isSoftwareGl = (() => {
      try {
        const probeCanvas = document.createElement('canvas');
        const gl = probeCanvas.getContext('webgl2') || probeCanvas.getContext('webgl');
        if (!gl) return true;
        const ext = gl.getExtension('WEBGL_debug_renderer_info');
        const param = ext ? ext.UNMASKED_RENDERER_WEBGL : gl.RENDERER;
        const gpu = String(gl.getParameter(param) || '');
        gl.getExtension('WEBGL_lose_context')?.loseContext();
        return /SwiftShader|llvmpipe|softpipe|Software|Basic Render/i.test(gpu);
      } catch {
        return true;
      }
    })();

    let renderer: THREE.WebGLRenderer;
    try {
      renderer = new THREE.WebGLRenderer({
        antialias: false,
        alpha: false,
        powerPreference: 'high-performance',
      });
    } catch {
      return;
    }

    renderer.outputColorSpace = THREE.SRGBColorSpace;
    renderer.setPixelRatio(isSoftwareGl ? 1 : Math.min(window.devicePixelRatio || 1, 1.75));
    renderer.domElement.setAttribute('data-testid', 'hero-pixelated-shader');
    renderer.domElement.setAttribute('aria-hidden', 'true');
    renderer.domElement.style.display = 'block';
    renderer.domElement.style.width = '100%';
    renderer.domElement.style.height = '100%';
    container.replaceChildren(renderer.domElement);

    const scene = new THREE.Scene();
    const camera = new THREE.OrthographicCamera(-1, 1, 1, -1, 0, 1);

    const uniforms = {
      uTime: { value: 0 },
      uResolution: { value: new THREE.Vector2(1440, 820) },
      uPointer: { value: new THREE.Vector2(0.5, 0.5) },
      uIsDark: { value: isDarkRef.current ? 1.0 : 0.0 },
    };
    uniformsRef.current = uniforms;

    const geometry = new THREE.PlaneGeometry(2, 2);
    const material = new THREE.ShaderMaterial({
      uniforms,
      vertexShader: VERTEX_SHADER,
      fragmentShader: FRAGMENT_SHADER,
      depthTest: false,
      depthWrite: false,
    });
    const quad = new THREE.Mesh(geometry, material);
    scene.add(quad);

    const pointerTarget = { x: 0.5, y: 0.5 };
    const pointerCurrent = { x: 0.5, y: 0.5 };

    const handlePointerMove = (e: PointerEvent) => {
      const rect = container.getBoundingClientRect();
      if (rect.width <= 0 || rect.height <= 0) return;
      pointerTarget.x = Math.max(0, Math.min(1, (e.clientX - rect.left) / rect.width));
      pointerTarget.y = Math.max(0, Math.min(1, 1 - (e.clientY - rect.top) / rect.height));
    };

    const resize = () => {
      const rect = container.getBoundingClientRect();
      const w = Math.max(1, Math.floor(rect.width));
      const h = Math.max(1, Math.floor(rect.height));
      renderer.setSize(w, h, false);
      uniforms.uResolution.value.set(w, h);
      renderer.render(scene, camera);
    };

    const reducedMotion = window.matchMedia('(prefers-reduced-motion: reduce)').matches;
    let isVisible = true;
    let disposed = false;
    let frameId = 0;
    const startMs = performance.now();

    const drawSingleFrame = () => {
      if (disposed) return;
      uniforms.uIsDark.value = isDarkRef.current ? 1.0 : 0.0;
      renderer.render(scene, camera);
    };
    renderImmediateRef.current = drawSingleFrame;

    function afterDraw(cb: () => void) {
      const gl = renderer.getContext() as WebGL2RenderingContext;
      if (gl && typeof gl.fenceSync === 'function') {
        const sync = gl.fenceSync(gl.SYNC_GPU_COMMANDS_COMPLETE, 0);
        gl.flush();
        const poll = () => {
          if (disposed) return;
          const state = sync && gl.getSyncParameter(sync, gl.SYNC_STATUS);
          if (state === gl.UNSIGNALED) {
            frameId = window.setTimeout(poll, 8);
          } else {
            if (sync) gl.deleteSync(sync);
            frameId = window.setTimeout(cb, 85);
          }
        };
        frameId = window.setTimeout(poll, 8);
      } else {
        frameId = window.setTimeout(cb, 95);
      }
    }

    const tick = () => {
      if (disposed) return;
      if (!isVisible || document.hidden) {
        frameId = 0;
        return;
      }

      const elapsedSec = reducedMotion ? 1.8 : (performance.now() - startMs) * 0.001 + 1.8;
      pointerCurrent.x += (pointerTarget.x - pointerCurrent.x) * 0.06;
      pointerCurrent.y += (pointerTarget.y - pointerCurrent.y) * 0.06;

      uniforms.uTime.value = elapsedSec;
      uniforms.uPointer.value.set(pointerCurrent.x, pointerCurrent.y);
      uniforms.uIsDark.value = isDarkRef.current ? 1.0 : 0.0;

      renderer.render(scene, camera);

      if (reducedMotion) return;
      if (isSoftwareGl) {
        afterDraw(tick);
      } else {
        frameId = window.requestAnimationFrame(tick);
      }
    };

    const observer = new IntersectionObserver(
      (entries) => {
        isVisible = entries[0]?.isIntersecting ?? false;
        if (isVisible && !disposed) {
          drawSingleFrame();
          if (!frameId) tick();
        }
      },
      { threshold: 0.01 }
    );

    const handleVisibilityChange = () => {
      if (!document.hidden && isVisible && !disposed) {
        drawSingleFrame();
        if (!frameId) tick();
      }
    };

    resize();
    observer.observe(container);
    window.addEventListener('resize', resize);
    window.addEventListener('pointermove', handlePointerMove, { passive: true });
    document.addEventListener('visibilitychange', handleVisibilityChange);
    tick();

    return () => {
      disposed = true;
      renderImmediateRef.current = null;
      observer.disconnect();
      window.removeEventListener('resize', resize);
      window.removeEventListener('pointermove', handlePointerMove);
      document.removeEventListener('visibilitychange', handleVisibilityChange);
      if (frameId) {
        window.cancelAnimationFrame(frameId);
        window.clearTimeout(frameId);
      }
      geometry.dispose();
      material.dispose();
      renderer.dispose();
      uniformsRef.current = null;
    };
  }, []);

  return (
    <div
      ref={mountRef}
      data-testid="hero-pixelated-shader-mount"
      className={`pointer-events-none absolute inset-0 w-full h-full ${className}`}
      aria-hidden="true"
    />
  );
}
