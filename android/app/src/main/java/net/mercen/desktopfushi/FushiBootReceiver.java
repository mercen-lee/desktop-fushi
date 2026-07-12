package net.mercen.desktopfushi;

import android.content.BroadcastReceiver;
import android.content.Context;
import android.content.Intent;
import android.os.Build;
import android.provider.Settings;
import android.util.Log;

public final class FushiBootReceiver extends BroadcastReceiver {
    private static final String TAG = "FushiBootReceiver";

    @Override public void onReceive(Context context, Intent intent) {
        if (intent == null || !Intent.ACTION_BOOT_COMPLETED.equals(intent.getAction())) return;
        if (!FushiSettings.startOnBoot(context)
                || !Settings.canDrawOverlays(context)
                || FushiOverlayService.isRunning()) {
            return;
        }

        Intent service = new Intent(context, FushiOverlayService.class)
                .setAction(FushiOverlayService.ACTION_START)
                .putExtra(FushiOverlayService.EXTRA_STARTED_FROM_BOOT, true);
        try {
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
                context.startForegroundService(service);
            } else {
                context.startService(service);
            }
        } catch (RuntimeException error) {
            Log.w(TAG, "Could not start Fushi after boot", error);
        }
    }
}
