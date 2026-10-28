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
import android.widget.Toast
import androidx.core.app.NotificationCompat
import org.json.JSONArray
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
        const val ACTION_SET_SCALE = "org.vpet.mobile.ACTION_SET_SCALE"
        const val ACTION_RELOAD_AI = "org.vpet.mobile.ACTION_RELOAD_AI"
    }

    private var windowManager: WindowManager? = null
    private var petView: PetView? = null
    private var layoutParams: WindowManager.LayoutParams? = null

    private var activeMenuView: View? = null
    private var activeStoreView: View? = null

    private var enginePtr: Long = 0
    private val handler = Handler(Looper.getMainLooper())
    private var isRunning = false
    private var lastFrameTime = 0L
    private var isDraggingPet = false

    private lateinit var prefs: SharedPreferences

    private val screenReceiver = object : BroadcastReceiver() {
        override fun onReceive(context: Context?, intent: Intent?) {
            when (intent?.action) {
                Intent.ACTION_SCREEN_OFF -> {
                    isRunning = false
                    handler.removeCallbacks(tickRunnable)
                    if (enginePtr != 0L) {
                        PetBridge.nativeSave(enginePtr)
                    }
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

            // Sync walking, climbing, and falling position from native engine
            if (!isDraggingPet) {
                val newX = PetBridge.nativeGetPosX(enginePtr)
                val pad = petView?.bubblePadPx ?: 0
                val newY = PetBridge.nativeGetPosY(enginePtr) - pad
                layoutParams?.let { params ->
                    if (params.x != newX || params.y != newY) {
                        params.x = newX
                        params.y = newY
                        windowManager?.updateViewLayout(petView, params)
                    }
                }
            }

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

    private fun extractAssetsSync(coreDir: File) {
        try {
            assets.open("0000_core.zip").use { input ->
                java.util.zip.ZipInputStream(input).use { zip ->
                    coreDir.mkdirs()
                    var entry = zip.nextEntry
                    while (entry != null) {
                        val outFile = File(coreDir, entry.name)
                        if (entry.isDirectory) {
                            outFile.mkdirs()
                        } else {
                            outFile.parentFile?.mkdirs()
                            java.io.FileOutputStream(outFile).use { fos ->
                                zip.copyTo(fos)
                            }
                        }
                        zip.closeEntry()
                        entry = zip.nextEntry
                    }
                }
            }
            prefs.edit().putInt("assets_version", 15).apply()
        } catch (e: Exception) {
            android.util.Log.e("VPetMobile", "Failed to extract assets in service", e)
        }
    }

    private fun setupFloatingPet() {
        val targetPx = prefs.getInt("pet_scale_px", 280).coerceIn(160, 500)
        val bubblePadPx = targetPx / 3

        val contentDir = File(filesDir, "content/0000_core")
        val sampleFood = File(contentDir, "image/food/三明治.png")
        val giftDir = File(contentDir, "pet/vup/Gift")
        if (!sampleFood.exists() || !giftDir.exists() || !File(contentDir, "info.lps").exists()) {
            extractAssetsSync(contentDir)
        }

        val saveDir = File(filesDir, "saves")
        saveDir.mkdirs()

        val aiBase = prefs.getString("ai_base", "http://10.0.2.2:20128/v1") ?: ""
        val aiKey = prefs.getString("ai_key", "") ?: ""
        val aiModel = prefs.getString("ai_model", "9routerall") ?: ""
        val aiPrompt = prefs.getString("ai_prompt", "") ?: ""
        val aiTimeout = prefs.getInt("ai_timeout", 25)

        enginePtr = PetBridge.nativeInit(
            contentDir.absolutePath,
            saveDir.absolutePath,
            targetPx,
            aiBase,
            aiKey,
            aiModel,
            aiPrompt,
            aiTimeout
        )

        windowManager = getSystemService(Context.WINDOW_SERVICE) as WindowManager

        val overlayType = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
            WindowManager.LayoutParams.TYPE_APPLICATION_OVERLAY
        } else {
            @Suppress("DEPRECATION")
            WindowManager.LayoutParams.TYPE_PHONE
        }

        val savedX = prefs.getInt("pet_pos_x", 120)
        val metrics = resources.displayMetrics
        val offsetTop = ((150f * targetPx) / 500f).toInt()
        // Overlay y is the sprite box minus the bubble padding drawn above it.
        val minY = -offsetTop - bubblePadPx
        val maxY = (metrics.heightPixels - targetPx - bubblePadPx).coerceAtLeast(0)
        val savedY = prefs.getInt("pet_pos_y", 350).coerceIn(minY, maxY)

        layoutParams = WindowManager.LayoutParams(
            targetPx,
            targetPx + bubblePadPx,
            overlayType,
            WindowManager.LayoutParams.FLAG_NOT_FOCUSABLE or
                WindowManager.LayoutParams.FLAG_LAYOUT_NO_LIMITS or
                WindowManager.LayoutParams.FLAG_LAYOUT_IN_SCREEN,
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
                    val dm = resources.displayMetrics
                    val pad = petView?.bubblePadPx ?: 0
                    val sprite = petView?.bitmapWidth ?: 280
                    val offsetLeft = ((219f * sprite) / 500f).toInt()
                    val offsetRight = (((500f - 281f) * sprite) / 500f).toInt()
                    val offsetTop = ((150f * sprite) / 500f).toInt()
                    val minX = -offsetLeft
                    val minY = -offsetTop - pad
                    val maxX = dm.widthPixels - sprite + offsetRight
                    val maxY = (dm.heightPixels - sprite - pad).coerceAtLeast(0)
                    params.x = (params.x + dx).coerceIn(minX, maxX)
                    params.y = (params.y + dy).coerceIn(minY, maxY)
                    if (enginePtr != 0L) {
                        // The engine works in sprite space: the bubble strip sits above it.
                        PetBridge.nativeSetPos(enginePtr, params.x, params.y + pad)
                    }
                    windowManager?.updateViewLayout(petView, params)
                }
            },
            onDragStart = {
                isDraggingPet = true
                dismissMenu()
                dismissStore()
            },
            onDragEnd = {
                isDraggingPet = false
                layoutParams?.let { params ->
                    if (enginePtr != 0L) {
                        val pad = petView?.bubblePadPx ?: 0
                        PetBridge.nativeSetPos(enginePtr, params.x, params.y + pad)
                        PetBridge.nativeTouch(enginePtr, "drop", 0f, 0f)
                        val snappedX = PetBridge.nativeGetPosX(enginePtr)
                        val snappedY = PetBridge.nativeGetPosY(enginePtr)
                        params.x = snappedX
                        params.y = snappedY - pad
                        windowManager?.updateViewLayout(petView, params)
                    }
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

        val dm = resources.displayMetrics
        PetBridge.nativeSetScreenBounds(enginePtr, dm.widthPixels, dm.heightPixels)
        PetBridge.nativeSetPos(enginePtr, savedX, savedY + bubblePadPx)
        val autoWalkPref = prefs.getBoolean("pet_auto_walk", true)
        PetBridge.nativeSetAutoWalk(enginePtr, autoWalkPref)

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
            ACTION_SET_SCALE -> {
                val newScale = intent.getIntExtra("scale_px", prefs.getInt("pet_scale_px", 280))
                applyScale(newScale)
            }
            ACTION_RELOAD_AI -> {
                applyAiConfig()
            }
        }
        return START_STICKY
    }

    private fun applyScale(newScale: Int) {
        val targetPx = newScale.coerceIn(160, 500)
        val bubblePadPx = targetPx / 3
        prefs.edit().putInt("pet_scale_px", targetPx).apply()
        if (enginePtr != 0L) {
            PetBridge.nativeSetTargetPx(enginePtr, targetPx)
        }
        val oldPad = petView?.bubblePadPx ?: bubblePadPx
        petView?.updateTargetSize(targetPx)
        layoutParams?.let { params ->
            val spriteTop = params.y + oldPad
            params.width = targetPx
            params.height = targetPx + bubblePadPx
            val dm = resources.displayMetrics
            val offsetLeft = ((219f * targetPx) / 500f).toInt()
            val offsetRight = (((500f - 281f) * targetPx) / 500f).toInt()
            val offsetTop = ((150f * targetPx) / 500f).toInt()
            val minX = -offsetLeft
            val minY = -offsetTop - bubblePadPx
            val maxX = dm.widthPixels - targetPx + offsetRight
            val maxY = (dm.heightPixels - targetPx - bubblePadPx).coerceAtLeast(0)
            params.x = params.x.coerceIn(minX, maxX)
            params.y = (spriteTop - bubblePadPx).coerceIn(minY, maxY)
            if (enginePtr != 0L) {
                PetBridge.nativeSetPos(enginePtr, params.x, params.y + bubblePadPx)
            }
            windowManager?.updateViewLayout(petView, params)
        }
    }

    private fun applyAiConfig() {
        val aiBase = prefs.getString("ai_base", "http://10.0.2.2:20128/v1") ?: ""
        val aiKey = prefs.getString("ai_key", "") ?: ""
        val aiModel = prefs.getString("ai_model", "9routerall") ?: ""
        val aiPrompt = prefs.getString("ai_prompt", "") ?: ""
        val aiTimeout = prefs.getInt("ai_timeout", 25)
        if (enginePtr != 0L) {
            PetBridge.nativeUpdateAiConfig(enginePtr, aiBase, aiKey, aiModel, aiPrompt, aiTimeout)
        }
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

    private fun dismissStore() {
        activeStoreView?.let { store ->
            try {
                windowManager?.removeView(store)
            } catch (_: Exception) {}
            activeStoreView = null
        }
    }

    private fun showFloatingMenu(defaultTab: String = "chat") {
        dismissMenu()
        dismissStore()

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
        val money = petState.optDouble("money", 0.0)

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

        val expCur = petState.optDouble("exp", 0.0).toInt()
        val expNeed = petState.optDouble("exp_need", 100.0).toInt()

        val tvTitle = TextView(this).apply {
            text = "$petName - Lv.$petLevel ($expCur / $expNeed EXP) - $${String.format("%.1f", money)}"
            textSize = 14f
            setTypeface(null, Typeface.BOLD)
            setTextColor(0xFF0F172A.toInt())
            layoutParams = LinearLayout.LayoutParams(0, LinearLayout.LayoutParams.WRAP_CONTENT, 1f)
        }
        header.addView(tvTitle)

        val btnClose = TextView(this).apply {
            text = "X"
            textSize = 18f
            setTypeface(null, Typeface.BOLD)
            setTextColor(0xFF64748B.toInt())
            setPadding(16, 0, 8, 0)
            setOnClickListener { dismissMenu() }
        }
        header.addView(btnClose)
        cardRoot.addView(header)

        val food = petState.optDouble("food", 100.0).toInt()
        val drink = petState.optDouble("drink", 100.0).toInt()
        val health = petState.optDouble("health", 100.0).toInt()

        val tvStats = TextView(this).apply {
            val statusSummary = when {
                isSleeping -> "Status: Sleeping (Tap Wake to wake up)"
                petMode == "Ill" -> "Status: Ill (Low Health! Feed Medicine)"
                petMode == "PoorCondition" -> "Status: Exhausted / Hungry (Feed Snack)"
                else -> "Status: $petMode (Energetic)"
            }
            text = "$statusSummary\nHunger: $food% | Thirst: $drink% | Health: $health%"
            textSize = 12f
            setTextColor(if (petMode == "Ill" || petMode == "PoorCondition") 0xFFDC2626.toInt() else 0xFF475569.toInt())
            setPadding(0, 0, 0, 12)
        }
        cardRoot.addView(tvStats)

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
                text = "Send to AI"
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
            for (chipText in arrayOf("Hello!", "How are you?", "Tell a joke", "unlockall")) {
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
                text = "Quick nourishment:"
                textSize = 13f
                setTextColor(0xFF64748B.toInt())
                setPadding(0, 0, 0, 12)
            }
            subContentPanel.addView(tvPrompt)

            val foods = arrayOf(
                "Snack" to "Snack",
                "Fruit Juice" to "Fruit Juice",
                "Sandwich" to "Sandwich",
                "Medicine" to "Medicine (Heal)"
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

            // Button to open full Food Store dialog!
            val btnOpenStore = Button(this).apply {
                text = "Browse Full Food Store (151 items)"
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
                ).apply { setMargins(0, 16, 0, 0) }
                setOnClickListener {
                    dismissMenu()
                    showFoodStoreDialog()
                }
            }
            subContentPanel.addView(btnOpenStore)
        }

        fun showWorkTab() {
            subContentPanel.removeAllViews()

            val activeWork = petState.optJSONObject("active_work")
            if (activeWork != null) {
                val activeName = activeWork.optString("name", "Work")
                val remSecs = activeWork.optLong("remaining_secs", 0)
                val mins = remSecs / 60
                val secs = remSecs % 60

                val activeCard = LinearLayout(this).apply {
                    orientation = LinearLayout.VERTICAL
                    setPadding(24, 20, 24, 20)
                    background = GradientDrawable().apply {
                        setColor(0xFFF0FDF4.toInt())
                        cornerRadius = 20f
                        setStroke(2, 0xFF86EFAC.toInt())
                    }
                    layoutParams = LinearLayout.LayoutParams(
                        LinearLayout.LayoutParams.MATCH_PARENT,
                        LinearLayout.LayoutParams.WRAP_CONTENT
                    ).apply { setMargins(0, 0, 0, 16) }
                }

                val tvWorkTitle = TextView(this).apply {
                    text = "Active Job: $activeName"
                    textSize = 14f
                    setTypeface(null, Typeface.BOLD)
                    setTextColor(0xFF166534.toInt())
                }
                activeCard.addView(tvWorkTitle)

                val tvCountdown = TextView(this).apply {
                    text = "Remaining: ${mins}m ${secs}s"
                    textSize = 12f
                    setTextColor(0xFF15803D.toInt())
                    setPadding(0, 4, 0, 12)
                }
                activeCard.addView(tvCountdown)

                val btnCancel = Button(this).apply {
                    text = "Take a Break (Cancel)"
                    textSize = 12f
                    isAllCaps = false
                    setTextColor(0xFFDC2626.toInt())
                    background = GradientDrawable().apply {
                        setColor(0xFFFEF2F2.toInt())
                        cornerRadius = 16f
                        setStroke(1, 0xFFFCA5A5.toInt())
                    }
                    setOnClickListener {
                        if (enginePtr != 0L) {
                            PetBridge.nativeCancelWork(enginePtr)
                            dismissMenu()
                        }
                    }
                }
                activeCard.addView(btnCancel)
                subContentPanel.addView(activeCard)
            }

            val tvPrompt = TextView(this).apply {
                text = "Available Work / Study / Play tasks (13 total):"
                textSize = 13f
                setTextColor(0xFF64748B.toInt())
                setPadding(0, 0, 0, 10)
            }
            subContentPanel.addView(tvPrompt)

            val workJson = if (enginePtr != 0L) PetBridge.nativeGetWorkListJson(enginePtr) else "[]"
            val workArray = try {
                JSONArray(workJson)
            } catch (_: Exception) {
                JSONArray()
            }

            val workListContainer = LinearLayout(this).apply {
                orientation = LinearLayout.VERTICAL
            }

            for (i in 0 until workArray.length()) {
                val item = workArray.optJSONObject(i) ?: continue
                val name = item.optString("name", "")
                val kind = item.optString("kind", "Work")
                val minLv = item.optInt("level_limit", 0)
                val timeMins = item.optInt("time_mins", 10)
                val isUnlocked = petLevel >= minLv

                val displayLabel = "$name ($kind - ${timeMins}m)"

                val btn = Button(this).apply {
                    text = if (isUnlocked) {
                        displayLabel
                    } else {
                        "$displayLabel (Req Lv.$minLv)"
                    }
                    textSize = 12f
                    isAllCaps = false
                    isEnabled = isUnlocked
                    setTextColor(if (isUnlocked) 0xFF0288D1.toInt() else 0xFF94A3B8.toInt())
                    background = GradientDrawable().apply {
                        setColor(if (isUnlocked) 0xFFF8FAFC.toInt() else 0xFFF1F5F9.toInt())
                        cornerRadius = 16f
                        setStroke(1, 0xFFE2E8F0.toInt())
                    }
                    layoutParams = LinearLayout.LayoutParams(
                        LinearLayout.LayoutParams.MATCH_PARENT,
                        LinearLayout.LayoutParams.WRAP_CONTENT
                    ).apply { setMargins(0, 0, 0, 8) }
                    setOnClickListener {
                        if (enginePtr != 0L) {
                            PetBridge.nativeStartWork(enginePtr, name)
                            dismissMenu()
                        }
                    }
                }
                workListContainer.addView(btn)
            }

            val workScroll = ScrollView(this).apply {
                layoutParams = LinearLayout.LayoutParams(
                    LinearLayout.LayoutParams.MATCH_PARENT,
                    (resources.displayMetrics.heightPixels * 0.38).toInt()
                )
            }
            workScroll.addView(workListContainer)
            subContentPanel.addView(workScroll)
        }

        tabsRow.addView(createTabBtn("Chat") { showChatTab() })
        tabsRow.addView(createTabBtn("Feed") { showFeedTab() })
        tabsRow.addView(createTabBtn("Work") { showWorkTab() })
        tabsRow.addView(createTabBtn(if (isSleeping) "Wake" else "Sleep") {
            if (enginePtr != 0L) {
                PetBridge.nativeToggleSleep(enginePtr)
                dismissMenu()
            }
        })
        tabsRow.addView(createTabBtn("Close") {
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

    // ─────────────────────────────────────────────────────────────────────────
    // Full Food Store Catalog Dialog (151 items with stats and pricing)
    // ─────────────────────────────────────────────────────────────────────────

    private fun showFoodStoreDialog() {
        dismissStore()
        dismissMenu()

        val wm = windowManager ?: return
        val rawJson = if (enginePtr != 0L) PetBridge.nativeGetFoodListJson(enginePtr) else "[]"
        val itemsArray = try {
            JSONArray(rawJson)
        } catch (_: Exception) {
            JSONArray()
        }

        val rootCard = LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            setPadding(32, 28, 32, 28)
            background = GradientDrawable().apply {
                setColor(0xFFFFFFFF.toInt())
                cornerRadius = 32f
                setStroke(3, 0xFF0288D1.toInt())
            }
            elevation = 28f
        }

        val dm = resources.displayMetrics

        // Header
        val header = LinearLayout(this).apply {
            orientation = LinearLayout.HORIZONTAL
            gravity = Gravity.CENTER_VERTICAL
            setPadding(0, 0, 0, 16)
        }

        val stateJson = if (enginePtr != 0L) PetBridge.nativeGetStateJson(enginePtr) else "{}"
        val userMoney = try {
            JSONObject(stateJson).optDouble("money", 0.0)
        } catch (_: Exception) {
            0.0
        }

        val tvTitle = TextView(this).apply {
            text = "Food Store - Balance: $${String.format("%.1f", userMoney)}"
            textSize = 15f
            setTypeface(null, Typeface.BOLD)
            setTextColor(0xFF0F172A.toInt())
            layoutParams = LinearLayout.LayoutParams(0, LinearLayout.LayoutParams.WRAP_CONTENT, 1f)
        }
        header.addView(tvTitle)

        val btnClose = TextView(this).apply {
            text = "X"
            textSize = 18f
            setTypeface(null, Typeface.BOLD)
            setTextColor(0xFF64748B.toInt())
            setPadding(16, 0, 8, 0)
            setOnClickListener { dismissStore() }
        }
        header.addView(btnClose)
        rootCard.addView(header)

        // Search Filter Input
        val etSearch = EditText(this).apply {
            hint = "Search food name..."
            textSize = 13f
            setTextColor(0xFF0F172A.toInt())
            setHintTextColor(0xFF94A3B8.toInt())
            background = GradientDrawable().apply {
                setColor(0xFFF8FAFC.toInt())
                cornerRadius = 16f
                setStroke(2, 0xFFE2E8F0.toInt())
            }
            setPadding(20, 16, 20, 16)
        }
        rootCard.addView(etSearch)

        // Scrollable List Container
        val itemsLayout = LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            setPadding(0, 16, 0, 0)
        }

        fun populateList(query: String) {
            itemsLayout.removeAllViews()
            var displayed = 0

            for (i in 0 until itemsArray.length()) {
                val item = itemsArray.optJSONObject(i) ?: continue
                val name = item.optString("name", "")
                if (query.isNotEmpty() && !name.contains(query, ignoreCase = true)) {
                    continue
                }

                val category = item.optString("category", "Meal")
                val price = item.optDouble("price", 0.0)
                val foodGain = item.optDouble("food", 0.0).toInt()
                val drinkGain = item.optDouble("drink", 0.0).toInt()
                val feelGain = item.optDouble("feeling", 0.0).toInt()
                val healthGain = item.optDouble("health", 0.0).toInt()
                val desc = item.optString("desc", "")
                val imagePath = item.optString("image_path", "")

                val rowCard = LinearLayout(this).apply {
                    orientation = LinearLayout.HORIZONTAL
                    gravity = Gravity.CENTER_VERTICAL
                    setPadding(16, 14, 16, 14)
                    background = GradientDrawable().apply {
                        setColor(0xFFF8FAFC.toInt())
                        cornerRadius = 18f
                        setStroke(1, 0xFFE2E8F0.toInt())
                    }
                    layoutParams = LinearLayout.LayoutParams(
                        LinearLayout.LayoutParams.MATCH_PARENT,
                        LinearLayout.LayoutParams.WRAP_CONTENT
                    ).apply { setMargins(0, 0, 0, 10) }
                }

                val ivIcon = android.widget.ImageView(this).apply {
                    val iconSize = (42 * dm.density).toInt()
                    layoutParams = LinearLayout.LayoutParams(iconSize, iconSize).apply {
                        setMargins(0, 0, (12 * dm.density).toInt(), 0)
                    }
                    scaleType = android.widget.ImageView.ScaleType.FIT_CENTER
                    if (imagePath.isNotEmpty()) {
                        val imgFile = File(imagePath)
                        if (imgFile.exists()) {
                            val bm = android.graphics.BitmapFactory.decodeFile(imagePath)
                            if (bm != null) {
                                setImageBitmap(bm)
                            }
                        }
                    }
                }
                rowCard.addView(ivIcon)

                val colDetails = LinearLayout(this).apply {
                    orientation = LinearLayout.VERTICAL
                    layoutParams = LinearLayout.LayoutParams(0, LinearLayout.LayoutParams.WRAP_CONTENT, 1f)
                }

                val tvItemName = TextView(this).apply {
                    text = "$name ($category)"
                    textSize = 13f
                    setTypeface(null, Typeface.BOLD)
                    setTextColor(0xFF0F172A.toInt())
                }
                colDetails.addView(tvItemName)

                val tvStatBoost = TextView(this).apply {
                    val buffs = mutableListOf<String>()
                    if (foodGain != 0) buffs.add("Food +$foodGain")
                    if (drinkGain != 0) buffs.add("Drink +$drinkGain")
                    if (feelGain != 0) buffs.add("Feel +$feelGain")
                    if (healthGain != 0) buffs.add("Health +$healthGain")
                    text = buffs.joinToString(" • ")
                    textSize = 11f
                    setTextColor(0xFF0288D1.toInt())
                    setPadding(0, 2, 0, if (desc.isNotEmpty()) 2 else 0)
                }
                colDetails.addView(tvStatBoost)

                if (desc.isNotEmpty()) {
                    val tvDesc = TextView(this).apply {
                        text = desc
                        textSize = 10f
                        setTextColor(0xFF64748B.toInt())
                    }
                    colDetails.addView(tvDesc)
                }
                rowCard.addView(colDetails)

                val btnBuy = Button(this).apply {
                    text = "$${String.format("%.1f", price)}"
                    textSize = 11f
                    isAllCaps = false
                    setTextColor(0xFFFFFFFF.toInt())
                    background = GradientDrawable().apply {
                        setColor(0xFF0288D1.toInt())
                        cornerRadius = 16f
                    }
                    layoutParams = LinearLayout.LayoutParams(
                        LinearLayout.LayoutParams.WRAP_CONTENT,
                        LinearLayout.LayoutParams.WRAP_CONTENT
                    ).apply {
                        setMargins((8 * dm.density).toInt(), 0, 0, 0)
                    }
                    setOnClickListener {
                        if (enginePtr != 0L) {
                            val res = PetBridge.nativeFeed(enginePtr, name)
                            Toast.makeText(this@PetOverlayService, res, Toast.LENGTH_SHORT).show()
                            dismissStore()
                        }
                    }
                }
                rowCard.addView(btnBuy)

                itemsLayout.addView(rowCard)
                displayed++
                if (displayed >= 40 && query.isEmpty()) {
                    // Show first 40 when no search to keep scrolling fast
                    val tvMore = TextView(this).apply {
                        text = "Use search above to find remaining items..."
                        textSize = 12f
                        setTextColor(0xFF94A3B8.toInt())
                        gravity = Gravity.CENTER
                        setPadding(0, 12, 0, 12)
                    }
                    itemsLayout.addView(tvMore)
                    break
                }
            }
        }

        populateList("")

        etSearch.addTextChangedListener(object : android.text.TextWatcher {
            override fun afterTextChanged(s: android.text.Editable?) {
                populateList(s?.toString()?.trim() ?: "")
            }
            override fun beforeTextChanged(s: CharSequence?, start: Int, count: Int, after: Int) {}
            override fun onTextChanged(s: CharSequence?, start: Int, before: Int, count: Int) {}
        })

        val scroll = ScrollView(this).apply {
            layoutParams = LinearLayout.LayoutParams(
                LinearLayout.LayoutParams.MATCH_PARENT,
                (resources.displayMetrics.heightPixels * 0.55).toInt()
            )
        }
        scroll.addView(itemsLayout)
        rootCard.addView(scroll)

        val overlayType = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
            WindowManager.LayoutParams.TYPE_APPLICATION_OVERLAY
        } else {
            @Suppress("DEPRECATION")
            WindowManager.LayoutParams.TYPE_PHONE
        }

        val storeWidth = (dm.widthPixels * 0.90).toInt().coerceAtMost(1000)

        val storeParams = WindowManager.LayoutParams(
            storeWidth,
            WindowManager.LayoutParams.WRAP_CONTENT,
            overlayType,
            WindowManager.LayoutParams.FLAG_NOT_TOUCH_MODAL or
                WindowManager.LayoutParams.FLAG_WATCH_OUTSIDE_TOUCH,
            PixelFormat.TRANSLUCENT
        ).apply {
            gravity = Gravity.CENTER
        }

        rootCard.setOnTouchListener { _, event ->
            if (event.action == MotionEvent.ACTION_OUTSIDE) {
                dismissStore()
                true
            } else {
                false
            }
        }

        wm.addView(rootCard, storeParams)
        activeStoreView = rootCard
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

    override fun onConfigurationChanged(newConfig: android.content.res.Configuration) {
        super.onConfigurationChanged(newConfig)
        val dm = resources.displayMetrics
        if (enginePtr != 0L) {
            PetBridge.nativeSetScreenBounds(enginePtr, dm.widthPixels, dm.heightPixels)
            layoutParams?.let { params ->
                val offsetLeft = ((219f * (petView?.bitmapWidth ?: 280)) / 500f).toInt()
                val offsetRight = (((500f - 281f) * (petView?.bitmapWidth ?: 280)) / 500f).toInt()
                val minX = -offsetLeft
                val maxX = dm.widthPixels - (petView?.bitmapWidth ?: 280) + offsetRight
                val maxY = (dm.heightPixels - (petView?.bitmapHeight ?: 350)).coerceAtLeast(0)
                params.x = params.x.coerceIn(minX, maxX)
                params.y = params.y.coerceIn(0, maxY)
                PetBridge.nativeSetPos(enginePtr, params.x, params.y)
                windowManager?.updateViewLayout(petView, params)
            }
        }
    }

    override fun onDestroy() {
        isRunning = false
        handler.removeCallbacks(tickRunnable)

        dismissMenu()
        dismissStore()

        try {
            unregisterReceiver(screenReceiver)
        } catch (_: Exception) {}

        if (petView != null && windowManager != null) {
            windowManager?.removeView(petView)
            petView?.renderBitmap?.recycle()
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
