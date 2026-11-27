import React, { useEffect, useRef } from 'react';

interface CanvasProps {
  className?: string;
  isDark?: boolean;
}

/**
 * Bottom CTA section dotted 3D funnel mesh adapting cleanly to dark & light themes.
 */
export function CtaParticleFunnel({ className = '', isDark = true }: CanvasProps) {
  const canvasRef = useRef<HTMLCanvasElement | null>(null);
  const isDarkRef = useRef(isDark);
  isDarkRef.current = isDark;

  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;
    const ctx = canvas.getContext('2d');
    if (!ctx) return;

    const reducedMotion = window.matchMedia('(prefers-reduced-motion: reduce)').matches;
    let animationId = 0;
    let isVisible = false;
    let lastDrawMs = 0;
    let width = 0;
    let height = 0;

    const cols = 34;
    const rows = 18;

    const resize = () => {
      const dpr = Math.min(window.devicePixelRatio || 1, 1.5);
      const rect = canvas.getBoundingClientRect();
      width = rect.width;
      height = rect.height;
      canvas.width = Math.max(1, Math.floor(width * dpr));
      canvas.height = Math.max(1, Math.floor(height * dpr));
      ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    };

    const drawScene = (timeMs: number) => {
      const dark = isDarkRef.current;
      const time = reducedMotion ? 0 : timeMs * 0.0005;
      ctx.clearRect(0, 0, width, height);

      const focusX = width * 0.72;
      const centerY = height * 0.5;
      const rgb = dark ? '74, 174, 248' : '46, 74, 236';
      const alphaScale = dark ? 1 : 0.76;

      const beamGrad = ctx.createLinearGradient(focusX - 18, centerY, width, centerY);
      beamGrad.addColorStop(0, `rgba(${rgb}, ${(0.92 * alphaScale).toFixed(2)})`);
      beamGrad.addColorStop(0.55, `rgba(${rgb}, ${(0.55 * alphaScale).toFixed(2)})`);
      beamGrad.addColorStop(1, `rgba(${rgb}, 0)`);
      ctx.strokeStyle = beamGrad;
      ctx.lineWidth = 2.5;
      ctx.beginPath();
      ctx.moveTo(focusX - 16, centerY);
      ctx.lineTo(width, centerY);
      ctx.stroke();

      const bloomGrad = ctx.createLinearGradient(focusX - 18, centerY, width, centerY);
      bloomGrad.addColorStop(0, `rgba(${rgb}, ${(0.25 * alphaScale).toFixed(2)})`);
      bloomGrad.addColorStop(1, `rgba(${rgb}, 0)`);
      ctx.strokeStyle = bloomGrad;
      ctx.lineWidth = 10;
      ctx.beginPath();
      ctx.moveTo(focusX - 16, centerY);
      ctx.lineTo(width, centerY);
      ctx.stroke();

      for (let c = 0; c < cols; c++) {
        const u = c / (cols - 1);
        const x = u * focusX;
        const flare = Math.pow(1 - u, 1.95);
        const radiusY = flare * (height * 0.56);

        for (let r = 0; r < rows; r++) {
          const angle = (r / rows) * Math.PI * 2 + u * 2.2 + time * 0.6;
          const sinA = Math.sin(angle);
          const cosA = Math.cos(angle);
          const depth = (cosA + 1) * 0.5;

          const y = centerY + sinA * radiusY;
          const xOffset = cosA * flare * 20;

          const dotSize = (1.8 + depth * 2.0) * (0.5 + 0.5 * (1 - u * 0.5));
          const alpha = (0.18 + depth * 0.78) * (0.25 + 0.75 * Math.pow(u, 0.35)) * alphaScale;

          ctx.fillStyle = `rgba(${rgb}, ${alpha.toFixed(2)})`;
          ctx.fillRect(x + xOffset, y, dotSize, dotSize);
        }
      }
    };

    const tick = (now: number) => {
      if (isVisible && !document.hidden && now - lastDrawMs >= 42) {
        lastDrawMs = now;
        drawScene(now);
      }
      if (!reducedMotion && isVisible) {
        animationId = window.requestAnimationFrame(tick);
      } else {
        animationId = 0;
      }
    };

    const observer = new IntersectionObserver(
      (entries) => {
        isVisible = entries[0]?.isIntersecting ?? false;
        if (isVisible) {
          drawScene(performance.now());
          if (!reducedMotion && !animationId) {
            animationId = window.requestAnimationFrame(tick);
          }
        }
      },
      { threshold: 0.01 }
    );

    resize();
    drawScene(0);
    observer.observe(canvas);
    window.addEventListener('resize', resize);

    return () => {
      observer.disconnect();
      window.removeEventListener('resize', resize);
      if (animationId) window.cancelAnimationFrame(animationId);
    };
  }, [isDark]);

  return (
    <canvas
      ref={canvasRef}
      data-testid="cta-funnel-canvas"
      className={`pointer-events-none block w-full h-full ${className}`}
      aria-hidden="true"
    />
  );
}
