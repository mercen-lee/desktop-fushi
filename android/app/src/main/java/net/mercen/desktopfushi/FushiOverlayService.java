package net.mercen.desktopfushi;

import android.app.Notification;
import android.app.NotificationChannel;
import android.app.NotificationManager;
import android.app.PendingIntent;
import android.app.Service;
import android.content.Context;
import android.content.Intent;
import android.content.SharedPreferences;
import android.content.pm.ServiceInfo;
import android.graphics.Insets;
import android.graphics.PixelFormat;
import android.graphics.Rect;
import android.hardware.Sensor;
import android.hardware.SensorEvent;
import android.hardware.SensorEventListener;
import android.hardware.SensorManager;
import android.os.Build;
import android.os.IBinder;
import android.provider.Settings;
import android.util.DisplayMetrics;
import android.view.Choreographer;
import android.view.Display;
import android.view.Gravity;
import android.view.Surface;
import android.view.WindowInsets;
import android.view.WindowManager;
import android.view.WindowMetrics;

public final class FushiOverlayService extends Service implements SensorEventListener {
    public static final String ACTION_START = "net.mercen.desktopfushi.START";
    public static final String ACTION_STOP = "net.mercen.desktopfushi.STOP";
    public static final String ACTION_STATE_CHANGED = "net.mercen.desktopfushi.STATE_CHANGED";
    public static final String EXTRA_RUNNING = "running";
    public static final String EXTRA_STARTED_FROM_BOOT = "started_from_boot";

    private static final String CHANNEL_ID = "desktop_fushi_overlay";
    private static final int NOTIFICATION_ID = 3118;
    private static final int MIN_WINDOW_PX = 96;
    private static final long MOTION_DISCONTINUITY_NS = 250_000_000L;
    private static volatile boolean running;
    private static final class DisplayGeometry {
        final int width;
        final int height;
        final int workLeft;
        final int workTop;
        final int workRight;
        final int workBottom;

        DisplayGeometry(
                int width,
                int height,
                int workLeft,
                int workTop,
                int workRight,
                int workBottom) {
            this.width = width;
            this.height = height;
            this.workLeft = workLeft;
            this.workTop = workTop;
            this.workRight = workRight;
            this.workBottom = workBottom;
        }
    }

    private final Choreographer.FrameCallback frameCallback = this::doFrame;
    private final SharedPreferences.OnSharedPreferenceChangeListener settingsListener =
            (preferences, key) -> {
                if (FushiSettings.KEY_SIZE_PRESET.equals(key)) {
                    applySizeSetting();
                }
            };

    private WindowManager windowManager;
    private WindowManager.LayoutParams layoutParams;
    private FushiOverlayView overlayView;
    private SensorManager sensorManager;
    private Choreographer choreographer;
    private boolean sensorsRegistered;
    private boolean rawAccelerometerFallback;
    private int motionSensorMode = FushiOverlayView.MOTION_SENSOR_MODE_NONE;
    private boolean framePosted;
    private int lastDisplayRotation = -1;
    private long lastFrameNs;

    @Override public void onCreate() {
        super.onCreate();
        windowManager = (WindowManager) getSystemService(WINDOW_SERVICE);
        sensorManager = (SensorManager) getSystemService(SENSOR_SERVICE);
        choreographer = Choreographer.getInstance();
        FushiSettings.preferences(this).registerOnSharedPreferenceChangeListener(settingsListener);
        createNotificationChannel();
    }

    @Override public int onStartCommand(Intent intent, int flags, int startId) {
        String action = intent == null ? ACTION_START : intent.getAction();
        if (ACTION_STOP.equals(action)) {
            setRunningState(false);
            unregisterSensors();
            stopSelf();
            return START_NOT_STICKY;
        }
        if (!canDrawOverlay()) {
            setRunningState(false);
            unregisterSensors();
            stopSelf();
            return START_NOT_STICKY;
        }
        boolean startedFromBoot = intent != null
                && intent.getBooleanExtra(EXTRA_STARTED_FROM_BOOT, false);
        try {
            // BOOT_COMPLETED is its own background-start exemption. Enter foreground immediately
            // on that path so Surface/GPU initialization cannot consume the promotion deadline.
            if (startedFromBoot) startForegroundCompat();
            showOverlayIfNeeded();
            if (!startedFromBoot) startForegroundCompat();
            registerSensors();
        } catch (RuntimeException error) {
            setRunningState(false);
            unregisterSensors();
            stopForeground(true);
            stopSelf();
            return START_NOT_STICKY;
        }
        setRunningState(overlayView != null);
        return START_STICKY;
    }

    @Override public void onDestroy() {
        setRunningState(false);
        FushiSettings.preferences(this).unregisterOnSharedPreferenceChangeListener(settingsListener);
        unregisterSensors();
        removeFrameCallback();
        removeOverlayView();
        super.onDestroy();
    }

    @Override public IBinder onBind(Intent intent) { return null; }

    public static boolean isRunning() {
        return running;
    }

    @Override public void onSensorChanged(SensorEvent event) {
        FushiOverlayView view = overlayView;
        if (!sensorsRegistered || view == null || event == null || event.sensor == null) return;

        int sensorType = event.sensor.getType();
        if (rawAccelerometerFallback
                ? sensorType != Sensor.TYPE_ACCELEROMETER
                : sensorType != Sensor.TYPE_LINEAR_ACCELERATION
                        && sensorType != Sensor.TYPE_GRAVITY) {
            // A callback queued before a partial pair registration was rolled back must not
            // contaminate the raw-accelerometer stream (or vice versa).
            return;
        }

        int sensorKind;
        switch (sensorType) {
            case Sensor.TYPE_LINEAR_ACCELERATION:
                sensorKind = FushiOverlayView.MOTION_SENSOR_LINEAR_ACCELERATION;
                break;
            case Sensor.TYPE_GRAVITY:
                sensorKind = FushiOverlayView.MOTION_SENSOR_GRAVITY;
                break;
            case Sensor.TYPE_ACCELEROMETER:
                sensorKind = FushiOverlayView.MOTION_SENSOR_RAW_ACCELEROMETER;
                break;
            default:
                return;
        }

        int rotation = displayRotation();
        if (rotation != lastDisplayRotation) {
            lastDisplayRotation = rotation;
            view.resetMotion();
        }
        float x = event.values.length > 0 ? event.values[0] : 0f;
        float y = event.values.length > 1 ? event.values[1] : 0f;
        float z = event.values.length > 2 ? event.values[2] : 0f;
        view.motionSample(sensorKind, x, y, z, event.timestamp, rotation);
    }

    @Override public void onAccuracyChanged(Sensor sensor, int accuracy) {}

    private void setRunningState(boolean value) {
        running = value;
        Intent state = new Intent(ACTION_STATE_CHANGED)
                .setPackage(getPackageName())
                .putExtra(EXTRA_RUNNING, value);
        sendBroadcast(state);
    }

    private void doFrame(long frameTimeNanos) {
        framePosted = false;
        if (overlayView == null || layoutParams == null || windowManager == null) return;

        int rotation = displayRotation();
        if (rotation != lastDisplayRotation) {
            lastDisplayRotation = rotation;
            overlayView.resetMotion();
        }
        if (lastFrameNs != 0L
                && frameTimeNanos - lastFrameNs > MOTION_DISCONTINUITY_NS) {
            // Do not replay a confirmed impulse after screen-off or a long UI/frame pause.
            overlayView.resetMotion();
        }
        float dt = lastFrameNs == 0L
                ? 1f / 60f
                : clamp((frameTimeNanos - lastFrameNs) / 1_000_000_000f, 0.001f, 0.050f);
        lastFrameNs = frameTimeNanos;
        DisplayGeometry geometry = displayGeometry();

        // Apply the origin of the latest frame before requesting another. Rust renders every
        // buffer around the same local anchor and keeps the surface size fixed for this preset, so
        // this update is position-only and cannot resize/crop the ANativeWindow.
        if (updateOverlayLayout(geometry)) {
            try {
                windowManager.updateViewLayout(overlayView, layoutParams);
            } catch (RuntimeException ignored) {
                // The view can disappear during service shutdown.
            }
        }
        overlayView.step(
                dt,
                geometry.width,
                geometry.height,
                geometry.workLeft,
                geometry.workTop,
                geometry.workRight,
                geometry.workBottom);
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
        DisplayGeometry geometry = displayGeometry();

        overlayView = new FushiOverlayView(
                this,
                FushiSettings.graphicsBackend(this),
                FushiSettings.sizePreset(this));
        overlayView.setMotionSensorMode(motionSensorMode);
        Display.Mode preferredMode = preferredDisplayMode();
        if (preferredMode != null) {
            overlayView.setPreferredFrameRate(preferredMode.getRefreshRate());
        }
        overlayView.setHost(() -> stopSelf());

        // Rust computes the preset's fixed square surface and the initial rendered world origin.
        // Resolve that layout before addView so the first ANativeWindow already has its final size.
        overlayView.step(
                1f / 60f,
                geometry.width,
                geometry.height,
                geometry.workLeft,
                geometry.workTop,
                geometry.workRight,
                geometry.workBottom);
        overlayView.resetMotion();

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
        if (preferredMode != null) {
            layoutParams.preferredDisplayModeId = preferredMode.getModeId();
            layoutParams.preferredRefreshRate = preferredMode.getRefreshRate();
        }
        updateOverlayLayout(geometry);
        windowManager.addView(overlayView, layoutParams);
        lastFrameNs = 0L;
        postFrameCallback();
    }

    private void applySizeSetting() {
        if (overlayView == null) return;
        // Window dimensions are fixed for the lifetime of a native renderer. Recreate so a preset
        // change cannot render one frame into the previous preset's Surface dimensions.
        recreateOverlay();
    }

    private void recreateOverlay() {
        if (overlayView == null) return;
        removeFrameCallback();
        removeOverlayView();
        showOverlayIfNeeded();
    }

    private void removeOverlayView() {
        FushiOverlayView view = overlayView;
        overlayView = null;
        layoutParams = null;
        if (view == null) return;

        // Drop the wgpu surface while SurfaceHolder is still valid.
        view.resetMotion();
        view.destroyNative();
        if (windowManager != null) {
            try {
                windowManager.removeView(view);
            } catch (RuntimeException ignored) {
            }
        }
    }

    private boolean canDrawOverlay() {
        return Build.VERSION.SDK_INT < Build.VERSION_CODES.M || Settings.canDrawOverlays(this);
    }

    private void registerSensors() {
        if (sensorsRegistered || sensorManager == null || overlayView == null) return;

        setMotionSensorMode(FushiOverlayView.MOTION_SENSOR_MODE_NONE);
        overlayView.resetMotion();
        lastDisplayRotation = displayRotation();

        Sensor linear = sensorManager.getDefaultSensor(Sensor.TYPE_LINEAR_ACCELERATION);
        Sensor gravity = sensorManager.getDefaultSensor(Sensor.TYPE_GRAVITY);
        if (linear != null && gravity != null) {
            boolean linearRegistered = sensorManager.registerListener(
                    this, linear, SensorManager.SENSOR_DELAY_GAME);
            boolean gravityRegistered = sensorManager.registerListener(
                    this, gravity, SensorManager.SENSOR_DELAY_GAME);
            if (linearRegistered && gravityRegistered) {
                rawAccelerometerFallback = false;
                setMotionSensorMode(FushiOverlayView.MOTION_SENSOR_MODE_DIRECT_PAIR);
                sensorsRegistered = true;
                return;
            }
            // Do not mix a partial software-sensor stream with raw acceleration. Rust owns one
            // coherent filtering path at a time.
            sensorManager.unregisterListener(this);
        }

        Sensor accelerometer = sensorManager.getDefaultSensor(Sensor.TYPE_ACCELEROMETER);
        if (accelerometer != null) {
            boolean registered = sensorManager.registerListener(
                    this, accelerometer, SensorManager.SENSOR_DELAY_GAME);
            if (registered) {
                rawAccelerometerFallback = true;
                setMotionSensorMode(FushiOverlayView.MOTION_SENSOR_MODE_RAW_ACCELEROMETER);
                sensorsRegistered = true;
            }
        }
    }

    private void unregisterSensors() {
        if (sensorManager != null) {
            // Also clears a partial two-sensor registration if registration failed midway.
            sensorManager.unregisterListener(this);
        }
        sensorsRegistered = false;
        rawAccelerometerFallback = false;
        setMotionSensorMode(FushiOverlayView.MOTION_SENSOR_MODE_NONE);
        lastDisplayRotation = -1;
        if (overlayView != null) overlayView.resetMotion();
    }

    private void setMotionSensorMode(int mode) {
        motionSensorMode = mode;
        if (overlayView != null) overlayView.setMotionSensorMode(mode);
    }

    private int displayRotation() {
        Display display = windowManager == null ? null : windowManager.getDefaultDisplay();
        if (display == null) return Surface.ROTATION_0;
        int rotation = display.getRotation();
        switch (rotation) {
            case Surface.ROTATION_90:
            case Surface.ROTATION_180:
            case Surface.ROTATION_270:
                return rotation;
            case Surface.ROTATION_0:
            default:
                return Surface.ROTATION_0;
        }
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

    private boolean updateOverlayLayout(DisplayGeometry geometry) {
        if (overlayView == null || layoutParams == null) return false;
        int width = Math.max(MIN_WINDOW_PX, Math.round(overlayView.getWindowWidth()));
        int height = Math.max(MIN_WINDOW_PX, Math.round(overlayView.getWindowHeight()));

        // START|TOP coordinates are relative to WindowManager's work area. Keep Rust's exact
        // rendered world origin, including negative values at a wall; FLAG_LAYOUT_NO_LIMITS makes
        // those coordinates legal and avoids one-sided clamping/cropping.
        int x = Math.round(overlayView.getWindowX()) - geometry.workLeft;
        int y = Math.round(overlayView.getWindowY()) - geometry.workTop;

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

    private DisplayGeometry displayGeometry() {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.R && windowManager != null) {
            WindowMetrics metrics = windowManager.getCurrentWindowMetrics();
            Rect bounds = metrics.getBounds();
            Insets safe = metrics.getWindowInsets().getInsetsIgnoringVisibility(
                    WindowInsets.Type.systemBars() | WindowInsets.Type.displayCutout());
            int width = Math.max(1, bounds.width());
            int height = Math.max(1, bounds.height());
            return new DisplayGeometry(
                    width,
                    height,
                    clampInt(safe.left, 0, width - 1),
                    clampInt(safe.top, 0, height - 1),
                    clampInt(width - safe.right, 1, width),
                    clampInt(height - safe.bottom, 1, height));
        }

        DisplayMetrics metrics = new DisplayMetrics();
        Display display = windowManager == null ? null : windowManager.getDefaultDisplay();
        if (display != null) {
            display.getRealMetrics(metrics);
        } else {
            metrics.setTo(getResources().getDisplayMetrics());
        }
        int width = Math.max(1, metrics.widthPixels);
        int height = Math.max(1, metrics.heightPixels);
        int statusId = getResources().getIdentifier("status_bar_height", "dimen", "android");
        int navigationId = getResources().getIdentifier("navigation_bar_height", "dimen", "android");
        int top = statusId > 0 ? getResources().getDimensionPixelSize(statusId) : 0;
        int bottom = navigationId > 0 ? getResources().getDimensionPixelSize(navigationId) : 0;
        return new DisplayGeometry(width, height, 0, top, width, Math.max(top + 1, height - bottom));
    }

    private Display.Mode preferredDisplayMode() {
        if (windowManager == null) return null;
        Display display = windowManager.getDefaultDisplay();
        if (display == null) return null;
        Display.Mode current = display.getMode();
        Display.Mode best = current;
        for (Display.Mode candidate : display.getSupportedModes()) {
            if (candidate.getPhysicalWidth() == current.getPhysicalWidth()
                    && candidate.getPhysicalHeight() == current.getPhysicalHeight()
                    && candidate.getRefreshRate() > best.getRefreshRate()) {
                best = candidate;
            }
        }
        return best;
    }

    private static int clampInt(int value, int lo, int hi) {
        return Math.max(lo, Math.min(hi, value));
    }

    private static float clamp(float value, float lo, float hi) {
        return Math.max(lo, Math.min(hi, value));
    }
}
