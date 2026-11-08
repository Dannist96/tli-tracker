"""Add the Android overlay service to Tauri's generated Android project."""

from pathlib import Path
import re


ROOT = Path("src-tauri/gen/android/app/src/main")
activities = list(ROOT.rglob("MainActivity.kt")) + list(ROOT.rglob("MainActivity.java"))
if len(activities) != 1:
    raise SystemExit(f"expected one generated MainActivity, found {len(activities)}")

activity = activities[0]
text = activity.read_text()
match = re.search(r"^\s*package\s+([A-Za-z0-9_.]+)", text, re.MULTILINE)
if not match:
    for candidate in ROOT.rglob("AndroidManifest.xml"):
        manifest_match = re.search(r"\bpackage=\"([A-Za-z0-9_.]+)\"", candidate.read_text())
        if manifest_match:
            match = manifest_match
            break
if not match:
    raise SystemExit("could not determine generated Android package")
package = match.group(1)
# Kotlin source files belong below the generated project's `main/java` tree.
# `ROOT` is `.../app/src/main`, so writing directly below it creates a file
# that Gradle does not compile (and MainActivity then cannot resolve the
# service class).
package_dir = ROOT / "java" / Path(*package.split("."))
package_dir.mkdir(parents=True, exist_ok=True)

service = package_dir / "OverlayService.kt"
service.write_text(
    f'''package {package}

import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.app.Service
import android.content.Intent
import android.content.pm.ServiceInfo
import android.graphics.Color
import android.graphics.PixelFormat
import android.graphics.drawable.GradientDrawable
import android.net.Uri
import android.os.Build
import android.os.Handler
import android.os.IBinder
import android.provider.Settings
import android.view.Gravity
import android.view.MotionEvent
import android.view.View
import android.view.WindowManager
import android.widget.LinearLayout
import android.widget.TextView
import org.json.JSONObject
import java.io.File

class OverlayService : Service() {{
  private val handler = Handler(mainLooper)
  private lateinit var windowManager: WindowManager
  private var overlay: View? = null
  private var expanded = false
  private lateinit var box: LinearLayout
  private var lastX = 0
  private var lastY = 0
  private var touchX = 0f
  private var touchY = 0f
  private lateinit var title: TextView
  private lateinit var stats: TextView
  private lateinit var details: TextView
  private lateinit var footer: TextView

  override fun onCreate() {{
    super.onCreate()
    try {{
      if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q) {{
        startForeground(4217, notification(), ServiceInfo.FOREGROUND_SERVICE_TYPE_DATA_SYNC)
      }} else {{
        startForeground(4217, notification())
      }}
      if (Settings.canDrawOverlays(this)) showOverlay() else stopSelf()
    }} catch (_: Throwable) {{
      // Never crash-loop the host app if an OEM blocks this foreground service.
      stopSelf()
    }}
  }}

  private fun notification(): Notification {{
    val channelId = "tli-overlay"
    val manager = getSystemService(NotificationManager::class.java)
    if (Build.VERSION.SDK_INT >= 26) manager.createNotificationChannel(
      NotificationChannel(channelId, "TLI Tracker overlay", NotificationManager.IMPORTANCE_LOW)
    )
    val settings = Intent(Settings.ACTION_MANAGE_OVERLAY_PERMISSION, Uri.parse("package:$packageName"))
    val pending = PendingIntent.getActivity(this, 1, settings, PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT)
    return if (Build.VERSION.SDK_INT >= 26) Notification.Builder(this, channelId)
      .setSmallIcon(android.R.drawable.ic_menu_info_details)
      .setContentTitle("TLI Tracker")
      .setContentText("Overlay đang chạy; nhấn để cấp quyền nếu chưa thấy HUD")
      .setContentIntent(pending).setOngoing(true).build()
    else Notification.Builder(this).setSmallIcon(android.R.drawable.ic_menu_info_details)
      .setContentTitle("TLI Tracker").setOngoing(true).build()
  }}

  private fun showOverlay() {{
    if (overlay != null) return
    windowManager = getSystemService(WINDOW_SERVICE) as WindowManager
    box = LinearLayout(this)
    box.orientation = LinearLayout.VERTICAL
    box.setPadding(18, 12, 18, 12)
    val background = GradientDrawable()
    background.setColor(Color.argb(238, 14, 13, 16))
    background.cornerRadius = 24f
    background.setStroke(2, Color.argb(80, 255, 122, 61))
    box.background = background
    title = TextView(this)
    title.setTextColor(Color.WHITE)
    title.textSize = 15f
    title.setTypeface(null, android.graphics.Typeface.BOLD)
    stats = TextView(this)
    stats.setTextColor(Color.rgb(79, 224, 160))
    stats.textSize = 28f
    stats.setTypeface(null, android.graphics.Typeface.BOLD)
    details = TextView(this)
    details.setTextColor(Color.LTGRAY)
    details.textSize = 13f
    footer = TextView(this)
    footer.setTextColor(Color.rgb(255, 195, 91))
    footer.textSize = 13f
    stats.visibility = View.GONE
    details.visibility = View.GONE
    footer.visibility = View.GONE
    box.addView(title)
    box.addView(stats)
    box.addView(details)
    box.addView(footer)
    box.setOnClickListener {{
      expanded = !expanded
      val p = overlay?.layoutParams as? WindowManager.LayoutParams ?: return@setOnClickListener
      p.width = if (expanded) 520 else 72
      p.height = if (expanded) 240 else 72
      title.text = if (expanded) title.text else "🔥"
      stats.visibility = if (expanded) View.VISIBLE else View.GONE
      details.visibility = if (expanded) View.VISIBLE else View.GONE
      footer.visibility = if (expanded) View.VISIBLE else View.GONE
      windowManager.updateViewLayout(box, p)
    }}
    val type = if (Build.VERSION.SDK_INT >= 26) WindowManager.LayoutParams.TYPE_APPLICATION_OVERLAY else WindowManager.LayoutParams.TYPE_PHONE
    val params = WindowManager.LayoutParams(72, 72, type, WindowManager.LayoutParams.FLAG_NOT_FOCUSABLE, PixelFormat.TRANSLUCENT)
    params.gravity = Gravity.TOP or Gravity.END
    params.x = 16; params.y = 120
    windowManager.addView(box, params)
    overlay = box
    handler.post(refresh)
  }}

  private val refresh = object : Runnable {{
    override fun run() {{
      readState()
      handler.postDelayed(this, 1500)
    }}
  }}

  private fun readState() {{
    try {{
      val state = findState() ?: return
      val tracker = JSONObject(state.readText()).optJSONObject("tracker") ?: return
      val current = tracker.optJSONObject("current")
      val active = tracker.optString("status", "paused") == "active" && current != null
      title.text = if (!expanded) "🔥" else if (current == null) "TLI TRACKER  ·  WAITING FOR A MAP" else "LIVE  ·  " + current.optString("areaId", "MAP")
      val profit = current?.optDouble("profit", 0.0) ?: 0.0
      val duration = current?.optLong("durationMs", 0L) ?: 0L
      val perHour = if (duration > 0) profit / (duration / 3600000.0) else 0.0
      val session = tracker.optJSONObject("session")
      val sessionProfit = session?.optDouble("profit", 0.0) ?: 0.0
      val maps = session?.optInt("mapCount", 0) ?: 0
      title.setTextColor(if (active) Color.rgb(79, 224, 160) else Color.LTGRAY)
      stats.setTextColor(if (profit >= 0) Color.rgb(79, 224, 160) else Color.rgb(255, 107, 107))
      stats.text = String.format("%+.0f FE", profit)
      details.text = String.format("TIME  %02d:%02d        MAP PROFIT  %+.0f/h", duration / 60000, (duration / 1000) % 60, perHour)
      footer.text = String.format("SESSION  %+.0f FE        MAPS  %d        %s", sessionProfit, maps, if (active) "LIVE" else "PAUSED")
    }} catch (_: Exception) {{}}
  }}

  private fun findState(): File? = filesDir.walkTopDown().firstOrNull {{ it.isFile && it.name == "state.json" }}

  override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {{
    try {{
      if (overlay == null && Settings.canDrawOverlays(this)) showOverlay()
      else if (!Settings.canDrawOverlays(this)) stopSelf()
    }} catch (_: Throwable) {{
      stopSelf()
    }}
    return START_NOT_STICKY
  }}

  override fun onDestroy() {{
    handler.removeCallbacksAndMessages(null)
    overlay?.let {{ windowManager.removeView(it) }}
    overlay = null
    super.onDestroy()
  }}

  override fun onBind(intent: Intent?): IBinder? = null
}}
'''
)

if activity.suffix != ".kt":
    raise SystemExit("Java MainActivity is not supported by this patch yet")
# Do not start the foreground overlay service from Activity.onCreate/onResume.
# Samsung Android can terminate the app during startup when a foreground
# service is started before the user has explicitly enabled overlay access.
# The service and manifest entry remain available for a later safe launcher.
# PiP is safe to enter from the foreground Activity when the user presses Home.
if "onUserLeaveHint" not in text:
    if "import android.os.Build" not in text:
        text = text.replace(f"package {package}", f"package {package}\n\nimport android.app.PictureInPictureParams\nimport android.content.Intent\nimport android.net.Uri\nimport android.os.Build\nimport android.os.Handler\nimport android.os.Looper\nimport android.provider.Settings\nimport android.util.Rational\nimport android.view.View\nimport android.view.ViewGroup\nimport android.webkit.JavascriptInterface\nimport android.webkit.WebView", 1)
    class_end = text.rfind("}")
    if class_end < 0:
        raise SystemExit("could not find MainActivity closing brace")
    pip = """
  private fun findTrackerWebView(view: View): WebView? {
    if (view is WebView) return view
    if (view is ViewGroup) {
      for (index in 0 until view.childCount) {
        findTrackerWebView(view.getChildAt(index))?.let { return it }
      }
    }
    return null
  }

  private val hudRouteHandler = Handler(Looper.getMainLooper())
  private var hudRouteAttempts = 0

  private fun switchToCompactHud() {
    val webView = findTrackerWebView(window.decorView)
    if (webView == null) {
      if (hudRouteAttempts++ < 20) {
        hudRouteHandler.postDelayed({ switchToCompactHud() }, 75)
      }
      return
    }
    hudRouteAttempts = 0
    webView.post(object : Runnable {
      override fun run() {
        webView.evaluateJavascript(
          "window.location.hash = '#/overlay';",
          null
        )
      }
    })
  }

  private var overlayBridgeInstalled = false

  private fun installOverlayBridge() {
    val webView = findTrackerWebView(window.decorView)
    if (webView == null) {
      hudRouteHandler.postDelayed({ installOverlayBridge() }, 200)
      return
    }
    if (overlayBridgeInstalled) return
    webView.addJavascriptInterface(object {
      @JavascriptInterface
      fun toggle() {
        hudRouteHandler.post {
          try {
            if (!Settings.canDrawOverlays(this@MainActivity)) {
              startActivity(Intent(Settings.ACTION_MANAGE_OVERLAY_PERMISSION, Uri.parse("package:$packageName")))
              return@post
            }
            val prefs = getSharedPreferences("tli-overlay", MODE_PRIVATE)
            val enabled = prefs.getBoolean("enabled", false)
            val serviceIntent = Intent(this@MainActivity, OverlayService::class.java)
            if (enabled) {
              stopService(serviceIntent)
            } else if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
              startForegroundService(serviceIntent)
            } else {
              startService(serviceIntent)
            }
            prefs.edit().putBoolean("enabled", !enabled).apply()
          } catch (_: Throwable) {
            getSharedPreferences("tli-overlay", MODE_PRIVATE).edit().putBoolean("enabled", false).apply()
          }
        }
      }
    }, "TLIOverlay")
    overlayBridgeInstalled = true
  }

  override fun onResume() {
    super.onResume()
    installOverlayBridge()
  }

  override fun onUserLeaveHint() {
    super.onUserLeaveHint()
    // Do not auto-enter PiP. Android overlay service is the supported in-game
    // window; automatic PiP was the source of the old Overview card.
  }

  override fun onPictureInPictureModeChanged(
    isInPictureInPictureMode: Boolean,
    newConfig: android.content.res.Configuration
  ) {
    super.onPictureInPictureModeChanged(isInPictureInPictureMode, newConfig)
    if (isInPictureInPictureMode) {
      // Samsung may resize the WebView after the PiP callback. Re-apply the
      // route after the resize so PiP cannot remain on Overview.
      hudRouteHandler.postDelayed({ switchToCompactHud() }, 100)
      hudRouteHandler.postDelayed({ switchToCompactHud() }, 500)
      hudRouteHandler.postDelayed({ switchToCompactHud() }, 1200)
    }
  }
"""
    text = text[:class_end] + pip + text[class_end:]
    activity.write_text(text)

manifest = next((ROOT.parent / "main").rglob("AndroidManifest.xml"), None)
if manifest is None:
    raise SystemExit("generated AndroidManifest.xml not found")
manifest_text = manifest.read_text()
if "SYSTEM_ALERT_WINDOW" not in manifest_text:
    manifest_text = manifest_text.replace("<application", '<uses-permission android:name="android.permission.SYSTEM_ALERT_WINDOW" />\n    <uses-permission android:name="android.permission.FOREGROUND_SERVICE" />\n    <uses-permission android:name="android.permission.FOREGROUND_SERVICE_DATA_SYNC" />\n    <application', 1)
if "supportsPictureInPicture" not in manifest_text:
    manifest_text = manifest_text.replace("<activity", '<activity android:supportsPictureInPicture="true" android:resizeableActivity="true"', 1)
if "OverlayService" not in manifest_text:
    manifest_text = manifest_text.replace("</application>", '        <service android:name=".OverlayService" android:exported="false" android:foregroundServiceType="dataSync" />\n    </application>', 1)
manifest.write_text(manifest_text)
