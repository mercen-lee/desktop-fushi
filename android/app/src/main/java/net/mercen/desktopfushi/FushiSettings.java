package net.mercen.desktopfushi;

import android.content.Context;
import android.content.SharedPreferences;
import android.content.pm.PackageManager;

final class FushiSettings {
    static final String PREFERENCES_NAME = "pocket_fushi_settings";
    static final String KEY_SIZE_PRESET = "fushi_size_preset";
    static final String KEY_START_ON_BOOT = "start_on_boot";
    private static final String LEGACY_GRAPHICS_BACKEND_KEY = "graphics_backend";

    static final int BACKEND_VULKAN = 0;
    static final int BACKEND_GLES = 1;

    static final int SIZE_SMALL = 0;
    static final int SIZE_NORMAL = 1;
    static final int SIZE_LARGE = 2;
    static final int SIZE_HUGE = 3;
    static final int SIZE_PRESET_COUNT = 4;

    private static Boolean nativeVulkanAvailable;
    private static boolean vulkanRuntimeFailed;

    private FushiSettings() {}

    static SharedPreferences preferences(Context context) {
        return context.getSharedPreferences(PREFERENCES_NAME, Context.MODE_PRIVATE);
    }

    static int graphicsBackend(Context context) {
        SharedPreferences preferences = preferences(context);
        if (preferences.contains(LEGACY_GRAPHICS_BACKEND_KEY)) {
            // Backend selection is automatic now. Remove a previous manual GLES choice so an
            // upgraded Vulkan-capable device cannot remain pinned to an invisible setting.
            preferences.edit().remove(LEGACY_GRAPHICS_BACKEND_KEY).apply();
        }
        return isVulkanSupported(context) ? BACKEND_VULKAN : BACKEND_GLES;
    }

    static void markVulkanRuntimeFailure() {
        synchronized (FushiSettings.class) {
            vulkanRuntimeFailed = true;
        }
    }

    static int sizePreset(Context context) {
        return clampSizePreset(preferences(context).getInt(KEY_SIZE_PRESET, SIZE_NORMAL));
    }

    static void setSizePreset(Context context, int preset) {
        preferences(context).edit().putInt(KEY_SIZE_PRESET, clampSizePreset(preset)).apply();
    }

    static boolean startOnBoot(Context context) {
        return preferences(context).getBoolean(KEY_START_ON_BOOT, false);
    }

    static void setStartOnBoot(Context context, boolean enabled) {
        preferences(context).edit().putBoolean(KEY_START_ON_BOOT, enabled).apply();
    }

    static int sizeLabelRes(int preset) {
        switch (clampSizePreset(preset)) {
            case SIZE_SMALL:
                return R.string.size_small;
            case SIZE_LARGE:
                return R.string.size_large;
            case SIZE_HUGE:
                return R.string.size_huge;
            default:
                return R.string.size_normal;
        }
    }

    static boolean isVulkanSupported(Context context) {
        PackageManager packageManager = context.getPackageManager();
        if (!packageManager.hasSystemFeature(PackageManager.FEATURE_VULKAN_HARDWARE_VERSION)) {
            return false;
        }
        synchronized (FushiSettings.class) {
            if (vulkanRuntimeFailed) {
                return false;
            }
            if (nativeVulkanAvailable == null) {
                nativeVulkanAvailable = FushiOverlayView.isVulkanSupported();
            }
            return nativeVulkanAvailable;
        }
    }

    private static int clampSizePreset(int preset) {
        return Math.max(SIZE_SMALL, Math.min(SIZE_HUGE, preset));
    }
}
