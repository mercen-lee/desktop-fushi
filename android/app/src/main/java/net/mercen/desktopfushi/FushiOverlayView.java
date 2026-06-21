package net.mercen.desktopfushi;

import android.content.Context;
import android.graphics.Bitmap;
import android.graphics.Canvas;
import android.graphics.Color;
import android.graphics.Paint;
import android.os.SystemClock;
import android.util.Log;
import android.view.MotionEvent;
import android.view.View;

public final class FushiOverlayView extends View {
    static {
        System.loadLibrary("desktop_fushi");
    }

    public interface Host {
        void closeOverlay();
    }

    private static final float MIN_WINDOW_DP = 96.0f;
    private static final String TAG = "FushiOverlayView";

    private final Paint paint = new Paint(Paint.ANTI_ALIAS_FLAG | Paint.FILTER_BITMAP_FLAG | Paint.DITHER_FLAG);

    private Host host;
    private long nativeHandle;
    private Bitmap bitmap;
    private boolean copyWarningLogged;
    private float windowX;
    private float windowY;
    private float windowWidth = 390f;
    private float windowHeight = 220f;
    private long lastTapMs;
    private long downMs;

    public FushiOverlayView(Context context) {
        super(context);
        setWillNotDraw(false);
        setFocusable(false);
        setFocusableInTouchMode(false);
        setBackgroundColor(Color.TRANSPARENT);
    }

    public void setHost(Host host) {
        this.host = host;
    }

    public void setWindowSize(float width, float height) {
        windowWidth = Math.max(minWindowPx(), width);
        windowHeight = Math.max(minWindowPx(), height);
    }

    public void setWindowPosition(float x, float y) {
        windowX = x;
        windowY = y;
    }

    public float getWindowX() { return windowX; }
    public float getWindowY() { return windowY; }
    public float getWindowWidth() { return windowWidth; }
    public float getWindowHeight() { return windowHeight; }

    @Override protected void onSizeChanged(int width, int height, int oldWidth, int oldHeight) {
        super.onSizeChanged(width, height, oldWidth, oldHeight);
        if (width <= 0 || height <= 0) return;
        ensureBitmap(width, height);
        if (nativeHandle == 0L) {
            ensureNative(width, height);
        } else {
            nativeResize(nativeHandle, width, height, density());
        }
    }

    @Override protected void onDetachedFromWindow() {
        destroyNative();
        recycleBitmap();
        super.onDetachedFromWindow();
    }

    public void destroyNative() {
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
            ensureNative(Math.max(1, getWidth()), Math.max(1, getHeight()));
        }
        if (nativeHandle == 0L) return;

        float[] frame = nativeStep(nativeHandle, clamp(dt, 0.001f, 0.050f), screenW, screenH);
        if (frame != null && frame.length >= 4) {
            float min = minWindowPx();
            windowX = frame[0];
            windowY = frame[1];
            windowWidth = Math.max(min, frame[2]);
            windowHeight = Math.max(min, frame[3]);
        }

        ensureBitmap(Math.max(1, Math.round(windowWidth)), Math.max(1, Math.round(windowHeight)));
        if (bitmap != null && !nativeCopyFrame(nativeHandle, bitmap) && !copyWarningLogged) {
            Log.e(TAG, "nativeCopyFrame failed for " + bitmap.getWidth() + "x" + bitmap.getHeight());
            copyWarningLogged = true;
        }
        postInvalidateOnAnimation();
    }

    @Override protected void onDraw(Canvas canvas) {
        super.onDraw(canvas);
        if (bitmap != null && !bitmap.isRecycled()) {
            canvas.drawBitmap(bitmap, 0f, 0f, paint);
        }
    }

    @Override public boolean onTouchEvent(MotionEvent event) {
        int action = event.getActionMasked();
        switch (action) {
            case MotionEvent.ACTION_DOWN:
                downMs = SystemClock.uptimeMillis();
                sendPointer(event, true);
                return true;
            case MotionEvent.ACTION_MOVE:
                sendPointer(event, true);
                return true;
            case MotionEvent.ACTION_UP:
            case MotionEvent.ACTION_CANCEL:
                sendPointer(event, false);
                long now = SystemClock.uptimeMillis();
                if (action == MotionEvent.ACTION_UP && now - downMs < 320) {
                    if (now - lastTapMs < 360 && host != null) {
                        host.closeOverlay();
                    }
                    lastTapMs = now;
                    performClick();
                }
                return true;
            default:
                return super.onTouchEvent(event);
        }
    }

    @Override public boolean performClick() {
        super.performClick();
        return true;
    }

    private void sendPointer(MotionEvent event, boolean down) {
        if (nativeHandle == 0L) return;
        nativePointer(nativeHandle, event.getRawX(), event.getRawY(), down);
    }

    private void ensureNative(int width, int height) {
        if (nativeHandle != 0L) return;
        int screenW = getResources().getDisplayMetrics().widthPixels;
        int screenH = getResources().getDisplayMetrics().heightPixels;
        nativeHandle = nativeCreate(Math.max(1, width), Math.max(1, height), density(), screenW, screenH);
        if (nativeHandle == 0L) {
            Log.e(TAG, "nativeCreate failed for " + width + "x" + height);
        } else {
            Log.d(TAG, "nativeCreate ok for " + width + "x" + height);
        }
    }

    private void ensureBitmap(int width, int height) {
        width = Math.max(1, width);
        height = Math.max(1, height);
        if (bitmap != null && !bitmap.isRecycled() && bitmap.getWidth() == width && bitmap.getHeight() == height) {
            return;
        }
        recycleBitmap();
        bitmap = Bitmap.createBitmap(width, height, Bitmap.Config.ARGB_8888);
        bitmap.setPremultiplied(true);
        copyWarningLogged = false;
    }

    private void recycleBitmap() {
        if (bitmap != null && !bitmap.isRecycled()) {
            bitmap.recycle();
        }
        bitmap = null;
    }

    private float density() {
        return Math.max(0.5f, getResources().getDisplayMetrics().density);
    }

    private float minWindowPx() {
        return MIN_WINDOW_DP * density();
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
    private static native void nativeResize(long handle, int width, int height, float density);
    private static native void nativePointer(long handle, float x, float y, boolean down);
    private static native void nativeShake(long handle, float ax, float ay, float az, float dt);
    private static native float[] nativeStep(long handle, float dt, int screenWidth, int screenHeight);
    private static native boolean nativeCopyFrame(long handle, Bitmap bitmap);
}
