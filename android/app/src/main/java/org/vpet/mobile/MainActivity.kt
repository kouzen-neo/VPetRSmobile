package org.vpet.mobile

import android.content.Context
import android.content.Intent
import android.content.SharedPreferences
import android.graphics.Typeface
import android.graphics.drawable.GradientDrawable
import android.net.Uri
import android.os.Build
import android.os.Bundle
import android.os.Handler
import android.os.Looper
import android.provider.Settings
import android.view.Gravity
import android.view.View
import android.widget.Button
import android.widget.CheckBox
import android.widget.EditText
import android.widget.LinearLayout
import android.widget.ProgressBar
import android.widget.ScrollView
import android.widget.SeekBar
import android.widget.TextView
import android.widget.Toast
import androidx.appcompat.app.AppCompatActivity
import java.io.File
import java.io.FileOutputStream
import java.util.zip.ZipInputStream
import kotlin.concurrent.thread

class MainActivity : AppCompatActivity() {

    companion object {
        const val CURRENT_ASSETS_VERSION = 10
    }

    private lateinit var prefs: SharedPreferences

    private lateinit var tvStatus: TextView
    private lateinit var tvPermissionNote: TextView
    private lateinit var btnPermission: Button
    private lateinit var btnToggleService: Button
    private lateinit var btnStopService: Button
    private lateinit var btnReextract: Button
    private lateinit var progressExtract: ProgressBar
    private lateinit var sbScale: SeekBar
    private lateinit var tvScaleVal: TextView
    private lateinit var cbAutoWalk: CheckBox
    private lateinit var etAiBase: EditText
    private lateinit var etAiKey: EditText
    private lateinit var etAiModel: EditText
    private lateinit var etAiPrompt: EditText
    private lateinit var etAiTimeout: EditText
    private lateinit var btnSaveAi: Button

    private val mainHandler = Handler(Looper.getMainLooper())

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        prefs = getSharedPreferences("vpet_prefs", Context.MODE_PRIVATE)

        setContentView(createUi())
        checkAndExtractAssets(force = false)
    }

    override fun onResume() {
        super.onResume()
        updatePermissionUi()
    }

    private fun updatePermissionUi() {
        val hasOverlay = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.M) {
            Settings.canDrawOverlays(this)
        } else {
            true
        }

        if (hasOverlay) {
            tvPermissionNote.visibility = View.GONE
            btnPermission.visibility = View.GONE
            btnToggleService.isEnabled = true
        } else {
            tvPermissionNote.visibility = View.VISIBLE
            btnPermission.visibility = View.VISIBLE
            btnToggleService.isEnabled = false
        }
    }

    private fun checkAndExtractAssets(force: Boolean) {
        val coreDir = File(filesDir, "content/0000_core")
        val sideHideDir = File(coreDir, "pet/vup/SideHide_Left_Main")
        val foodImgDir = File(coreDir, "image/food")
        val sampleFood = File(foodImgDir, "三明治.png")
        val savedVer = prefs.getInt("assets_version", 0)

        if (!force && savedVer >= CURRENT_ASSETS_VERSION && sideHideDir.exists() && foodImgDir.exists() && sampleFood.exists()) {
            tvStatus.text = "All pet assets ready! Tap Start below."
            tvStatus.setTextColor(0xFF16A34A.toInt())
            progressExtract.visibility = View.GONE
            btnToggleService.isEnabled = true
            return
        }

        tvStatus.text = "Extracting complete animations & sounds (one-time setup)..."
        tvStatus.setTextColor(0xFF0288D1.toInt())
        progressExtract.visibility = View.VISIBLE
        btnToggleService.isEnabled = false
        btnReextract.isEnabled = false

        thread {
            try {
                assets.open("0000_core.zip").use { input ->
                    ZipInputStream(input).use { zip ->
                        coreDir.mkdirs()
                        var entry = zip.nextEntry
                        while (entry != null) {
                            val outFile = File(coreDir, entry.name)
                            if (entry.isDirectory) {
                                outFile.mkdirs()
                            } else {
                                outFile.parentFile?.mkdirs()
                                FileOutputStream(outFile).use { fos ->
                                    zip.copyTo(fos)
                                }
                            }
                            zip.closeEntry()
                            entry = zip.nextEntry
                        }
                    }
                }
                prefs.edit().putInt("assets_version", CURRENT_ASSETS_VERSION).apply()
                mainHandler.post {
                    tvStatus.text = "Extracted all animations successfully! Ready."
                    tvStatus.setTextColor(0xFF16A34A.toInt())
                    progressExtract.visibility = View.GONE
                    btnReextract.isEnabled = true
                    updatePermissionUi()
                    Toast.makeText(this@MainActivity, "Assets ready!", Toast.LENGTH_SHORT).show()
                }
            } catch (e: Exception) {
                mainHandler.post {
                    tvStatus.text = "Extraction failed: ${e.message}"
                    tvStatus.setTextColor(0xFFDC2626.toInt())
                    progressExtract.visibility = View.GONE
                    btnReextract.isEnabled = true
                }
            }
        }
    }

    private fun startPetService() {
        val intent = Intent(this, PetOverlayService::class.java)
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
            startForegroundService(intent)
        } else {
            startService(intent)
        }
        Toast.makeText(this, "VPet Floating Overlay started!", Toast.LENGTH_SHORT).show()
    }

    private fun stopPetService() {
        val intent = Intent(this, PetOverlayService::class.java)
        stopService(intent)
        Toast.makeText(this, "VPet Floating Overlay stopped", Toast.LENGTH_SHORT).show()
    }

    private fun createUi(): View {
        val root = LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            setPadding(40, 50, 40, 60)
            setBackgroundColor(0xFFF8FAFC.toInt())
        }

        // Header
        val tvTitle = TextView(this).apply {
            text = "VPet Mobile"
            textSize = 26f
            setTypeface(null, Typeface.BOLD)
            setTextColor(0xFF0288D1.toInt())
            setPadding(0, 0, 0, 8)
        }
        root.addView(tvTitle)

        val tvSub = TextView(this).apply {
            text = "Pure Rust Desktop Pet for Android"
            textSize = 13f
            setTextColor(0xFF64748B.toInt())
            setPadding(0, 0, 0, 24)
        }
        root.addView(tvSub)

        // Status Card
        val statusCard = LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            setPadding(32, 28, 32, 28)
            background = GradientDrawable().apply {
                setColor(0xFFFFFFFF.toInt())
                cornerRadius = 24f
                setStroke(2, 0xFFE2E8F0.toInt())
            }
            layoutParams = LinearLayout.LayoutParams(
                LinearLayout.LayoutParams.MATCH_PARENT,
                LinearLayout.LayoutParams.WRAP_CONTENT
            ).apply { setMargins(0, 0, 0, 24) }
        }

        tvStatus = TextView(this).apply {
            text = "Checking assets..."
            textSize = 14f
            setTypeface(null, Typeface.BOLD)
            setTextColor(0xFF0F172A.toInt())
            setPadding(0, 0, 0, 12)
        }
        statusCard.addView(tvStatus)

        progressExtract = ProgressBar(this).apply {
            visibility = View.GONE
        }
        statusCard.addView(progressExtract)

        btnReextract = Button(this).apply {
            text = "Force Re-Extract Complete Assets"
            textSize = 12f
            isAllCaps = false
            setTextColor(0xFF0288D1.toInt())
            background = GradientDrawable().apply {
                setColor(0xFFF0F9FF.toInt())
                cornerRadius = 16f
                setStroke(1, 0xFFBAE6FD.toInt())
            }
            setOnClickListener {
                checkAndExtractAssets(force = true)
            }
        }
        statusCard.addView(btnReextract)

        root.addView(statusCard)

        // Permission Notice
        tvPermissionNote = TextView(this).apply {
            text = getString(R.string.permission_required_desc)
            textSize = 13f
            setTextColor(0xFFDC2626.toInt())
            setPadding(0, 0, 0, 12)
        }
        root.addView(tvPermissionNote)

        btnPermission = Button(this).apply {
            text = getString(R.string.grant_permission)
            textSize = 14f
            setTypeface(null, Typeface.BOLD)
            setTextColor(0xFFFFFFFF.toInt())
            background = GradientDrawable().apply {
                setColor(0xFFDC2626.toInt())
                cornerRadius = 20f
            }
            layoutParams = LinearLayout.LayoutParams(
                LinearLayout.LayoutParams.MATCH_PARENT,
                LinearLayout.LayoutParams.WRAP_CONTENT
            ).apply { setMargins(0, 0, 0, 16) }
            setOnClickListener {
                if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.M) {
                    val intent = Intent(
                        Settings.ACTION_MANAGE_OVERLAY_PERMISSION,
                        Uri.parse("package:$packageName")
                    )
                    startActivity(intent)
                }
            }
        }
        root.addView(btnPermission)

        // Main Controls (Start / Stop)
        btnToggleService = Button(this).apply {
            text = "Start Floating Pet"
            textSize = 15f
            setTypeface(null, Typeface.BOLD)
            setTextColor(0xFFFFFFFF.toInt())
            background = GradientDrawable().apply {
                setColor(0xFF0288D1.toInt())
                cornerRadius = 24f
            }
            layoutParams = LinearLayout.LayoutParams(
                LinearLayout.LayoutParams.MATCH_PARENT,
                LinearLayout.LayoutParams.WRAP_CONTENT
            ).apply { setMargins(0, 0, 0, 12) }
            setOnClickListener {
                startPetService()
            }
        }
        root.addView(btnToggleService)

        btnStopService = Button(this).apply {
            text = "Stop Pet Service"
            textSize = 14f
            setTextColor(0xFF475569.toInt())
            background = GradientDrawable().apply {
                setColor(0xFFF1F5F9.toInt())
                cornerRadius = 20f
                setStroke(1, 0xFFCBD5E1.toInt())
            }
            layoutParams = LinearLayout.LayoutParams(
                LinearLayout.LayoutParams.MATCH_PARENT,
                LinearLayout.LayoutParams.WRAP_CONTENT
            ).apply { setMargins(0, 0, 0, 32) }
            setOnClickListener {
                stopPetService()
            }
        }
        root.addView(btnStopService)

        // Scale Section
        val tvScaleHeader = TextView(this).apply {
            text = "Pet Display Size"
            textSize = 15f
            setTypeface(null, Typeface.BOLD)
            setTextColor(0xFF0F172A.toInt())
            setPadding(0, 0, 0, 4)
        }
        root.addView(tvScaleHeader)

        val currentScale = prefs.getInt("pet_scale_px", 280)
        tvScaleVal = TextView(this).apply {
            text = "$currentScale px (Slide to resize live on screen)"
            textSize = 13f
            setTextColor(0xFF64748B.toInt())
            setPadding(0, 0, 0, 8)
        }
        root.addView(tvScaleVal)

        sbScale = SeekBar(this).apply {
            max = 240
            progress = currentScale - 160
            setOnSeekBarChangeListener(object : SeekBar.OnSeekBarChangeListener {
                override fun onProgressChanged(seekBar: SeekBar?, progress: Int, fromUser: Boolean) {
                    val scalePx = 160 + progress
                    tvScaleVal.text = "$scalePx px (Slide to resize live on screen)"
                    prefs.edit().putInt("pet_scale_px", scalePx).apply()
                    if (fromUser) {
                        val intent = Intent(this@MainActivity, PetOverlayService::class.java).apply {
                            action = PetOverlayService.ACTION_SET_SCALE
                            putExtra("scale_px", scalePx)
                        }
                        startService(intent)
                    }
                }
                override fun onStartTrackingTouch(seekBar: SeekBar?) {}
                override fun onStopTrackingTouch(seekBar: SeekBar?) {}
            })
            layoutParams = LinearLayout.LayoutParams(
                LinearLayout.LayoutParams.MATCH_PARENT,
                LinearLayout.LayoutParams.WRAP_CONTENT
            ).apply { setMargins(0, 0, 0, 16) }
        }
        root.addView(sbScale)

        cbAutoWalk = CheckBox(this).apply {
            text = "Enable Auto-Walk & Idle Animations (Yawning, bubbles, cat meow, walking)"
            textSize = 13f
            setTextColor(0xFF0F172A.toInt())
            isChecked = prefs.getBoolean("pet_auto_walk", true)
            setOnCheckedChangeListener { _, isChecked ->
                prefs.edit().putBoolean("pet_auto_walk", isChecked).apply()
                val intent = Intent(this@MainActivity, PetOverlayService::class.java).apply {
                    action = PetOverlayService.ACTION_SET_SCALE
                    putExtra("scale_px", prefs.getInt("pet_scale_px", 280))
                }
                startService(intent)
            }
            layoutParams = LinearLayout.LayoutParams(
                LinearLayout.LayoutParams.MATCH_PARENT,
                LinearLayout.LayoutParams.WRAP_CONTENT
            ).apply { setMargins(0, 0, 0, 28) }
        }
        root.addView(cbAutoWalk)

        // AI Chat Settings Card
        val aiCard = LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            setPadding(32, 28, 32, 28)
            background = GradientDrawable().apply {
                setColor(0xFFFFFFFF.toInt())
                cornerRadius = 24f
                setStroke(2, 0xFFE2E8F0.toInt())
            }
        }

        val tvAiHeader = TextView(this).apply {
            text = "AI Chat Settings (OpenAI / DeepSeek / 9router / Ollama)"
            textSize = 15f
            setTypeface(null, Typeface.BOLD)
            setTextColor(0xFF0F172A.toInt())
            setPadding(0, 0, 0, 8)
        }
        aiCard.addView(tvAiHeader)

        // Provider Preset Chips Row
        val tvPresetsLbl = TextView(this).apply {
            text = "Quick Presets:"
            textSize = 12f
            setTextColor(0xFF64748B.toInt())
            setPadding(0, 0, 0, 6)
        }
        aiCard.addView(tvPresetsLbl)

        val presetsRow = LinearLayout(this).apply {
            orientation = LinearLayout.HORIZONTAL
            setPadding(0, 0, 0, 16)
        }

        fun createPresetBtn(label: String, base: String, model: String) {
            val btn = TextView(this).apply {
                text = label
                textSize = 11f
                setTextColor(0xFF0288D1.toInt())
                background = GradientDrawable().apply {
                    setColor(0xFFF0F9FF.toInt())
                    cornerRadius = 14f
                    setStroke(1, 0xFFBAE6FD.toInt())
                }
                setPadding(16, 8, 16, 8)
                layoutParams = LinearLayout.LayoutParams(
                    LinearLayout.LayoutParams.WRAP_CONTENT,
                    LinearLayout.LayoutParams.WRAP_CONTENT
                ).apply { setMargins(0, 0, 8, 0) }
                setOnClickListener {
                    etAiBase.setText(base)
                    etAiModel.setText(model)
                }
            }
            presetsRow.addView(btn)
        }

        createPresetBtn("9router Local", "http://192.168.18.7:20128/v1", "9routerall")
        createPresetBtn("OpenAI", "https://api.openai.com/v1", "gpt-4o-mini")
        createPresetBtn("DeepSeek", "https://api.deepseek.com/v1", "deepseek-chat")
        createPresetBtn("Groq", "https://api.groq.com/openai/v1", "llama-3.3-70b-versatile")

        val presetScroll = android.widget.HorizontalScrollView(this).apply {
            addView(presetsRow)
            setPadding(0, 0, 0, 8)
        }
        aiCard.addView(presetScroll)

        // API Base URL
        val tvAiBaseLbl = TextView(this).apply {
            text = "API Base URL (HTTP / HTTPS):"
            textSize = 12f
            setTextColor(0xFF64748B.toInt())
            setPadding(0, 0, 0, 4)
        }
        aiCard.addView(tvAiBaseLbl)

        etAiBase = EditText(this).apply {
            setText(prefs.getString("ai_base", "http://192.168.18.7:20128/v1"))
            textSize = 13f
            setTextColor(0xFF0F172A.toInt())
            setHintTextColor(0xFF94A3B8.toInt())
            background = GradientDrawable().apply {
                setColor(0xFFF8FAFC.toInt())
                cornerRadius = 16f
                setStroke(2, 0xFFE2E8F0.toInt())
            }
            setPadding(24, 20, 24, 20)
            layoutParams = LinearLayout.LayoutParams(
                LinearLayout.LayoutParams.MATCH_PARENT,
                LinearLayout.LayoutParams.WRAP_CONTENT
            ).apply { setMargins(0, 0, 0, 14) }
        }
        aiCard.addView(etAiBase)

        // API Key
        val tvAiKeyLbl = TextView(this).apply {
            text = "API Key (Bearer Token, leave blank for local without key):"
            textSize = 12f
            setTextColor(0xFF64748B.toInt())
            setPadding(0, 0, 0, 4)
        }
        aiCard.addView(tvAiKeyLbl)

        etAiKey = EditText(this).apply {
            setText(prefs.getString("ai_key", ""))
            hint = "sk-... (Required for OpenAI / DeepSeek / Groq)"
            textSize = 13f
            setTextColor(0xFF0F172A.toInt())
            setHintTextColor(0xFF94A3B8.toInt())
            background = GradientDrawable().apply {
                setColor(0xFFF8FAFC.toInt())
                cornerRadius = 16f
                setStroke(2, 0xFFE2E8F0.toInt())
            }
            setPadding(24, 20, 24, 20)
            layoutParams = LinearLayout.LayoutParams(
                LinearLayout.LayoutParams.MATCH_PARENT,
                LinearLayout.LayoutParams.WRAP_CONTENT
            ).apply { setMargins(0, 0, 0, 14) }
        }
        aiCard.addView(etAiKey)

        // Model Name
        val tvAiModelLbl = TextView(this).apply {
            text = "Model Name:"
            textSize = 12f
            setTextColor(0xFF64748B.toInt())
            setPadding(0, 0, 0, 4)
        }
        aiCard.addView(tvAiModelLbl)

        etAiModel = EditText(this).apply {
            setText(prefs.getString("ai_model", "9routerall"))
            textSize = 13f
            setTextColor(0xFF0F172A.toInt())
            setHintTextColor(0xFF94A3B8.toInt())
            background = GradientDrawable().apply {
                setColor(0xFFF8FAFC.toInt())
                cornerRadius = 16f
                setStroke(2, 0xFFE2E8F0.toInt())
            }
            setPadding(24, 20, 24, 20)
            layoutParams = LinearLayout.LayoutParams(
                LinearLayout.LayoutParams.MATCH_PARENT,
                LinearLayout.LayoutParams.WRAP_CONTENT
            ).apply { setMargins(0, 0, 0, 14) }
        }
        aiCard.addView(etAiModel)

        // System Prompt
        val tvAiPromptLbl = TextView(this).apply {
            text = "System Prompt (Pet personality instructions):"
            textSize = 12f
            setTextColor(0xFF64748B.toInt())
            setPadding(0, 0, 0, 4)
        }
        aiCard.addView(tvAiPromptLbl)

        etAiPrompt = EditText(this).apply {
            setText(
                prefs.getString(
                    "ai_prompt",
                    "You are Loris, a lively and affectionate desktop pet. Keep replies concise (1-2 short sentences)."
                )
            )
            textSize = 13f
            setTextColor(0xFF0F172A.toInt())
            setHintTextColor(0xFF94A3B8.toInt())
            background = GradientDrawable().apply {
                setColor(0xFFF8FAFC.toInt())
                cornerRadius = 16f
                setStroke(2, 0xFFE2E8F0.toInt())
            }
            setPadding(24, 20, 24, 20)
            layoutParams = LinearLayout.LayoutParams(
                LinearLayout.LayoutParams.MATCH_PARENT,
                LinearLayout.LayoutParams.WRAP_CONTENT
            ).apply { setMargins(0, 0, 0, 14) }
        }
        aiCard.addView(etAiPrompt)

        // Timeout
        val tvAiTimeoutLbl = TextView(this).apply {
            text = "Timeout (seconds):"
            textSize = 12f
            setTextColor(0xFF64748B.toInt())
            setPadding(0, 0, 0, 4)
        }
        aiCard.addView(tvAiTimeoutLbl)

        etAiTimeout = EditText(this).apply {
            setText(prefs.getInt("ai_timeout", 25).toString())
            inputType = android.text.InputType.TYPE_CLASS_NUMBER
            textSize = 13f
            setTextColor(0xFF0F172A.toInt())
            setHintTextColor(0xFF94A3B8.toInt())
            background = GradientDrawable().apply {
                setColor(0xFFF8FAFC.toInt())
                cornerRadius = 16f
                setStroke(2, 0xFFE2E8F0.toInt())
            }
            setPadding(24, 20, 24, 20)
            layoutParams = LinearLayout.LayoutParams(
                LinearLayout.LayoutParams.MATCH_PARENT,
                LinearLayout.LayoutParams.WRAP_CONTENT
            ).apply { setMargins(0, 0, 0, 20) }
        }
        aiCard.addView(etAiTimeout)

        btnSaveAi = Button(this).apply {
            text = "Save and Apply AI Configuration"
            textSize = 13f
            setTypeface(null, Typeface.BOLD)
            setTextColor(0xFFFFFFFF.toInt())
            background = GradientDrawable().apply {
                setColor(0xFF0288D1.toInt())
                cornerRadius = 20f
            }
            setOnClickListener {
                val timeoutVal = etAiTimeout.text.toString().trim().toIntOrNull() ?: 25
                prefs.edit()
                    .putString("ai_base", etAiBase.text.toString().trim())
                    .putString("ai_key", etAiKey.text.toString().trim())
                    .putString("ai_model", etAiModel.text.toString().trim())
                    .putString("ai_prompt", etAiPrompt.text.toString().trim())
                    .putInt("ai_timeout", timeoutVal)
                    .apply()

                val intent = Intent(this@MainActivity, PetOverlayService::class.java).apply {
                    action = PetOverlayService.ACTION_RELOAD_AI
                }
                startService(intent)
                Toast.makeText(this@MainActivity, "AI Config Saved and Applied Live!", Toast.LENGTH_SHORT).show()
            }
        }
        aiCard.addView(btnSaveAi)

        root.addView(aiCard)

        val scroll = ScrollView(this).apply {
            setBackgroundColor(0xFFF8FAFC.toInt())
        }
        scroll.addView(root)
        return scroll
    }
}
