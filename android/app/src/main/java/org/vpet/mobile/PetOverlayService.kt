package org.vpet.mobile

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
import android.graphics.Color
import android.graphics.PixelFormat
import android.graphics.Typeface
import android.graphics.drawable.GradientDrawable
import android.os.Build
import android.os.Handler
import android.os.IBinder
import android.os.Looper
import android.os.SystemClock
import android.view.Gravity
import android.view.MotionEvent
import android.view.View
import android.view.WindowManager
import android.widget.Button
import android.widget.EditText
import android.widget.LinearLayout
import android.widget.ScrollView
import android.widget.TextView
import androidx.core.app.NotificationCompat
import org.json.JSONObject
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

    private var activeMenuView: View? = null

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

            handler.postDelayed(this, 33)
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

        val savedX = prefs.getInt("pet_pos_x", 120)
        val savedY = prefs.getInt("pet_pos_y", 350)

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
            onDragStart = {
                dismissMenu()
            },
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
                showFloatingMenu()
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
                showFloatingMenu(defaultTab = "chat")
            }
            ACTION_STOP -> {
                stopSelf()
            }
        }
        return START_STICKY
    }

    override fun onTaskRemoved(rootIntent: Intent?) {
        // Keep running as a floating companion even when user closes MainActivity from recents
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Desktop-Style Floating Menu (Like PC VPet Right-Click Menu)
    // ─────────────────────────────────────────────────────────────────────────

    private fun dismissMenu() {
        activeMenuView?.let { menu ->
            try {
                windowManager?.removeView(menu)
            } catch (_: Exception) {}
            activeMenuView = null
        }
    }

    private fun showFloatingMenu(defaultTab: String = "chat") {
        dismissMenu()

        val wm = windowManager ?: return
        val petParams = layoutParams ?: return

        val stateJson = if (enginePtr != 0L) PetBridge.nativeGetStateJson(enginePtr) else "{}"
        val petState = try {
            JSONObject(stateJson)
        } catch (_: Exception) {
            JSONObject()
        }

        val petName = petState.optString("name", "Loris")
        val petLevel = petState.optInt("level", 1)
        val petMode = petState.optString("mode", "Normal")
        val isSleeping = petState.optBoolean("sleeping", false)

        val cardRoot = LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            setPadding(32, 28, 32, 28)
            background = GradientDrawable().apply {
                setColor(0xFFFFFFFF.toInt())
                cornerRadius = 36f
                setStroke(3, 0xFF0288D1.toInt())
            }
            elevation = 24f
        }

        // 1. Header (Pet status & Close button)
        val header = LinearLayout(this).apply {
            orientation = LinearLayout.HORIZONTAL
            gravity = Gravity.CENTER_VERTICAL
            setPadding(0, 0, 0, 16)
        }

        val tvTitle = TextView(this).apply {
            text = "$petName • Lv.$petLevel ($petMode)"
            textSize = 15f
            setTypeface(null, Typeface.BOLD)
            setTextColor(0xFF0F172A.toInt())
            layoutParams = LinearLayout.LayoutParams(0, LinearLayout.LayoutParams.WRAP_CONTENT, 1f)
        }
        header.addView(tvTitle)

        val btnClose = TextView(this).apply {
            text = "✕"
            textSize = 18f
            setTypeface(null, Typeface.BOLD)
            setTextColor(0xFF64748B.toInt())
            setPadding(16, 0, 8, 0)
            setOnClickListener { dismissMenu() }
        }
        header.addView(btnClose)
        cardRoot.addView(header)

        // 2. Action Tabs Bar
        val tabsRow = LinearLayout(this).apply {
            orientation = LinearLayout.HORIZONTAL
            gravity = Gravity.CENTER
            setPadding(0, 0, 0, 16)
        }

        val subContentPanel = LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
        }

        fun createTabBtn(label: String, onClick: () -> Unit): Button {
            return Button(this).apply {
                text = label
                textSize = 12f
                isAllCaps = false
                setTextColor(0xFF0288D1.toInt())
                background = GradientDrawable().apply {
                    setColor(0xFFF0F9FF.toInt())
                    cornerRadius = 20f
                }
                layoutParams = LinearLayout.LayoutParams(0, LinearLayout.LayoutParams.WRAP_CONTENT, 1f).apply {
                    setMargins(6, 0, 6, 0)
                }
                setOnClickListener { onClick() }
            }
        }

        // Subpanel view renderers
        fun showChatTab() {
            subContentPanel.removeAllViews()

            val etChat = EditText(this).apply {
                hint = "Chat with $petName..."
                textSize = 14f
                setTextColor(0xFF0F172A.toInt())
                setHintTextColor(0xFF94A3B8.toInt())
                background = GradientDrawable().apply {
                    setColor(0xFFF8FAFC.toInt())
                    cornerRadius = 18f
                    setStroke(2, 0xFFE2E8F0.toInt())
                }
                setPadding(24, 20, 24, 20)
            }
            subContentPanel.addView(etChat)

            val btnSend = Button(this).apply {
                text = "💬 Send to AI"
                textSize = 13f
                isAllCaps = false
                setTextColor(0xFFFFFFFF.toInt())
                background = GradientDrawable().apply {
                    setColor(0xFF0288D1.toInt())
                    cornerRadius = 20f
                }
                layoutParams = LinearLayout.LayoutParams(
                    LinearLayout.LayoutParams.MATCH_PARENT,
                    LinearLayout.LayoutParams.WRAP_CONTENT
                ).apply {
                    setMargins(0, 16, 0, 0)
                }
                setOnClickListener {
                    val prompt = etChat.text.toString().trim()
                    if (prompt.isNotEmpty() && enginePtr != 0L) {
                        PetBridge.nativeAskAi(enginePtr, prompt)
                        dismissMenu()
                    }
                }
            }
            subContentPanel.addView(btnSend)

            // Quick suggestion chips
            val chipsRow = LinearLayout(this).apply {
                orientation = LinearLayout.HORIZONTAL
                setPadding(0, 16, 0, 0)
            }
            for (chipText in arrayOf("Hello!", "How are you?", "Tell a joke")) {
                val chip = TextView(this).apply {
                    text = chipText
                    textSize = 11f
                    setTextColor(0xFF0288D1.toInt())
                    background = GradientDrawable().apply {
                        setColor(0xFFE0F2FE.toInt())
                        cornerRadius = 16f
                    }
                    setPadding(18, 8, 18, 8)
                    layoutParams = LinearLayout.LayoutParams(
                        LinearLayout.LayoutParams.WRAP_CONTENT,
                        LinearLayout.LayoutParams.WRAP_CONTENT
                    ).apply { setMargins(0, 0, 12, 0) }
                    setOnClickListener {
                        if (enginePtr != 0L) {
                            PetBridge.nativeAskAi(enginePtr, chipText)
                            dismissMenu()
                        }
                    }
                }
                chipsRow.addView(chip)
            }
            subContentPanel.addView(chipsRow)
        }

        fun showFeedTab() {
            subContentPanel.removeAllViews()
            val tvPrompt = TextView(this).apply {
                text = "Feed items & drinks:"
                textSize = 13f
                setTextColor(0xFF64748B.toInt())
                setPadding(0, 0, 0, 12)
            }
            subContentPanel.addView(tvPrompt)

            val foods = arrayOf(
                "Snack" to "🍪 Snack",
                "Fruit Juice" to "🧃 Fruit Juice",
                "1L Pure Water" to "💧 1L Pure Water",
                "Sandwich" to "🥪 Sandwich"
            )

            val grid = LinearLayout(this).apply {
                orientation = LinearLayout.HORIZONTAL
            }

            for ((foodKey, displayLabel) in foods) {
                val btn = Button(this).apply {
                    text = displayLabel
                    textSize = 11f
                    isAllCaps = false
                    setTextColor(0xFF0F172A.toInt())
                    background = GradientDrawable().apply {
                        setColor(0xFFF1F5F9.toInt())
                        cornerRadius = 18f
                    }
                    layoutParams = LinearLayout.LayoutParams(0, LinearLayout.LayoutParams.WRAP_CONTENT, 1f).apply {
                        setMargins(4, 0, 4, 0)
                    }
                    setOnClickListener {
                        if (enginePtr != 0L) {
                            PetBridge.nativeFeed(enginePtr, foodKey)
                            dismissMenu()
                        }
                    }
                }
                grid.addView(btn)
            }
            subContentPanel.addView(grid)
        }

        fun showWorkTab() {
            subContentPanel.removeAllViews()
            val tvPrompt = TextView(this).apply {
                text = "Work / Study / Play (PC System):"
                textSize = 13f
                setTextColor(0xFF64748B.toInt())
                setPadding(0, 0, 0, 12)
            }
            subContentPanel.addView(tvPrompt)

            val works = arrayOf(
                "Copywriting" to "✍️ Copywrite (Work)",
                "Learning" to "📚 Study (Learning)",
                "Gaming" to "🎮 Gaming (Play)",
                "Cancel" to "🛑 Take a Break"
            )

            for ((workKey, displayLabel) in works) {
                val btn = Button(this).apply {
                    text = displayLabel
                    textSize = 12f
                    isAllCaps = false
                    setTextColor(if (workKey == "Cancel") 0xFFD32F2F.toInt() else 0xFF0288D1.toInt())
                    background = GradientDrawable().apply {
                        setColor(0xFFF8FAFC.toInt())
                        cornerRadius = 16f
                        setStroke(1, 0xFFE2E8F0.toInt())
                    }
                    layoutParams = LinearLayout.LayoutParams(
                        LinearLayout.LayoutParams.MATCH_PARENT,
                        LinearLayout.LayoutParams.WRAP_CONTENT
                    ).apply { setMargins(0, 0, 0, 8) }
                    setOnClickListener {
                        if (enginePtr != 0L) {
                            if (workKey == "Cancel") {
                                PetBridge.nativeCancelWork(enginePtr)
                            } else {
                                PetBridge.nativeStartWork(enginePtr, workKey)
                            }
                            dismissMenu()
                        }
                    }
                }
                subContentPanel.addView(btn)
            }
        }

        tabsRow.addView(createTabBtn("💬 Chat") { showChatTab() })
        tabsRow.addView(createTabBtn("🍽️ Feed") { showFeedTab() })
        tabsRow.addView(createTabBtn("💼 Work") { showWorkTab() })
        tabsRow.addView(createTabBtn(if (isSleeping) "☀️ Wake" else "💤 Sleep") {
            if (enginePtr != 0L) {
                PetBridge.nativeToggleSleep(enginePtr)
                dismissMenu()
            }
        })
        tabsRow.addView(createTabBtn("❌ Close") {
            stopSelf()
        })

        cardRoot.addView(tabsRow)
        cardRoot.addView(subContentPanel)

        when (defaultTab) {
            "feed" -> showFeedTab()
            "work" -> showWorkTab()
            else -> showChatTab()
        }

        val overlayType = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
            WindowManager.LayoutParams.TYPE_APPLICATION_OVERLAY
        } else {
            @Suppress("DEPRECATION")
            WindowManager.LayoutParams.TYPE_PHONE
        }

        val dm = resources.displayMetrics
        val screenW = dm.widthPixels
        val menuWidth = (screenW * 0.88).toInt().coerceAtMost(1000)

        val menuX = (petParams.x - 20).coerceIn(20, screenW - menuWidth - 20)
        val menuY = (petParams.y - 420).coerceAtLeast(80)

        val menuParams = WindowManager.LayoutParams(
            menuWidth,
            WindowManager.LayoutParams.WRAP_CONTENT,
            overlayType,
            WindowManager.LayoutParams.FLAG_NOT_TOUCH_MODAL or
                WindowManager.LayoutParams.FLAG_WATCH_OUTSIDE_TOUCH,
            PixelFormat.TRANSLUCENT
        ).apply {
            gravity = Gravity.TOP or Gravity.START
            x = menuX
            y = menuY
        }

        cardRoot.setOnTouchListener { _, event ->
            if (event.action == MotionEvent.ACTION_OUTSIDE) {
                dismissMenu()
                true
            } else {
                false
            }
        }

        wm.addView(cardRoot, menuParams)
        activeMenuView = cardRoot
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

        dismissMenu()

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
