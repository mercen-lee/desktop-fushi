package net.mercen.desktopfushi;

import android.Manifest;
import android.app.Activity;
import android.content.ActivityNotFoundException;
import android.content.BroadcastReceiver;
import android.content.Context;
import android.content.Intent;
import android.content.IntentFilter;
import android.content.SharedPreferences;
import android.graphics.Insets;
import android.graphics.Typeface;
import android.graphics.drawable.GradientDrawable;
import android.net.Uri;
import android.os.Build;
import android.os.Bundle;
import android.provider.Settings;
import android.util.TypedValue;
import android.view.Gravity;
import android.view.HapticFeedbackConstants;
import android.view.View;
import android.view.WindowInsets;
import android.widget.ImageView;
import android.widget.LinearLayout;
import android.widget.ScrollView;
import android.widget.SeekBar;
import android.widget.Switch;
import android.widget.TextView;
import android.widget.Toast;

public final class MainActivity extends Activity {
    private static final String STATE_PENDING_OVERLAY_ACTION = "pending_overlay_action";
    private static final int PENDING_OVERLAY_NONE = 0;
    private static final int PENDING_OVERLAY_SHOW_FUSHI = 1;
    private static final int PENDING_OVERLAY_ENABLE_STARTUP = 2;

    private Switch overlaySwitch;
    private TextView overlayStatus;
    private Switch startOnBootSwitch;
    private SeekBar sizeSlider;
    private TextView sizeValue;
    private int pendingOverlayAction;
    private boolean bindingSettings;
    private boolean bindingStartOnBoot;
    private boolean bindingOverlayState;
    private boolean stateReceiverRegistered;

    private final SharedPreferences.OnSharedPreferenceChangeListener settingsListener =
            (preferences, key) -> runOnUiThread(() -> {
                if (FushiSettings.KEY_SIZE_PRESET.equals(key)) {
                    updateSizeSelection();
                } else if (FushiSettings.KEY_START_ON_BOOT.equals(key)) {
                    updateStartOnBootSelection();
                }
            });

    private final BroadcastReceiver overlayStateReceiver = new BroadcastReceiver() {
        @Override public void onReceive(Context context, Intent intent) {
            if (!FushiOverlayService.ACTION_STATE_CHANGED.equals(intent.getAction())) return;
            // Treat the broadcast as an invalidation signal. The in-process service state is the
            // authority, so an app on older Android versions cannot spoof the visible toggle.
            updateOverlayState(FushiOverlayService.isRunning());
        }
    };

    @Override
    protected void onCreate(Bundle savedInstanceState) {
        super.onCreate(savedInstanceState);
        if (savedInstanceState != null) {
            pendingOverlayAction = savedInstanceState.getInt(
                    STATE_PENDING_OVERLAY_ACTION,
                    PENDING_OVERLAY_NONE);
        }

        ScrollView scroll = new ScrollView(this);
        scroll.setFillViewport(true);

        LinearLayout root = new LinearLayout(this);
        root.setOrientation(LinearLayout.VERTICAL);
        root.setGravity(Gravity.CENTER_HORIZONTAL);
        // Elevated cards draw their ambient shadow outside their own bounds. The default
        // LinearLayout clipping cuts that shadow exactly at the padded content edge, which makes
        // both vertical sides look flat. Keep the generous screen padding, but allow child
        // shadows to render into it.
        root.setClipToPadding(false);
        root.setClipChildren(false);
        int pad = dp(24);
        root.setPadding(pad, pad, pad, pad);
        root.setOnApplyWindowInsetsListener((view, insets) -> {
            int left;
            int top;
            int right;
            int bottom;
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.R) {
                Insets safe = insets.getInsets(
                        WindowInsets.Type.systemBars() | WindowInsets.Type.displayCutout());
                left = safe.left;
                top = safe.top;
                right = safe.right;
                bottom = safe.bottom;
            } else {
                left = insets.getSystemWindowInsetLeft();
                top = insets.getSystemWindowInsetTop();
                right = insets.getSystemWindowInsetRight();
                bottom = insets.getSystemWindowInsetBottom();
            }
            view.setPadding(pad + left, pad + top, pad + right, pad + bottom);
            return insets;
        });

        ImageView appIcon = new ImageView(this);
        appIcon.setImageDrawable(getApplicationInfo().loadIcon(getPackageManager()));
        appIcon.setScaleType(ImageView.ScaleType.FIT_CENTER);
        appIcon.setImportantForAccessibility(View.IMPORTANT_FOR_ACCESSIBILITY_NO);
        LinearLayout.LayoutParams iconParams = new LinearLayout.LayoutParams(dp(72), dp(72));
        iconParams.gravity = Gravity.CENTER_HORIZONTAL;
        iconParams.bottomMargin = dp(12);
        root.addView(appIcon, iconParams);

        TextView title = new TextView(this);
        title.setText(getString(
                R.string.app_title_with_version,
                getString(R.string.app_name),
                getString(R.string.app_version_name)));
        title.setTextSize(26.0f);
        title.setTypeface(Typeface.DEFAULT, Typeface.BOLD);
        title.setGravity(Gravity.CENTER);
        title.setPadding(0, 0, 0, dp(4));
        root.addView(title, matchWrap());

        root.addView(createOverlayCard(), cardLayoutParams());
        root.addView(createStartupCard(), cardLayoutParams());
        root.addView(createSizeCard(), cardLayoutParams());
        root.addView(createCreditsCard(), cardLayoutParams());

        scroll.addView(root, new ScrollView.LayoutParams(
                ScrollView.LayoutParams.MATCH_PARENT,
                ScrollView.LayoutParams.WRAP_CONTENT));
        setContentView(scroll);
        root.requestApplyInsets();
        updateOverlayState(FushiOverlayService.isRunning());
        updateStartOnBootSelection();
        updateSizeSelection();
        requestNotificationPermissionIfNeeded();
    }

    @Override
    protected void onStart() {
        super.onStart();
        registerOverlayStateReceiver();
        FushiSettings.preferences(this).registerOnSharedPreferenceChangeListener(settingsListener);
        updateOverlayState(FushiOverlayService.isRunning());
    }

    @Override
    protected void onResume() {
        super.onResume();
        if (pendingOverlayAction != PENDING_OVERLAY_NONE) {
            int completedAction = pendingOverlayAction;
            pendingOverlayAction = PENDING_OVERLAY_NONE;
            if (Settings.canDrawOverlays(this)) {
                if (completedAction == PENDING_OVERLAY_SHOW_FUSHI) {
                    startOverlayService();
                    updateSizeSelection();
                    return;
                }
                if (completedAction == PENDING_OVERLAY_ENABLE_STARTUP) {
                    FushiSettings.setStartOnBoot(this, true);
                    updateStartOnBootSelection();
                    startOverlayService();
                    updateSizeSelection();
                    return;
                }
            }
            updateStartOnBootSelection();
        }
        if (!Settings.canDrawOverlays(this) && FushiOverlayService.isRunning()) {
            stopOverlayService();
        } else {
            updateOverlayState(FushiOverlayService.isRunning());
        }
        updateSizeSelection();
    }

    @Override
    protected void onStop() {
        FushiSettings.preferences(this).unregisterOnSharedPreferenceChangeListener(settingsListener);
        unregisterOverlayStateReceiver();
        super.onStop();
    }

    @Override
    protected void onSaveInstanceState(Bundle outState) {
        outState.putInt(STATE_PENDING_OVERLAY_ACTION, pendingOverlayAction);
        super.onSaveInstanceState(outState);
    }

    private View createOverlayCard() {
        LinearLayout card = createCard();
        card.addView(sectionTitle(R.string.fushi_overlay), matchWrap());
        card.addView(sectionDescription(R.string.fushi_overlay_description), matchWrap());

        overlaySwitch = new Switch(this);
        overlaySwitch.setText(R.string.show_fushi);
        overlaySwitch.setTextSize(15.0f);
        overlaySwitch.setGravity(Gravity.CENTER_VERTICAL);
        overlaySwitch.setMinHeight(dp(48));
        overlaySwitch.setOnCheckedChangeListener((button, checked) -> {
            if (bindingOverlayState) return;
            if (checked) {
                startOrRequestOverlay();
            } else {
                stopOverlayService();
            }
        });
        card.addView(overlaySwitch, matchWrap());

        overlayStatus = sectionDescription(R.string.fushi_status_stopped);
        overlayStatus.setPadding(0, dp(2), 0, 0);
        card.addView(overlayStatus, matchWrap());
        return card;
    }

    private View createStartupCard() {
        LinearLayout card = createCard();
        card.addView(sectionTitle(R.string.startup), matchWrap());

        startOnBootSwitch = new Switch(this);
        startOnBootSwitch.setText(R.string.start_on_boot);
        startOnBootSwitch.setTextSize(15.0f);
        startOnBootSwitch.setGravity(Gravity.CENTER_VERTICAL);
        startOnBootSwitch.setMinHeight(dp(48));
        startOnBootSwitch.setPadding(0, dp(6), 0, 0);
        startOnBootSwitch.setOnCheckedChangeListener((button, checked) -> {
            if (bindingStartOnBoot) return;
            if (checked && !Settings.canDrawOverlays(this)) {
                updateStartOnBootSelection();
                requestOverlayPermission(PENDING_OVERLAY_ENABLE_STARTUP);
                return;
            }
            FushiSettings.setStartOnBoot(this, checked);
            if (checked && !FushiOverlayService.isRunning()) {
                startOverlayService();
            }
        });
        card.addView(startOnBootSwitch, matchWrap());

        TextView startOnBootDescription = sectionDescription(R.string.start_on_boot_description);
        startOnBootDescription.setPadding(0, dp(2), 0, 0);
        card.addView(startOnBootDescription, matchWrap());
        return card;
    }

    private View createSizeCard() {
        LinearLayout card = createCard();
        card.addView(sectionTitle(R.string.fushi_size), matchWrap());

        sizeValue = new TextView(this);
        sizeValue.setTextSize(15.0f);
        sizeValue.setTypeface(Typeface.DEFAULT, Typeface.BOLD);
        sizeValue.setPadding(0, dp(8), 0, 0);
        card.addView(sizeValue, matchWrap());

        sizeSlider = new SeekBar(this);
        sizeSlider.setMax(FushiSettings.SIZE_PRESET_COUNT - 1);
        sizeSlider.setKeyProgressIncrement(1);
        sizeSlider.setMinHeight(dp(48));
        sizeSlider.setSplitTrack(false);
        sizeSlider.setOnSeekBarChangeListener(new SeekBar.OnSeekBarChangeListener() {
            private int previousProgress = -1;
            private boolean trackingTouch;

            @Override
            public void onProgressChanged(SeekBar seekBar, int progress, boolean fromUser) {
                updateSizeValue(progress);
                if (!fromUser || bindingSettings) return;
                if (progress != previousProgress) {
                    seekBar.performHapticFeedback(HapticFeedbackConstants.CLOCK_TICK);
                    previousProgress = progress;
                }
                // Keyboard and accessibility changes do not start a touch-tracking session.
                if (!trackingTouch) {
                    FushiSettings.setSizePreset(MainActivity.this, progress);
                }
            }

            @Override public void onStartTrackingTouch(SeekBar seekBar) {
                trackingTouch = true;
            }

            @Override public void onStopTrackingTouch(SeekBar seekBar) {
                trackingTouch = false;
                FushiSettings.setSizePreset(MainActivity.this, seekBar.getProgress());
            }
        });
        card.addView(sizeSlider, matchWrap());

        LinearLayout tickLabels = new LinearLayout(this);
        tickLabels.setOrientation(LinearLayout.HORIZONTAL);
        for (int preset = 0; preset < FushiSettings.SIZE_PRESET_COUNT; preset++) {
            TextView label = new TextView(this);
            label.setText(FushiSettings.sizeLabelRes(preset));
            label.setTextSize(12.0f);
            label.setGravity(Gravity.CENTER);
            tickLabels.addView(label, new LinearLayout.LayoutParams(0, dp(40), 1.0f));
        }
        card.addView(tickLabels, matchWrap());
        return card;
    }

    private View createCreditsCard() {
        LinearLayout card = createCard();
        card.addView(sectionTitle(R.string.credits), matchWrap());

        LinearLayout projectLink = new LinearLayout(this);
        projectLink.setOrientation(LinearLayout.VERTICAL);
        projectLink.setGravity(Gravity.CENTER_VERTICAL);
        projectLink.setMinimumHeight(dp(56));
        projectLink.setPadding(0, dp(7), 0, dp(7));
        projectLink.setClickable(true);
        projectLink.setFocusable(true);
        projectLink.setContentDescription(getString(
                R.string.project_website_accessibility,
                getString(R.string.app_version_name),
                getString(R.string.project_website)));
        applySelectableBackground(projectLink);
        projectLink.setOnClickListener(view -> openProjectWebsite());

        TextView version = new TextView(this);
        version.setText(getString(R.string.credit_version, getString(R.string.app_version_name)));
        version.setTextSize(15.0f);
        version.setTypeface(Typeface.DEFAULT, Typeface.BOLD);
        projectLink.addView(version, matchWrap());

        TextView website = sectionDescription(R.string.project_website);
        website.setPadding(0, dp(2), 0, 0);
        projectLink.addView(website, matchWrap());
        card.addView(projectLink, matchWrap());

        TextView developer = sectionDescription(R.string.developer_credit);
        developer.setPadding(0, dp(6), 0, 0);
        card.addView(developer, matchWrap());

        TextView character = sectionDescription(R.string.character_copyright_explanation);
        character.setPadding(0, dp(7), 0, 0);
        card.addView(character, matchWrap());

        TextView source = sectionDescription(R.string.source_copyright);
        source.setPadding(0, dp(7), 0, 0);
        card.addView(source, matchWrap());
        return card;
    }

    private LinearLayout createCard() {
        LinearLayout card = new LinearLayout(this);
        card.setOrientation(LinearLayout.VERTICAL);
        card.setPadding(dp(16), dp(14), dp(16), dp(12));
        GradientDrawable background = new GradientDrawable();
        background.setColor(getColor(R.color.settings_surface));
        background.setCornerRadius(dp(18));
        card.setBackground(background);
        card.setClipToOutline(true);
        card.setElevation(dp(2));
        return card;
    }

    private TextView sectionTitle(int textRes) {
        TextView title = new TextView(this);
        title.setText(textRes);
        title.setTextSize(17.0f);
        title.setTypeface(Typeface.DEFAULT, Typeface.BOLD);
        return title;
    }

    private TextView sectionDescription(int textRes) {
        TextView description = new TextView(this);
        description.setText(textRes);
        description.setTextSize(13.0f);
        description.setLineSpacing(0.0f, 1.12f);
        description.setAlpha(0.72f);
        description.setPadding(0, dp(4), 0, 0);
        return description;
    }

    private void updateSizeSelection() {
        if (sizeSlider == null) return;
        bindingSettings = true;
        int preset = FushiSettings.sizePreset(this);
        sizeSlider.setProgress(preset);
        updateSizeValue(preset);
        bindingSettings = false;
    }

    private void updateStartOnBootSelection() {
        if (startOnBootSwitch == null) return;
        bindingStartOnBoot = true;
        startOnBootSwitch.setChecked(FushiSettings.startOnBoot(this));
        bindingStartOnBoot = false;
    }

    private void updateSizeValue(int preset) {
        if (sizeValue == null || sizeSlider == null) return;
        String label = getString(FushiSettings.sizeLabelRes(preset));
        sizeValue.setText(getString(R.string.selected_size, label));
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.R) {
            sizeSlider.setStateDescription(label);
        }
        sizeSlider.setContentDescription(getString(R.string.fushi_size_accessibility, label));
    }

    private void startOrRequestOverlay() {
        if (!Settings.canDrawOverlays(this)) {
            updateOverlayState(false);
            requestOverlayPermission(PENDING_OVERLAY_SHOW_FUSHI);
            return;
        }
        startOverlayService();
    }

    private void requestOverlayPermission(int pendingAction) {
        pendingOverlayAction = pendingAction;
        Intent intent = new Intent(
                Settings.ACTION_MANAGE_OVERLAY_PERMISSION,
                Uri.parse("package:" + getPackageName()));
        startActivity(intent);
        Toast.makeText(this, R.string.overlay_permission_request, Toast.LENGTH_LONG).show();
    }

    private void startOverlayService() {
        Intent service = new Intent(this, FushiOverlayService.class)
                .setAction(FushiOverlayService.ACTION_START);
        try {
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
                startForegroundService(service);
            } else {
                startService(service);
            }
            setOverlayStartingState();
        } catch (RuntimeException error) {
            updateOverlayState(false);
            Toast.makeText(this, R.string.fushi_start_failed, Toast.LENGTH_LONG).show();
        }
    }

    private void stopOverlayService() {
        pendingOverlayAction = PENDING_OVERLAY_NONE;
        Intent service = new Intent(this, FushiOverlayService.class)
                .setAction(FushiOverlayService.ACTION_STOP);
        stopService(service);
        updateOverlayState(false);
    }

    private void setOverlayStartingState() {
        if (overlaySwitch == null || overlayStatus == null) return;
        bindingOverlayState = true;
        overlaySwitch.setChecked(true);
        bindingOverlayState = false;
        overlayStatus.setText(R.string.fushi_status_starting);
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.R) {
            overlaySwitch.setStateDescription(getString(R.string.starting));
        }
    }

    private void updateOverlayState(boolean running) {
        if (overlaySwitch == null || overlayStatus == null) return;
        boolean permitted = Settings.canDrawOverlays(this);
        boolean checked = permitted && running;
        bindingOverlayState = true;
        overlaySwitch.setChecked(checked);
        bindingOverlayState = false;
        if (!permitted) {
            overlayStatus.setText(R.string.overlay_permission_required);
        } else {
            overlayStatus.setText(checked ? R.string.fushi_status_running : R.string.fushi_status_stopped);
        }
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.R) {
            overlaySwitch.setStateDescription(getString(
                    !permitted
                            ? R.string.overlay_permission_required
                            : (checked ? R.string.running : R.string.stopped)));
        }
    }

    private void registerOverlayStateReceiver() {
        if (stateReceiverRegistered) return;
        IntentFilter filter = new IntentFilter(FushiOverlayService.ACTION_STATE_CHANGED);
        if (Build.VERSION.SDK_INT >= 33) {
            registerReceiver(overlayStateReceiver, filter, Context.RECEIVER_NOT_EXPORTED);
        } else {
            registerReceiver(overlayStateReceiver, filter);
        }
        stateReceiverRegistered = true;
    }

    private void unregisterOverlayStateReceiver() {
        if (!stateReceiverRegistered) return;
        unregisterReceiver(overlayStateReceiver);
        stateReceiverRegistered = false;
    }

    private void openProjectWebsite() {
        try {
            startActivity(new Intent(Intent.ACTION_VIEW, Uri.parse(getString(R.string.project_website))));
        } catch (ActivityNotFoundException error) {
            Toast.makeText(this, R.string.website_open_failed, Toast.LENGTH_LONG).show();
        }
    }

    private void requestNotificationPermissionIfNeeded() {
        if (Build.VERSION.SDK_INT >= 33
                && checkSelfPermission(Manifest.permission.POST_NOTIFICATIONS)
                        != android.content.pm.PackageManager.PERMISSION_GRANTED) {
            requestPermissions(new String[] { Manifest.permission.POST_NOTIFICATIONS }, 10);
        }
    }

    private void applySelectableBackground(View view) {
        TypedValue value = new TypedValue();
        if (getTheme().resolveAttribute(android.R.attr.selectableItemBackground, value, true)
                && value.resourceId != 0) {
            view.setBackgroundResource(value.resourceId);
        }
    }

    private LinearLayout.LayoutParams matchWrap() {
        return new LinearLayout.LayoutParams(
                LinearLayout.LayoutParams.MATCH_PARENT,
                LinearLayout.LayoutParams.WRAP_CONTENT);
    }

    private LinearLayout.LayoutParams cardLayoutParams() {
        LinearLayout.LayoutParams params = matchWrap();
        params.topMargin = dp(12);
        return params;
    }

    private int dp(float value) {
        return Math.round(value * getResources().getDisplayMetrics().density);
    }
}
