import init, { WebFushiEngine } from "../generated/wasm/desktop_fushi.js";

const DPR_CAP = 2.5;
const stage = document.querySelector("[data-fushi-stage]");
const canvas = document.querySelector("#fushi-canvas");
const downloadLink = document.querySelector("[data-download-link]");
const menuButton = document.querySelector("[data-platform-button]");
const menu = document.querySelector("[data-platform-menu]");
const selectedIcon = document.querySelector("[data-selected-icon]");
const selectedLabel = document.querySelector("[data-selected-label]");
const selectedMeta = document.querySelector("[data-selected-meta]");
const platformStatus = document.querySelector("[data-platform-status]");
const releaseData = JSON.parse(document.querySelector("#release-data").textContent);

let engine = null;
let selectedPlatform = null;
let lastFrame = performance.now();
let fushiGrabbing = false;
let obstacleSyncQueued = false;

const iconMarkup = {
  windows:
    '<svg viewBox="0 0 24 24" aria-hidden="true"><path d="M3 5.1 10.7 4v7.4H3V5.1Zm0 7.5h7.7V20L3 18.9v-6.3Zm8.9-8.8L21 2.5v8.9h-9.1V3.8Zm0 8.8H21v8.9l-9.1-1.3v-7.6Z"/></svg>',
  apple:
    '<svg viewBox="0 0 24 24" aria-hidden="true"><path d="M16.8 13.1c0-2.2 1.8-3.2 1.9-3.3-1-1.5-2.6-1.7-3.2-1.7-1.4-.1-2.6.8-3.3.8s-1.8-.8-2.9-.8c-1.5 0-2.9.9-3.7 2.2-1.6 2.8-.4 6.9 1.1 9.1.8 1.1 1.7 2.3 2.9 2.3 1.1 0 1.6-.7 3-.7s1.8.7 3 .7c1.3 0 2.1-1.1 2.8-2.2.9-1.3 1.2-2.5 1.2-2.6 0 0-2.7-1-2.8-3.8ZM14.6 6.6c.6-.7 1-1.8.9-2.8-.9 0-2 .6-2.7 1.3-.6.7-1 1.7-.9 2.7 1 0 2-.5 2.7-1.2Z"/></svg>',
};

function dpr() {
  return Math.min(window.devicePixelRatio || 1, DPR_CAP);
}

function canvasSize() {
  return {
    width: Math.max(1, Math.round(window.innerWidth * dpr())),
    height: Math.max(1, Math.round(window.innerHeight * dpr())),
  };
}

function showcaseLine() {
  return window.innerHeight * 0.72 * dpr();
}

function resizeCanvas() {
  canvas.style.left = "0";
  canvas.style.top = "0";
  canvas.style.transform = "none";
  canvas.style.width = "100vw";
  canvas.style.height = "100vh";
  const size = canvasSize();
  canvas.width = size.width;
  canvas.height = size.height;
  if (engine) {
    engine.resize(size.width, size.height, dpr(), showcaseLine());
    syncUiObstacles();
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

function syncAwakenedState() {
  stage.dataset.awakened = String(engine?.isAwakened() ?? false);
}

function collectUiObstacleRects() {
  const rects = [];
  if (!menu.hidden) {
    const canvasRect = canvas.getBoundingClientRect();
    const rect = menu.getBoundingClientRect();
    if (rect.width > 0 && rect.height > 0) {
      const left = Math.max(0, rect.left - canvasRect.left - 10);
      const top = Math.max(0, rect.top - canvasRect.top - 10);
      const right = Math.min(canvasRect.width, rect.right - canvasRect.left + 10);
      const bottom = Math.min(canvasRect.height, rect.bottom - canvasRect.top + 10);
      if (right > left && bottom > top) {
        rects.push(left * dpr(), top * dpr(), right * dpr(), bottom * dpr());
      }
    }
  }

  return new Float32Array(rects);
}

function syncUiObstacles() {
  if (!engine) return;
  engine.setUiRects(collectUiObstacleRects());
}

function queueUiObstacleSync() {
  if (obstacleSyncQueued) return;
  obstacleSyncQueued = true;
  requestAnimationFrame(() => {
    obstacleSyncQueued = false;
    syncUiObstacles();
  });
}

function choosePlatform() {
  const ua = navigator.userAgent.toLowerCase();
  const platform = (navigator.userAgentData?.platform || navigator.platform || "").toLowerCase();
  const isAppleMobile = /iphone|ipad|ipod/.test(ua) || (platform.includes("mac") && navigator.maxTouchPoints > 1);
  if (ua.includes("android") || isAppleMobile || platform.includes("linux") || platform.includes("chrome os")) {
    return null;
  }
  if (platform.includes("mac")) {
    return "macos-universal";
  }
  if (platform.includes("win")) {
    if (ua.includes("arm64") || ua.includes("aarch64")) {
      return "windows-arm64";
    }
    return "windows-x64";
  }
  return null;
}

async function refinePlatformGuess() {
  if (!navigator.userAgentData?.getHighEntropyValues) {
    return choosePlatform();
  }
  try {
    const hints = await navigator.userAgentData.getHighEntropyValues(["architecture", "platform"]);
    const platform = (hints.platform || "").toLowerCase();
    const architecture = (hints.architecture || "").toLowerCase();
    if (platform.includes("android") || platform.includes("linux") || platform.includes("chrome os")) return null;
    if (platform.includes("mac")) {
      return architecture.includes("arm") ? "macos-arm64" : "macos-universal";
    }
    if (platform.includes("win")) {
      return architecture.includes("arm") ? "windows-arm64" : "windows-x64";
    }
  } catch {
    return choosePlatform();
  }
  return choosePlatform();
}

function setBrowserSupport(isSupported) {
  stage.dataset.browserSupported = String(isSupported);
  platformStatus.hidden = isSupported;
  if (!isSupported) {
    platformStatus.textContent = `This platform is not supported in Desktop Fushi ${releaseData.version}. Windows and macOS builds are available.`;
  }
}

function setPlatform(key) {
  selectedPlatform = releaseData.platforms.find((item) => item.key === key) ?? releaseData.platforms[0];
  selectedIcon.innerHTML = iconMarkup[selectedPlatform.icon];
  selectedLabel.textContent = selectedPlatform.label;
  selectedMeta.textContent = selectedPlatform.meta;
  downloadLink.href = selectedPlatform.href;
  downloadLink.download = selectedPlatform.fileName;
  downloadLink.setAttribute("aria-label", `Download ${selectedPlatform.label}`);
  for (const item of menu.querySelectorAll("[data-platform-option]")) {
    item.setAttribute("aria-selected", String(item.dataset.platformOption === selectedPlatform.key));
  }
  queueUiObstacleSync();
}

function trackDownloadClick() {
  if (typeof window.gtag !== "function" || !selectedPlatform) return;
  window.gtag("event", "download_click", {
    event_category: "download",
    event_label: selectedPlatform.key,
    platform_key: selectedPlatform.key,
    platform_label: selectedPlatform.label,
    platform_file: selectedPlatform.fileName,
    file_name: selectedPlatform.fileName,
    platform_url: selectedPlatform.href,
    download_url: selectedPlatform.href,
    release_version: releaseData.version,
    release_tag: releaseData.tag,
  });
}

function closeMenu() {
  menu.hidden = true;
  menuButton.setAttribute("aria-expanded", "false");
  queueUiObstacleSync();
}

function toggleMenu() {
  const opening = !menu.hidden;
  menu.hidden = opening;
  menuButton.setAttribute("aria-expanded", String(!opening));
  queueUiObstacleSync();
}

function animate(now) {
  const dt = Math.min(0.05, Math.max(0.001, (now - lastFrame) / 1000));
  lastFrame = now;
  engine?.tick(dt);
  syncAwakenedState();
  window.requestAnimationFrame(animate);
}

async function startFushi() {
  resizeCanvas();
  await init();
  const size = canvasSize();
  engine = await WebFushiEngine.create(canvas, size.width, size.height, dpr(), showcaseLine());
  syncUiObstacles();
  syncAwakenedState();
  stage.dataset.ready = "true";
  finishLoading();
  window.requestAnimationFrame(animate);
}

menuButton.addEventListener("click", toggleMenu);
menu.addEventListener("click", (event) => {
  const option = event.target.closest("[data-platform-option]");
  if (!option) return;
  setPlatform(option.dataset.platformOption);
  closeMenu();
  downloadLink.focus();
});
downloadLink.addEventListener("click", trackDownloadClick);
document.addEventListener("click", (event) => {
  if (!event.target.closest("[data-platform-picker]")) {
    closeMenu();
  }
});
document.addEventListener("keydown", (event) => {
  if (event.key === "Escape") closeMenu();
});

canvas.addEventListener("pointerdown", (event) => {
  const p = pointerPosition(event);
  const hit = engine?.pointer(p.x, p.y, true) ?? false;
  setFushiHit(hit);
  setFushiGrabbing(hit);
  if (hit) {
    event.preventDefault();
    canvas.setPointerCapture(event.pointerId);
  }
});
canvas.addEventListener("pointermove", (event) => {
  const p = pointerPosition(event);
  if (event.buttons > 0 && fushiGrabbing) {
    setFushiHit(engine?.pointer(p.x, p.y, true) ?? false);
  } else {
    setFushiHit(engine?.hover(p.x, p.y) ?? false);
  }
});
canvas.addEventListener("pointerup", (event) => {
  const p = pointerPosition(event);
  const hit = engine?.pointer(p.x, p.y, false) ?? false;
  setFushiHit(hit);
  setFushiGrabbing(false);
  if (canvas.hasPointerCapture(event.pointerId)) {
    canvas.releasePointerCapture(event.pointerId);
  }
});
canvas.addEventListener("pointercancel", (event) => {
  const p = pointerPosition(event);
  engine?.pointer(p.x, p.y, false);
  setFushiHit(false);
  setFushiGrabbing(false);
  if (canvas.hasPointerCapture(event.pointerId)) {
    canvas.releasePointerCapture(event.pointerId);
  }
});

window.addEventListener("resize", resizeCanvas);
if ("ResizeObserver" in window) {
  const menuObserver = new ResizeObserver(queueUiObstacleSync);
  menuObserver.observe(menu);
}
const detectedPlatform = await refinePlatformGuess();
setBrowserSupport(Boolean(detectedPlatform));
setPlatform(detectedPlatform ?? releaseData.platforms[0].key);
startFushi().catch((error) => {
  console.error(error);
  stage.dataset.ready = "false";
  stage.dataset.failed = "true";
  finishLoading();
});
