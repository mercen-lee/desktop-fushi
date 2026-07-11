package net.mercen.desktopfushi;

import android.content.Context;
import android.graphics.Color;
import android.graphics.PixelFormat;
import android.graphics.Rect;
import android.os.SystemClock;
import android.util.Log;
import android.view.MotionEvent;
import android.view.Surface;
import android.view.SurfaceHolder;
import android.view.SurfaceView;

public final class FushiOverlayView extends SurfaceView implements SurfaceHolder.Callback {
    static {
        System.loadLibrary("desktop_fushi");
    }

    public interface Host {
        void closeOverlay();
    }

    private static final float MIN_WINDOW_PX = 96.0f;
    private static final int INVALID_POINTER_ID = -1;
    private static final String TAG = "FushiOverlayView";

    private final float[] nativeLayout = new float[4];

    private Host host;
    private long nativeHandle;
    private boolean surfaceAttached;
    private int activePointerId = INVALID_POINTER_ID;
    private float windowX;
    private float windowY;
    private float windowWidth = 390f;
    private float windowHeight = 220f;
    private long lastTapMs;
    private long downMs;

    public FushiOverlayView(Context context) {
        super(context);
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

    public void setWindowSize(float width, float height) {
        windowWidth = Math.max(MIN_WINDOW_PX, width);
        windowHeight = Math.max(MIN_WINDOW_PX, height);
    }

    public void setWindowPosition(float x, float y) {
        windowX = x;
        windowY = y;
    }

    public float getWindowX() { return windowX; }
    public float getWindowY() { return windowY; }
    public float getWindowWidth() { return windowWidth; }
    public float getWindowHeight() { return windowHeight; }

    @Override public void surfaceCreated(SurfaceHolder holder) {
        Rect frame = holder.getSurfaceFrame();
        int width = Math.max(1, frame.width());
        int height = Math.max(1, frame.height());
        ensureNative(width, height);
        attachSurface(holder.getSurface(), width, height);
    }

    @Override public void surfaceChanged(SurfaceHolder holder, int format, int width, int height) {
        width = Math.max(1, width);
        height = Math.max(1, height);
        ensureNative(width, height);
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

    public void applyPhoneShake(float ax, float ay, float az, float dt) {
        if (nativeHandle != 0L) {
            nativeShake(nativeHandle, ax, ay, az, clamp(dt, 0.001f, 0.060f));
        }
    }

    public void step(float dt, int screenW, int screenH) {
        if (nativeHandle == 0L) {
            int width = getWidth() > 0 ? getWidth() : Math.max(1, Math.round(windowWidth));
            int height = getHeight() > 0 ? getHeight() : Math.max(1, Math.round(windowHeight));
            ensureNative(width, height);
        }
        if (nativeHandle == 0L) return;

        nativeStep(
                nativeHandle,
                clamp(dt, 0.001f, 0.050f),
                screenW,
                screenH,
                nativeLayout);
        windowX = nativeLayout[0];
        windowY = nativeLayout[1];
        windowWidth = Math.max(MIN_WINDOW_PX, nativeLayout[2]);
        windowHeight = Math.max(MIN_WINDOW_PX, nativeLayout[3]);
    }

    @Override public boolean onTouchEvent(MotionEvent event) {
        int action = event.getActionMasked();
        int actionIndex = event.getActionIndex();
        switch (action) {
            case MotionEvent.ACTION_DOWN:
                activePointerId = event.getPointerId(0);
                downMs = SystemClock.uptimeMillis();
                if (event.getToolType(0) == MotionEvent.TOOL_TYPE_FINGER) {
                    sendHover(event, 0, false);
                }
                sendPointer(event, 0, true, true);
                return true;
            case MotionEvent.ACTION_MOVE:
                int moveIndex = activePointerIndex(event);
                if (moveIndex >= 0) {
                    sendPointer(event, moveIndex, true, true);
                }
                return true;
            case MotionEvent.ACTION_POINTER_UP:
                if (event.getPointerId(actionIndex) == activePointerId) {
                    sendPointer(event, actionIndex, false, false);
                    activePointerId = INVALID_POINTER_ID;
                }
                return true;
            case MotionEvent.ACTION_UP:
                int upIndex = activePointerIndex(event);
                if (upIndex < 0) upIndex = actionIndex;
                sendPointer(event, upIndex, false, false);
                activePointerId = INVALID_POINTER_ID;
                handleTap(event);
                return true;
            case MotionEvent.ACTION_CANCEL:
                int cancelIndex = activePointerIndex(event);
                if (cancelIndex >= 0) {
                    sendPointer(event, cancelIndex, false, false);
                } else if (nativeHandle != 0L) {
                    nativePointer(nativeHandle, 0f, 0f, false);
                }
                activePointerId = INVALID_POINTER_ID;
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

    private void handleTap(MotionEvent event) {
        long now = SystemClock.uptimeMillis();
        if (now - downMs < 320) {
            if (now - lastTapMs < 360 && host != null) {
                host.closeOverlay();
            }
            lastTapMs = now;
            performClick();
        }
    }

    private int activePointerIndex(MotionEvent event) {
        return activePointerId == INVALID_POINTER_ID
                ? -1
                : event.findPointerIndex(activePointerId);
    }

    private void sendPointer(
            MotionEvent event,
            int pointerIndex,
            boolean down,
            boolean includeHistory) {
        if (nativeHandle == 0L || pointerIndex < 0 || pointerIndex >= event.getPointerCount()) return;
        float rawOffsetX = event.getRawX() - event.getX();
        float rawOffsetY = event.getRawY() - event.getY();
        if (includeHistory) {
            for (int h = 0; h < event.getHistorySize(); h++) {
                nativePointer(
                        nativeHandle,
                        event.getHistoricalX(pointerIndex, h) + rawOffsetX,
                        event.getHistoricalY(pointerIndex, h) + rawOffsetY,
                        down);
            }
        }
        nativePointer(
                nativeHandle,
                event.getX(pointerIndex) + rawOffsetX,
                event.getY(pointerIndex) + rawOffsetY,
                down);
    }

    private void sendHover(MotionEvent event, int pointerIndex, boolean inside) {
        if (nativeHandle == 0L || pointerIndex < 0 || pointerIndex >= event.getPointerCount()) return;
        float rawOffsetX = event.getRawX() - event.getX();
        float rawOffsetY = event.getRawY() - event.getY();
        nativeHover(
                nativeHandle,
                event.getX(pointerIndex) + rawOffsetX,
                event.getY(pointerIndex) + rawOffsetY,
                inside);
    }

    private void attachSurface(Surface surface, int width, int height) {
        if (nativeHandle == 0L || surface == null || !surface.isValid()) return;
        if (surfaceAttached) {
            nativeDetachSurface(nativeHandle);
            surfaceAttached = false;
        }
        surfaceAttached = nativeAttachSurface(nativeHandle, surface, width, height);
        if (!surfaceAttached) {
            Log.e(TAG, "nativeAttachSurface failed for " + width + "x" + height);
        } else {
            Log.i(TAG, "nativeAttachSurface ok for " + width + "x" + height);
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
        int screenW = getResources().getDisplayMetrics().widthPixels;
        int screenH = getResources().getDisplayMetrics().heightPixels;
        nativeHandle = nativeCreate(
                Math.max(1, width),
                Math.max(1, height),
                density(),
                screenW,
                screenH);
        if (nativeHandle == 0L) {
            Log.e(TAG, "nativeCreate failed for " + width + "x" + height);
        } else {
            Log.i(TAG, "nativeCreate ok for " + width + "x" + height);
        }
    }

    private float density() {
        return Math.max(0.5f, getResources().getDisplayMetrics().density);
    }

    private static float clamp(float value, float lo, float hi) {
        return Math.max(lo, Math.min(hi, value));
    }

    private static native long nativeCreate(
            int width,
            int height,
            float density,
            int screenWidth,
            int screenHeight);
    private static native void nativeDestroy(long handle);
    private static native boolean nativeAttachSurface(
            long handle,
            Surface surface,
            int width,
            int height);
    private static native void nativeDetachSurface(long handle);
    private static native void nativeResize(long handle, int width, int height, float density);
    private static native void nativePointer(long handle, float x, float y, boolean down);
    private static native void nativeHover(long handle, float x, float y, boolean inside);
    private static native void nativeShake(long handle, float ax, float ay, float az, float dt);
    private static native void nativeStep(
            long handle,
            float dt,
            int screenWidth,
            int screenHeight,
            float[] layout);
}
