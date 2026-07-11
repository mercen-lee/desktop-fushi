package net.mercen.desktopfushi;

import android.app.Notification;
import android.app.NotificationChannel;
import android.app.NotificationManager;
import android.app.PendingIntent;
import android.app.Service;
import android.content.Context;
import android.content.Intent;
import android.content.pm.ServiceInfo;
import android.graphics.Insets;
import android.graphics.PixelFormat;
import android.hardware.Sensor;
import android.hardware.SensorEvent;
import android.hardware.SensorEventListener;
import android.hardware.SensorManager;
import android.os.Build;
import android.os.IBinder;
import android.provider.Settings;
import android.view.Choreographer;
import android.view.Gravity;
import android.view.WindowInsets;
import android.view.WindowManager;

public final class FushiOverlayService extends Service implements SensorEventListener {
    public static final String ACTION_START = "net.mercen.desktopfushi.START";
    public static final String ACTION_STOP = "net.mercen.desktopfushi.STOP";

    private static final String CHANNEL_ID = "desktop_fushi_overlay";
    private static final int NOTIFICATION_ID = 3118;
    private static final int MIN_WINDOW_PX = 96;

    private final float[] gravity = new float[]{0f, 0f, 0f};
    private final Choreographer.FrameCallback frameCallback = this::doFrame;

    private WindowManager windowManager;
    private WindowManager.LayoutParams layoutParams;
    private FushiOverlayView overlayView;
    private SensorManager sensorManager;
    private Sensor motionSensor;
    private Choreographer choreographer;
    private boolean sensorIsLinearAcceleration;
    private boolean framePosted;
    private long lastSensorNs;
    private long lastFrameNs;

    @Override public void onCreate() {
        super.onCreate();
        windowManager = (WindowManager) getSystemService(WINDOW_SERVICE);
        sensorManager = (SensorManager) getSystemService(SENSOR_SERVICE);
        choreographer = Choreographer.getInstance();
        createNotificationChannel();
    }

    @Override public int onStartCommand(Intent intent, int flags, int startId) {
        String action = intent == null ? ACTION_START : intent.getAction();
        if (ACTION_STOP.equals(action)) {
            stopSelf();
            return START_NOT_STICKY;
        }
        if (!canDrawOverlay()) {
            stopSelf();
            return START_NOT_STICKY;
        }
        // Create the visible application-overlay window before entering foreground mode.
        // This keeps the Android 15 SYSTEM_ALERT_WINDOW foreground-service exemption
        // path compatible while the service is started from MainActivity.
        showOverlayIfNeeded();
        startForegroundCompat();
        registerSensors();
        return START_STICKY;
    }

    @Override public void onDestroy() {
        unregisterSensors();
        removeFrameCallback();
        if (overlayView != null) {
            // Drop the wgpu surface while SurfaceHolder is still valid.
            overlayView.destroyNative();
        }
        if (windowManager != null && overlayView != null) {
            try {
                windowManager.removeView(overlayView);
            } catch (RuntimeException ignored) {
            }
        }
        overlayView = null;
        layoutParams = null;
        super.onDestroy();
    }

    @Override public IBinder onBind(Intent intent) { return null; }

    @Override public void onSensorChanged(SensorEvent event) {
        if (overlayView == null) return;
        float dt = 1f / 60f;
        if (lastSensorNs != 0L) {
            dt = clamp((event.timestamp - lastSensorNs) / 1_000_000_000f, 0.001f, 0.060f);
        }
        lastSensorNs = event.timestamp;

        float ax = event.values.length > 0 ? event.values[0] : 0f;
        float ay = event.values.length > 1 ? event.values[1] : 0f;
        float az = event.values.length > 2 ? event.values[2] : 0f;
        if (!sensorIsLinearAcceleration) {
            final float alpha = 0.82f;
            gravity[0] = alpha * gravity[0] + (1f - alpha) * ax;
            gravity[1] = alpha * gravity[1] + (1f - alpha) * ay;
            gravity[2] = alpha * gravity[2] + (1f - alpha) * az;
            ax -= gravity[0];
            ay -= gravity[1];
            az -= gravity[2];
        }
        overlayView.applyPhoneShake(ax, ay, az, dt);
    }

    @Override public void onAccuracyChanged(Sensor sensor, int accuracy) {}

    private void doFrame(long frameTimeNanos) {
        framePosted = false;
        if (overlayView == null || layoutParams == null || windowManager == null) return;

        float dt = lastFrameNs == 0L
                ? 1f / 60f
                : clamp((frameTimeNanos - lastFrameNs) / 1_000_000_000f, 0.001f, 0.050f);
        lastFrameNs = frameTimeNanos;
        int screenW = getResources().getDisplayMetrics().widthPixels;
        int screenH = getResources().getDisplayMetrics().heightPixels;
        overlayView.step(dt, screenW, screenH);

        if (updateOverlayLayout(screenW, screenH)) {
            try {
                windowManager.updateViewLayout(overlayView, layoutParams);
            } catch (RuntimeException ignored) {
                // The view can disappear during service shutdown.
            }
        }
        postFrameCallback();
    }

    private void postFrameCallback() {
        if (!framePosted && choreographer != null && overlayView != null) {
            framePosted = true;
            choreographer.postFrameCallback(frameCallback);
        }
    }

    private void removeFrameCallback() {
        if (choreographer != null && framePosted) {
            choreographer.removeFrameCallback(frameCallback);
        }
        framePosted = false;
        lastFrameNs = 0L;
    }

    private void showOverlayIfNeeded() {
        if (overlayView != null) return;
        int screenW = getResources().getDisplayMetrics().widthPixels;
        int screenH = getResources().getDisplayMetrics().heightPixels;
        int initialWidth = Math.min(screenW, dp(390));
        int initialHeight = Math.min(screenH, dp(220));

        overlayView = new FushiOverlayView(this);
        overlayView.setHost(() -> stopSelf());
        overlayView.setWindowSize(initialWidth, initialHeight);
        overlayView.setWindowPosition(
                screenW * 0.5f - initialWidth * 0.5f,
                Math.max(0f, screenH * 0.68f - initialHeight * 0.5f));

        // Resolve the Rust body's initial bounds before creating the SurfaceView so the first
        // ANativeWindow and swapchain are already close to the final pet envelope.
        overlayView.step(1f / 60f, screenW, screenH);

        layoutParams = new WindowManager.LayoutParams(
                Math.max(MIN_WINDOW_PX, Math.round(overlayView.getWindowWidth())),
                Math.max(MIN_WINDOW_PX, Math.round(overlayView.getWindowHeight())),
                WindowManager.LayoutParams.TYPE_APPLICATION_OVERLAY,
                WindowManager.LayoutParams.FLAG_NOT_FOCUSABLE
                        | WindowManager.LayoutParams.FLAG_NOT_TOUCH_MODAL
                        | WindowManager.LayoutParams.FLAG_LAYOUT_NO_LIMITS,
                PixelFormat.TRANSLUCENT
        );
        layoutParams.gravity = Gravity.START | Gravity.TOP;
        updateOverlayLayout(screenW, screenH);
        windowManager.addView(overlayView, layoutParams);
        lastFrameNs = 0L;
        postFrameCallback();
    }

    private void registerSensors() {
        if (sensorManager == null) return;
        if (motionSensor == null) {
            motionSensor = sensorManager.getDefaultSensor(Sensor.TYPE_LINEAR_ACCELERATION);
            sensorIsLinearAcceleration = motionSensor != null;
            if (motionSensor == null) {
                motionSensor = sensorManager.getDefaultSensor(Sensor.TYPE_ACCELEROMETER);
                sensorIsLinearAcceleration = false;
            }
        }
        if (motionSensor != null) {
            sensorManager.registerListener(this, motionSensor, SensorManager.SENSOR_DELAY_GAME);
        }
    }

    private void unregisterSensors() {
        if (sensorManager != null) sensorManager.unregisterListener(this);
        lastSensorNs = 0L;
    }

    private boolean canDrawOverlay() {
        return Build.VERSION.SDK_INT < Build.VERSION_CODES.M || Settings.canDrawOverlays(this);
    }

    private void startForegroundCompat() {
        Notification notification = buildNotification();
        if (Build.VERSION.SDK_INT >= 34) {
            startForeground(
                    NOTIFICATION_ID,
                    notification,
                    ServiceInfo.FOREGROUND_SERVICE_TYPE_SPECIAL_USE);
        } else {
            startForeground(NOTIFICATION_ID, notification);
        }
    }

    private Notification buildNotification() {
        Intent openIntent = new Intent(this, MainActivity.class);
        int flags = PendingIntent.FLAG_UPDATE_CURRENT;
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.M) flags |= PendingIntent.FLAG_IMMUTABLE;
        PendingIntent pendingIntent = PendingIntent.getActivity(this, 0, openIntent, flags);
        Notification.Builder builder = Build.VERSION.SDK_INT >= Build.VERSION_CODES.O
                ? new Notification.Builder(this, CHANNEL_ID)
                : new Notification.Builder(this);
        return builder
                .setContentTitle(getString(R.string.overlay_notification_title))
                .setContentText(getString(R.string.overlay_notification_text))
                .setSmallIcon(R.drawable.ic_fushi_notification)
                .setContentIntent(pendingIntent)
                .setOngoing(true)
                .setShowWhen(false)
                .build();
    }

    private void createNotificationChannel() {
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.O) return;
        NotificationChannel channel = new NotificationChannel(
                CHANNEL_ID,
                getString(R.string.app_name) + " overlay",
                NotificationManager.IMPORTANCE_LOW
        );
        channel.setDescription(getString(R.string.app_name) + " floating overlay service");
        NotificationManager manager =
                (NotificationManager) getSystemService(Context.NOTIFICATION_SERVICE);
        if (manager != null) manager.createNotificationChannel(channel);
    }

    private int dp(float value) {
        return Math.round(value * getResources().getDisplayMetrics().density);
    }

    private boolean updateOverlayLayout(int screenW, int screenH) {
        if (overlayView == null || layoutParams == null) return false;
        int width = Math.max(MIN_WINDOW_PX, Math.round(overlayView.getWindowWidth()));
        int height = Math.max(MIN_WINDOW_PX, Math.round(overlayView.getWindowHeight()));
        int topInset = topWindowInsetPx();
        int x = clampInt(
                Math.round(overlayView.getWindowX()),
                0,
                Math.max(0, screenW - width));
        int y = clampInt(
                Math.round(overlayView.getWindowY()) - topInset,
                -topInset,
                Math.max(-topInset, screenH - height - topInset));

        if (layoutParams.width == width
                && layoutParams.height == height
                && layoutParams.x == x
                && layoutParams.y == y) {
            return false;
        }
        layoutParams.width = width;
        layoutParams.height = height;
        layoutParams.x = x;
        layoutParams.y = y;
        return true;
    }

    private int topWindowInsetPx() {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.R && windowManager != null) {
            WindowInsets windowInsets = windowManager.getCurrentWindowMetrics().getWindowInsets();
            Insets safe = windowInsets.getInsetsIgnoringVisibility(
                    WindowInsets.Type.statusBars() | WindowInsets.Type.displayCutout());
            return Math.max(0, safe.top);
        }
        int id = getResources().getIdentifier("status_bar_height", "dimen", "android");
        return id > 0 ? getResources().getDimensionPixelSize(id) : 0;
    }

    private static int clampInt(int value, int lo, int hi) {
        return Math.max(lo, Math.min(hi, value));
    }

    private static float clamp(float value, float lo, float hi) {
        return Math.max(lo, Math.min(hi, value));
    }
}
