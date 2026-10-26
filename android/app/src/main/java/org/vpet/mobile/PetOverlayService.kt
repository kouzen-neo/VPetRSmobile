package org.vpet.mobile

import android.app.AlertDialog
import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.app.Service
import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.content.IntentFilter
import android.content.SharedPreferences
import android.graphics.PixelFormat
import android.os.Build
import android.os.Handler
import android.os.IBinder
import android.os.Looper
import android.os.SystemClock
import android.view.Gravity
import android.view.WindowManager
import android.widget.EditText
import androidx.core.app.NotificationCompat
import java.io.File

class PetOverlayService : Service() {

    companion object {
        const val CHANNEL_ID = "vpet_floating_channel"
        const val NOTIFICATION_ID = 4041

        const val ACTION_FEED = "org.vpet.mobile.ACTION_FEED"
        const val ACTION_SLEEP = "org.vpet.mobile.ACTION_SLEEP"
        const val ACTION_CHAT = "org.vpet.mobile.ACTION_CHAT"
        const val ACTION_STOP = "org.vpet.mobile.ACTION_STOP"
    }

    private var windowManager: WindowManager? = null
    private var petView: PetView? = null
    private var layoutParams: WindowManager.LayoutParams? = null

    private var enginePtr: Long = 0
    private val handler = Handler(Looper.getMainLooper())
    private var isRunning = false
    private var lastFrameTime = 0L

    private lateinit var prefs: SharedPreferences

    private val screenReceiver = object : BroadcastReceiver() {
        override fun onReceive(context: Context?, intent: Intent?) {
            when (intent?.action) {
                Intent.ACTION_SCREEN_OFF -> {
                    isRunning = false
                }
                Intent.ACTION_SCREEN_ON -> {
                    if (!isRunning) {
                        isRunning = true
                        lastFrameTime = SystemClock.uptimeMillis()
                        handler.post(tickRunnable)
                    }
                }
            }
        }
    }

    private val tickRunnable = object : Runnable {
        override fun run() {
            if (!isRunning || enginePtr == 0L || petView == null) return

            val now = SystemClock.uptimeMillis()
            val dt = if (lastFrameTime > 0) ((now - lastFrameTime) / 1000f).coerceIn(0.01f, 0.1f) else 0.033f
            lastFrameTime = now

            val needsRender = PetBridge.nativeTick(enginePtr, dt)
            if (needsRender) {
                petView?.let { view ->
                    PetBridge.nativeRender(enginePtr, view.renderBitmap)
                    view.invalidate()
                }
            }

            handler.postDelayed(this, 33) // ~30 FPS
        }
    }

    override fun onCreate() {
        super.onCreate()
        prefs = getSharedPreferences("vpet_prefs", Context.MODE_PRIVATE)

        createNotificationChannel()
        startForeground(NOTIFICATION_ID, buildNotification())

        val filter = IntentFilter().apply {
            addAction(Intent.ACTION_SCREEN_OFF)
            addAction(Intent.ACTION_SCREEN_ON)
        }
        registerReceiver(screenReceiver, filter)

        setupFloatingPet()
    }

    private fun setupFloatingPet() {
        val targetPx = prefs.getInt("pet_scale_px", 280).coerceIn(160, 500)
        val bubblePadPx = targetPx / 3

        val contentDir = File(filesDir, "content/0000_core")
        val saveDir = File(filesDir, "saves")
        saveDir.mkdirs()

        val aiBase = prefs.getString("ai_base", "http://10.0.2.2:20128/v1") ?: ""
        val aiModel = prefs.getString("ai_model", "9routerall") ?: ""

        enginePtr = PetBridge.nativeInit(
            contentDir.absolutePath,
            saveDir.absolutePath,
            targetPx,
            aiBase,
            aiModel
        )

        windowManager = getSystemService(Context.WINDOW_SERVICE) as WindowManager

        val overlayType = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
            WindowManager.LayoutParams.TYPE_APPLICATION_OVERLAY
        } else {
            @Suppress("DEPRECATION")
            WindowManager.LayoutParams.TYPE_PHONE
        }

        val savedX = prefs.getInt("pet_pos_x", 100)
        val savedY = prefs.getInt("pet_pos_y", 300)

        layoutParams = WindowManager.LayoutParams(
            targetPx,
            targetPx + bubblePadPx,
            overlayType,
            WindowManager.LayoutParams.FLAG_NOT_FOCUSABLE or
                WindowManager.LayoutParams.FLAG_LAYOUT_NO_LIMITS,
            PixelFormat.TRANSLUCENT
        ).apply {
            gravity = Gravity.TOP or Gravity.START
            x = savedX
            y = savedY
        }

        petView = PetView(
            context = this,
            targetPx = targetPx,
            bubblePadPx = bubblePadPx,
            onMoveDelta = { dx, dy ->
                layoutParams?.let { params ->
                    params.x += dx
                    params.y += dy
                    windowManager?.updateViewLayout(petView, params)
                }
            },
            onDragStart = {},
            onDragEnd = {
                layoutParams?.let { params ->
                    prefs.edit()
                        .putInt("pet_pos_x", params.x)
                        .putInt("pet_pos_y", params.y)
                        .apply()
                }
            },
            onActionTouch = { type, x, y ->
                if (enginePtr != 0L) {
                    PetBridge.nativeTouch(enginePtr, type, x, y)
                }
            },
            onLongPressAction = {
                showQuickMenu()
            }
        )

        windowManager?.addView(petView, layoutParams)

        isRunning = true
        lastFrameTime = SystemClock.uptimeMillis()
        handler.post(tickRunnable)
    }

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        when (intent?.action) {
            ACTION_FEED -> {
                if (enginePtr != 0L) {
                    PetBridge.nativeFeed(enginePtr, "Snack")
                }
            }
            ACTION_SLEEP -> {
                if (enginePtr != 0L) {
                    PetBridge.nativeToggleSleep(enginePtr)
                }
            }
            ACTION_CHAT -> {
                showChatPrompt()
            }
            ACTION_STOP -> {
                stopSelf()
            }
        }
        return START_STICKY
    }

    private fun showQuickMenu() {
        val items = arrayOf(
            getString(R.string.action_feed),
            getString(R.string.action_sleep),
            getString(R.string.action_chat),
            getString(R.string.action_stop)
        )

        val dialog = AlertDialog.Builder(this)
            .setTitle(getString(R.string.app_name))
            .setItems(items) { _, which ->
                when (which) {
                    0 -> if (enginePtr != 0L) PetBridge.nativeFeed(enginePtr, "Snack")
                    1 -> if (enginePtr != 0L) PetBridge.nativeToggleSleep(enginePtr)
                    2 -> showChatPrompt()
                    3 -> stopSelf()
                }
            }
            .create()

        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
            dialog.window?.setType(WindowManager.LayoutParams.TYPE_APPLICATION_OVERLAY)
        } else {
            @Suppress("DEPRECATION")
            dialog.window?.setType(WindowManager.LayoutParams.TYPE_PHONE)
        }
        dialog.show()
    }

    private fun showChatPrompt() {
        val input = EditText(this).apply {
            hint = "Ask Loris anything..."
        }

        val dialog = AlertDialog.Builder(this)
            .setTitle(getString(R.string.action_chat))
            .setView(input)
            .setPositiveButton("Send") { _, _ ->
                val text = input.text.toString().trim()
                if (text.isNotEmpty() && enginePtr != 0L) {
                    PetBridge.nativeAskAi(enginePtr, text)
                }
            }
            .setNegativeButton("Cancel", null)
            .create()

        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
            dialog.window?.setType(WindowManager.LayoutParams.TYPE_APPLICATION_OVERLAY)
        } else {
            @Suppress("DEPRECATION")
            dialog.window?.setType(WindowManager.LayoutParams.TYPE_PHONE)
        }
        dialog.show()
    }

    private fun createNotificationChannel() {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
            val channel = NotificationChannel(
                CHANNEL_ID,
                getString(R.string.overlay_service_title),
                NotificationManager.IMPORTANCE_LOW
            ).apply {
                description = getString(R.string.overlay_service_desc)
                setShowBadge(false)
            }
            val nm = getSystemService(NotificationManager::class.java)
            nm.createNotificationChannel(channel)
        }
    }

    private fun buildNotification(): Notification {
        val feedIntent = PendingIntent.getService(
            this, 1, Intent(this, PetOverlayService::class.java).apply { action = ACTION_FEED },
            PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE
        )
        val sleepIntent = PendingIntent.getService(
            this, 2, Intent(this, PetOverlayService::class.java).apply { action = ACTION_SLEEP },
            PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE
        )
        val chatIntent = PendingIntent.getService(
            this, 3, Intent(this, PetOverlayService::class.java).apply { action = ACTION_CHAT },
            PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE
        )
        val stopIntent = PendingIntent.getService(
            this, 4, Intent(this, PetOverlayService::class.java).apply { action = ACTION_STOP },
            PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE
        )

        val mainIntent = PendingIntent.getActivity(
            this, 0, Intent(this, MainActivity::class.java),
            PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE
        )

        return NotificationCompat.Builder(this, CHANNEL_ID)
            .setContentTitle(getString(R.string.overlay_service_title))
            .setContentText(getString(R.string.overlay_service_desc))
            .setSmallIcon(R.mipmap.ic_launcher)
            .setContentIntent(mainIntent)
            .addAction(0, getString(R.string.action_feed), feedIntent)
            .addAction(0, getString(R.string.action_sleep), sleepIntent)
            .addAction(0, getString(R.string.action_chat), chatIntent)
            .addAction(0, getString(R.string.action_stop), stopIntent)
            .setOngoing(true)
            .setPriority(NotificationCompat.PRIORITY_LOW)
            .build()
    }

    override fun onDestroy() {
        isRunning = false
        handler.removeCallbacks(tickRunnable)

        try {
            unregisterReceiver(screenReceiver)
        } catch (_: Exception) {}

        if (petView != null && windowManager != null) {
            windowManager?.removeView(petView)
            petView = null
        }

        if (enginePtr != 0L) {
            PetBridge.nativeDestroy(enginePtr)
            enginePtr = 0L
        }

        super.onDestroy()
    }

    override fun onBind(intent: Intent?): IBinder? = null
}
