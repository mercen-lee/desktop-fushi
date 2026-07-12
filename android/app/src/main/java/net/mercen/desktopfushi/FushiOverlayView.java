package net.mercen.desktopfushi;

import android.content.Context;
import android.graphics.Color;
import android.graphics.PixelFormat;
import android.graphics.Rect;
import android.os.Build;
import android.os.SystemClock;
import android.util.Log;
import android.view.MotionEvent;
import android.view.Surface;
import android.view.SurfaceHolder;
import android.view.SurfaceView;
import android.view.ViewConfiguration;

public final class FushiOverlayView extends SurfaceView implements
        SurfaceHolder.Callback {
    static {
        System.loadLibrary("desktop_fushi");
    }

    public interface Host {
        void closeOverlay();
    }

    private static final int INVALID_POINTER_ID = -1;
    private static final String TAG = "FushiOverlayView";
    private static final float MIN_WINDOW_PX = 96.0f;

    private final float[] nativeLayout = new float[4];
    private int graphicsBackend;
    private final int touchSlop;
    private final int doubleTapSlop;

    private Host host;
    private long nativeHandle;
    private boolean surfaceAttached;
    private final int sizePreset;
    private int activePointerId = INVALID_POINTER_ID;
    private float windowX;
    private float windowY;
    private float windowWidth = MIN_WINDOW_PX;
    private float windowHeight = MIN_WINDOW_PX;
    private long lastTapMs;
    private long downMs;
    private float downRawX;
    private float downRawY;
    private float lastTapRawX;
    private float lastTapRawY;
    private boolean tapCandidate;
    private int screenWidth;
    private int screenHeight;
    private int workLeft;
    private int workTop;
    private int workRight;
    private int workBottom;
    private float preferredFrameRate;

    public FushiOverlayView(Context context, int graphicsBackend, int sizePreset) {
        super(context);
        this.graphicsBackend = graphicsBackend;
        this.sizePreset = sizePreset;
        ViewConfiguration configuration = ViewConfiguration.get(context);
        touchSlop = configuration.getScaledTouchSlop();
        doubleTapSlop = configuration.getScaledDoubleTapSlop();
        screenWidth = getResources().getDisplayMetrics().widthPixels;
        screenHeight = getResources().getDisplayMetrics().heightPixels;
        workRight = screenWidth;
        workBottom = screenHeight;
        setFocusable(false);
        setFocusableInTouchMode(false);
        setBackgroundColor(Color.TRANSPARENT);
        setZOrderOnTop(true);
        SurfaceHolder holder = getHolder();
        holder.setFormat(PixelFormat.TRANSLUCENT);
        holder.addCallback(this);
    }

    public void setHost(Host host) {
        this.host = host;
    }

    public float getWindowX() { return windowX; }
    public float getWindowY() { return windowY; }
    public float getWindowWidth() { return windowWidth; }
    public float getWindowHeight() { return windowHeight; }

    public void setPreferredFrameRate(float frameRate) {
        preferredFrameRate = Math.max(0f, frameRate);
    }

    public static boolean isVulkanSupported() {
        return nativeIsVulkanSupported();
    }

    @Override public void surfaceCreated(SurfaceHolder holder) {
        Rect frame = holder.getSurfaceFrame();
        int width = Math.max(1, frame.width());
        int height = Math.max(1, frame.height());
        ensureNative(width, height);
        requestSurfaceFrameRate(holder.getSurface());
        attachSurface(holder.getSurface(), width, height);
    }

    @Override public void surfaceChanged(SurfaceHolder holder, int format, int width, int height) {
        width = Math.max(1, width);
        height = Math.max(1, height);
        ensureNative(width, height);
        requestSurfaceFrameRate(holder.getSurface());
        if (!surfaceAttached) {
            attachSurface(holder.getSurface(), width, height);
        } else if (nativeHandle != 0L) {
            nativeResize(nativeHandle, width, height, density());
        }
    }

    @Override public void surfaceDestroyed(SurfaceHolder holder) {
        detachSurface();
    }

    @Override protected void onDetachedFromWindow() {
        destroyNative();
        super.onDetachedFromWindow();
    }

    public void destroyNative() {
        detachSurface();
        long handle = nativeHandle;
        nativeHandle = 0L;
        if (handle != 0L) {
            nativeDestroy(handle);
        }
    }

    public void step(
            float dt,
            int screenW,
            int screenH,
            int safeLeft,
            int safeTop,
            int safeRight,
            int safeBottom) {
        screenWidth = Math.max(1, screenW);
        screenHeight = Math.max(1, screenH);
        workLeft = safeLeft;
        workTop = safeTop;
        workRight = safeRight;
        workBottom = safeBottom;
        if (nativeHandle == 0L) {
            int width = getWidth() > 0 ? getWidth() : screenWidth;
            int height = getHeight() > 0 ? getHeight() : screenHeight;
            ensureNative(width, height);
        }
        if (nativeHandle == 0L) return;

        nativeStep(
                nativeHandle,
                clamp(dt, 0.001f, 0.050f),
                screenW,
                screenH,
                safeLeft,
                safeTop,
                safeRight,
                safeBottom,
                nativeLayout);
        if (isFinite(nativeLayout[0])
                && isFinite(nativeLayout[1])
                && isFinite(nativeLayout[2])
                && isFinite(nativeLayout[3])
                && nativeLayout[2] >= MIN_WINDOW_PX
                && nativeLayout[3] >= MIN_WINDOW_PX) {
            windowX = nativeLayout[0];
            windowY = nativeLayout[1];
            windowWidth = nativeLayout[2];
            windowHeight = nativeLayout[3];
        }
    }

    @Override public boolean onTouchEvent(MotionEvent event) {
        int action = event.getActionMasked();
        int actionIndex = event.getActionIndex();
        switch (action) {
            case MotionEvent.ACTION_DOWN:
                float rawX = rawX(event, 0);
                float rawY = rawY(event, 0);
                if (nativeHandle == 0L || !nativeTryBeginDrag(nativeHandle, rawX, rawY)) {
                    activePointerId = INVALID_POINTER_ID;
                    tapCandidate = false;
                    return false;
                }
                activePointerId = event.getPointerId(0);
                downMs = SystemClock.uptimeMillis();
                downRawX = rawX;
                downRawY = rawY;
                tapCandidate = true;
                if (event.getToolType(0) == MotionEvent.TOOL_TYPE_FINGER) {
                    sendHover(event, 0, false);
                }
                return true;
            case MotionEvent.ACTION_MOVE:
                int moveIndex = activePointerIndex(event);
                if (moveIndex >= 0) {
                    float moveX = rawX(event, moveIndex);
                    float moveY = rawY(event, moveIndex);
                    float dx = moveX - downRawX;
                    float dy = moveY - downRawY;
                    if (tapCandidate && dx * dx + dy * dy > touchSlop * touchSlop) {
                        tapCandidate = false;
                        lastTapMs = 0L;
                    }
                    sendPointer(event, moveIndex, true);
                }
                return true;
            case MotionEvent.ACTION_POINTER_UP:
                if (event.getPointerId(actionIndex) == activePointerId) {
                    sendPointer(event, actionIndex, false);
                    activePointerId = INVALID_POINTER_ID;
                    tapCandidate = false;
                    lastTapMs = 0L;
                }
                return true;
            case MotionEvent.ACTION_UP:
                int upIndex = activePointerIndex(event);
                if (upIndex < 0) upIndex = actionIndex;
                float upX = rawX(event, upIndex);
                float upY = rawY(event, upIndex);
                sendPointer(event, upIndex, false);
                activePointerId = INVALID_POINTER_ID;
                handleTap(upX, upY);
                tapCandidate = false;
                return true;
            case MotionEvent.ACTION_CANCEL:
                activePointerId = INVALID_POINTER_ID;
                tapCandidate = false;
                lastTapMs = 0L;
                if (nativeHandle != 0L) {
                    nativeCancelPointer(nativeHandle);
                }
                return true;
            default:
                return super.onTouchEvent(event);
        }
    }

    @Override public boolean onHoverEvent(MotionEvent event) {
        int action = event.getActionMasked();
        switch (action) {
            case MotionEvent.ACTION_HOVER_ENTER:
            case MotionEvent.ACTION_HOVER_MOVE:
                sendHover(event, 0, true);
                return true;
            case MotionEvent.ACTION_HOVER_EXIT:
                sendHover(event, 0, false);
                return true;
            default:
                return super.onHoverEvent(event);
        }
    }

    @Override public boolean performClick() {
        super.performClick();
        return true;
    }

    private void handleTap(float rawX, float rawY) {
        long now = SystemClock.uptimeMillis();
        if (tapCandidate && now - downMs < 320) {
            float dx = rawX - lastTapRawX;
            float dy = rawY - lastTapRawY;
            boolean isDoubleTap = lastTapMs != 0L
                    && now - lastTapMs <= ViewConfiguration.getDoubleTapTimeout()
                    && dx * dx + dy * dy <= doubleTapSlop * doubleTapSlop;
            if (isDoubleTap && host != null) {
                lastTapMs = 0L;
                host.closeOverlay();
            } else {
                lastTapMs = now;
                lastTapRawX = rawX;
                lastTapRawY = rawY;
            }
            performClick();
        }
    }

    private int activePointerIndex(MotionEvent event) {
        return activePointerId == INVALID_POINTER_ID
                ? -1
                : event.findPointerIndex(activePointerId);
    }

    private void sendPointer(MotionEvent event, int pointerIndex, boolean down) {
        if (nativeHandle == 0L || pointerIndex < 0 || pointerIndex >= event.getPointerCount()) return;
        nativePointer(nativeHandle, rawX(event, pointerIndex), rawY(event, pointerIndex), down);
    }

    private void sendHover(MotionEvent event, int pointerIndex, boolean inside) {
        if (nativeHandle == 0L || pointerIndex < 0 || pointerIndex >= event.getPointerCount()) return;
        nativeHover(nativeHandle, rawX(event, pointerIndex), rawY(event, pointerIndex), inside);
    }

    private static float rawX(MotionEvent event, int pointerIndex) {
        return event.getX(pointerIndex) + event.getRawX() - event.getX();
    }

    private static float rawY(MotionEvent event, int pointerIndex) {
        return event.getY(pointerIndex) + event.getRawY() - event.getY();
    }

    private void attachSurface(Surface surface, int width, int height) {
        if (nativeHandle == 0L || surface == null || !surface.isValid()) return;
        if (surfaceAttached) {
            nativeDetachSurface(nativeHandle);
            surfaceAttached = false;
        }
        surfaceAttached = nativeAttachSurface(nativeHandle, surface, width, height);
        if (!surfaceAttached && graphicsBackend == FushiSettings.BACKEND_VULKAN) {
            Log.w(TAG, "Vulkan surface initialization failed; retrying with GLES");
            FushiSettings.markVulkanRuntimeFailure();
            destroyNative();
            graphicsBackend = FushiSettings.BACKEND_GLES;
            ensureNative(width, height);
            if (nativeHandle != 0L) {
                surfaceAttached = nativeAttachSurface(nativeHandle, surface, width, height);
            }
        }
        if (!surfaceAttached) {
            Log.e(TAG, "nativeAttachSurface failed for " + width + "x" + height
                    + " backend=" + graphicsBackend);
        } else {
            Log.i(TAG, "nativeAttachSurface ok for " + width + "x" + height
                    + " backend=" + graphicsBackend);
        }
    }

    private void requestSurfaceFrameRate(Surface surface) {
        if (surface == null || !surface.isValid() || preferredFrameRate <= 0f) return;
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) {
            surface.setFrameRate(
                    preferredFrameRate,
                    Surface.FRAME_RATE_COMPATIBILITY_DEFAULT,
                    Surface.CHANGE_FRAME_RATE_ONLY_IF_SEAMLESS);
        } else if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.R) {
            surface.setFrameRate(preferredFrameRate, Surface.FRAME_RATE_COMPATIBILITY_DEFAULT);
        }
    }

    private void detachSurface() {
        if (nativeHandle != 0L && surfaceAttached) {
            nativeDetachSurface(nativeHandle);
        }
        surfaceAttached = false;
    }

    private void ensureNative(int width, int height) {
        if (nativeHandle != 0L) return;
        nativeHandle = nativeCreate(
                Math.max(1, width),
                Math.max(1, height),
                density(),
                screenWidth,
                screenHeight,
                workLeft,
                workTop,
                workRight,
                workBottom,
                graphicsBackend,
                sizePreset);
        if (nativeHandle == 0L) {
            Log.e(TAG, "nativeCreate failed for " + width + "x" + height);
        } else {
            Log.i(TAG, "nativeCreate ok for " + width + "x" + height
                    + " backend=" + graphicsBackend + " sizePreset=" + sizePreset);
        }
    }

    private float density() {
        return Math.max(0.5f, getResources().getDisplayMetrics().density);
    }

    private static float clamp(float value, float lo, float hi) {
        return Math.max(lo, Math.min(hi, value));
    }

    private static boolean isFinite(float value) {
        return !Float.isNaN(value) && !Float.isInfinite(value);
    }

    private static native boolean nativeIsVulkanSupported();
    private static native long nativeCreate(
            int width,
            int height,
            float density,
            int screenWidth,
            int screenHeight,
            int workLeft,
            int workTop,
            int workRight,
            int workBottom,
            int graphicsBackend,
            int sizePreset);
    private static native void nativeDestroy(long handle);
    private static native boolean nativeAttachSurface(
            long handle,
            Surface surface,
            int width,
            int height);
    private static native void nativeDetachSurface(long handle);
    private static native void nativeResize(long handle, int width, int height, float density);
    private static native boolean nativeTryBeginDrag(long handle, float x, float y);
    private static native void nativePointer(long handle, float x, float y, boolean down);
    private static native void nativeCancelPointer(long handle);
    private static native void nativeHover(long handle, float x, float y, boolean inside);
    private static native void nativeStep(
            long handle,
            float dt,
            int screenWidth,
            int screenHeight,
            int workLeft,
            int workTop,
            int workRight,
            int workBottom,
            float[] layout);
}
