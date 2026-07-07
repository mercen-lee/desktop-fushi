import init, { WebFushiEngine } from "../generated/wasm/desktop_fushi.js";

const DPR_CAP = 2.5;
const stage = document.querySelector("[data-fushi-embed]");
const canvas = document.querySelector("#fushi-canvas");

let engine = null;
let lastFrame = performance.now();
let fushiGrabbing = false;

function dpr() {
  return Math.min(window.devicePixelRatio || 1, DPR_CAP);
}

function canvasSize() {
  const rect = stage.getBoundingClientRect();
  const cssWidth = rect.width || window.innerWidth || 1;
  const cssHeight = rect.height || window.innerHeight || 1;
  return {
    width: Math.max(1, Math.round(cssWidth * dpr())),
    height: Math.max(1, Math.round(cssHeight * dpr())),
  };
}

function showcaseLine() {
  return canvasSize().height * 0.72;
}

function resizeCanvas() {
  const size = canvasSize();
  canvas.width = size.width;
  canvas.height = size.height;
  if (engine) {
    engine.resize(size.width, size.height, dpr(), showcaseLine());
  }
}

function pointerPosition(event) {
  const rect = canvas.getBoundingClientRect();
  return {
    x: (event.clientX - rect.left) * dpr(),
    y: (event.clientY - rect.top) * dpr(),
  };
}

function setFushiHit(hit) {
  stage.dataset.fushiHit = String(Boolean(hit));
}

function setFushiGrabbing(grabbing) {
  fushiGrabbing = Boolean(grabbing);
  stage.dataset.fushiGrabbing = String(fushiGrabbing);
}

function finishLoading() {
  stage.dataset.loading = "false";
  stage.setAttribute("aria-busy", "false");
}

function animate(now) {
  const dt = Math.min(0.05, Math.max(0.001, (now - lastFrame) / 1000));
  lastFrame = now;
  engine?.tick(dt);
  window.requestAnimationFrame(animate);
}

async function startFushi() {
  if (!stage || !canvas) return;
  resizeCanvas();
  await init();
  const size = canvasSize();
  engine = await WebFushiEngine.createDesktop(canvas, size.width, size.height, dpr(), showcaseLine());
  stage.dataset.ready = "true";
  finishLoading();
  window.requestAnimationFrame(animate);
}

canvas?.addEventListener("pointerdown", (event) => {
  const p = pointerPosition(event);
  const hit = engine?.pointer(p.x, p.y, true) ?? false;
  setFushiHit(hit);
  setFushiGrabbing(hit);
  if (hit) {
    event.preventDefault();
    canvas.setPointerCapture(event.pointerId);
  }
});

canvas?.addEventListener("pointermove", (event) => {
  const p = pointerPosition(event);
  if (event.buttons > 0 && fushiGrabbing) {
    setFushiHit(engine?.pointer(p.x, p.y, true) ?? false);
  } else {
    setFushiHit(engine?.hover(p.x, p.y) ?? false);
  }
});

canvas?.addEventListener("pointerup", (event) => {
  const p = pointerPosition(event);
  const hit = engine?.pointer(p.x, p.y, false) ?? false;
  setFushiHit(hit);
  setFushiGrabbing(false);
  if (canvas.hasPointerCapture(event.pointerId)) {
    canvas.releasePointerCapture(event.pointerId);
  }
});

canvas?.addEventListener("pointercancel", (event) => {
  const p = pointerPosition(event);
  engine?.pointer(p.x, p.y, false);
  setFushiHit(false);
  setFushiGrabbing(false);
  if (canvas.hasPointerCapture(event.pointerId)) {
    canvas.releasePointerCapture(event.pointerId);
  }
});

window.addEventListener("resize", resizeCanvas);
if (stage && "ResizeObserver" in window) {
  const resizeObserver = new ResizeObserver(resizeCanvas);
  resizeObserver.observe(stage);
}

startFushi().catch((error) => {
  console.error(error);
  if (stage) {
    stage.dataset.ready = "false";
    stage.dataset.failed = "true";
    finishLoading();
  }
});
