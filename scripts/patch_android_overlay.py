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
package_dir = ROOT / Path(*package.split("."))
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
import android.graphics.Color
import android.graphics.PixelFormat
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
  private var lastX = 0
  private var lastY = 0
  private var touchX = 0f
  private var touchY = 0f
  private lateinit var title: TextView
  private lateinit var stats: TextView

  override fun onCreate() {{
    super.onCreate()
    startForeground(4217, notification())
    if (Settings.canDrawOverlays(this)) showOverlay()
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
    val box = LinearLayout(this)
    box.orientation = LinearLayout.VERTICAL
    box.setPadding(18, 12, 18, 12)
    box.setBackgroundColor(Color.argb(225, 20, 18, 24))
    title = TextView(this)
    title.setTextColor(Color.WHITE)
    title.textSize = 12f
    stats = TextView(this)
    stats.setTextColor(Color.rgb(255, 195, 91))
    stats.textSize = 14f
    box.addView(title)
    box.addView(stats)
    box.setOnTouchListener {{ _, event ->
      when (event.action) {{
        MotionEvent.ACTION_DOWN -> {{ lastX = event.rawX.toInt(); lastY = event.rawY.toInt(); touchX = event.rawX; touchY = event.rawY; true }}
        MotionEvent.ACTION_MOVE -> {{
          val p = overlay?.layoutParams as? WindowManager.LayoutParams ?: return@setOnTouchListener true
          p.x += event.rawX.toInt() - lastX; p.y += event.rawY.toInt() - lastY
          lastX = event.rawX.toInt(); lastY = event.rawY.toInt(); windowManager.updateViewLayout(box, p); true
        }}
        else -> true
      }}
    }}
    val type = if (Build.VERSION.SDK_INT >= 26) WindowManager.LayoutParams.TYPE_APPLICATION_OVERLAY else WindowManager.LayoutParams.TYPE_PHONE
    val params = WindowManager.LayoutParams(330, 90, type, WindowManager.LayoutParams.FLAG_NOT_FOCUSABLE, PixelFormat.TRANSLUCENT)
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
      title.text = if (current == null) "TLI Tracker · chờ map" else "TLI Tracker · đang chạy"
      val profit = current?.optDouble("profit", 0.0) ?: 0.0
      val maps = tracker.optJSONArray("history")?.length() ?: 0
      stats.text = "${{String.format("%.0f", profit)}} FE  ·  $maps map"
    }} catch (_: Exception) {{}}
  }}

  private fun findState(): File? = filesDir.walkTopDown().firstOrNull {{ it.isFile && it.name == "state.json" }}

  override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {{
    if (overlay == null && Settings.canDrawOverlays(this)) showOverlay()
    return START_STICKY
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

if activity.suffix == ".kt":
    if "OverlayService" not in text:
        if "import android.os.Bundle" not in text:
            text = text.replace(f"package {package}", f"package {package}\n\nimport android.os.Bundle\nimport android.content.Intent", 1)
        else:
            text = text.replace("import android.os.Bundle", "import android.os.Bundle\nimport android.content.Intent", 1)
        text = text.replace(") : TauriActivity() {", ") : TauriActivity() {\n    override fun onCreate(savedInstanceState: Bundle?) {\n        super.onCreate(savedInstanceState)\n        startService(Intent(this, OverlayService::class.java))\n    }")
        activity.write_text(text)
else:
    raise SystemExit("Java MainActivity is not supported by this patch yet")

manifest = next((ROOT.parent / "main").rglob("AndroidManifest.xml"), None)
if manifest is None:
    raise SystemExit("generated AndroidManifest.xml not found")
manifest_text = manifest.read_text()
if "SYSTEM_ALERT_WINDOW" not in manifest_text:
    manifest_text = manifest_text.replace("<application", '<uses-permission android:name="android.permission.SYSTEM_ALERT_WINDOW" />\n    <uses-permission android:name="android.permission.FOREGROUND_SERVICE" />\n    <uses-permission android:name="android.permission.FOREGROUND_SERVICE_DATA_SYNC" />\n    <application', 1)
if "OverlayService" not in manifest_text:
    manifest_text = manifest_text.replace("</application>", '        <service android:name=".OverlayService" android:exported="false" android:foregroundServiceType="dataSync" />\n    </application>', 1)
manifest.write_text(manifest_text)
