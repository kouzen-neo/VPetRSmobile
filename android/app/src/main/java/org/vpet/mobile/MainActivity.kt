package org.vpet.mobile

import android.content.Context
import android.content.Intent
import android.content.SharedPreferences
import android.net.Uri
import android.os.Build
import android.os.Bundle
import android.os.Handler
import android.os.Looper
import android.provider.Settings
import android.view.View
import android.widget.Button
import android.widget.EditText
import android.widget.ProgressBar
import android.widget.SeekBar
import android.widget.TextView
import android.widget.Toast
import androidx.appcompat.app.AppCompatActivity
import java.io.File
import java.io.FileOutputStream
import java.util.zip.ZipInputStream
import kotlin.concurrent.thread

class MainActivity : AppCompatActivity() {

    private lateinit var prefs: SharedPreferences

    private lateinit var tvStatus: TextView
    private lateinit var tvPermissionNote: TextView
    private lateinit var btnPermission: Button
    private lateinit var btnToggleService: Button
    private lateinit var progressExtract: ProgressBar
    private lateinit var sbScale: SeekBar
    private lateinit var tvScaleVal: TextView
    private lateinit var etAiBase: EditText
    private lateinit var etAiModel: EditText
    private lateinit var btnSaveAi: Button

    private val mainHandler = Handler(Looper.getMainLooper())

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        prefs = getSharedPreferences("vpet_prefs", Context.MODE_PRIVATE)

        setContentView(createProgrammaticLayout())
        checkAndExtractAssets()
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

    private fun checkAndExtractAssets() {
        val coreDir = File(filesDir, "content/0000_core")
        val infoFile = File(coreDir, "info.lps")

        if (infoFile.exists()) {
            tvStatus.text = "Assets ready. Start floating pet below!"
            progressExtract.visibility = View.GONE
            return
        }

        tvStatus.text = "Extracting starter assets (first run)..."
        progressExtract.visibility = View.VISIBLE
        btnToggleService.isEnabled = false

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
                mainHandler.post {
                    tvStatus.text = "Assets extracted successfully!"
                    progressExtract.visibility = View.GONE
                    updatePermissionUi()
                }
            } catch (e: Exception) {
                mainHandler.post {
                    tvStatus.text = "Failed to extract assets: ${e.message}"
                    progressExtract.visibility = View.GONE
                }
            }
        }
    }

    private fun togglePetService() {
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

    private fun createProgrammaticLayout(): View {
        val root = android.widget.LinearLayout(this).apply {
            orientation = android.widget.LinearLayout.VERTICAL
            setPadding(40, 50, 40, 40)
            setBackgroundColor(0xFFF7F9FC.toInt())
        }

        val tvTitle = TextView(this).apply {
            text = "VPet Mobile"
            textSize = 24f
            setTypeface(null, android.graphics.Typeface.BOLD)
            setTextColor(0xFF0288D1.toInt())
            setPadding(0, 0, 0, 16)
        }
        root.addView(tvTitle)

        tvStatus = TextView(this).apply {
            text = "Checking system..."
            textSize = 14f
            setTextColor(0xFF555555.toInt())
            setPadding(0, 0, 0, 20)
        }
        root.addView(tvStatus)

        progressExtract = ProgressBar(this).apply {
            visibility = View.GONE
        }
        root.addView(progressExtract)

        tvPermissionNote = TextView(this).apply {
            text = getString(R.string.permission_required_desc)
            textSize = 13f
            setTextColor(0xFFD32F2F.toInt())
            setPadding(0, 10, 0, 10)
        }
        root.addView(tvPermissionNote)

        btnPermission = Button(this).apply {
            text = getString(R.string.grant_permission)
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

        btnToggleService = Button(this).apply {
            text = getString(R.string.start_pet)
            setBackgroundColor(0xFF0288D1.toInt())
            setTextColor(0xFFFFFFFF.toInt())
            setOnClickListener {
                togglePetService()
            }
        }
        root.addView(btnToggleService)

        val btnStop = Button(this).apply {
            text = getString(R.string.stop_pet)
            setOnClickListener {
                stopPetService()
            }
        }
        root.addView(btnStop)

        // Scale controls
        val tvScaleHeader = TextView(this).apply {
            text = "Pet Sprite Scale (Size)"
            textSize = 15f
            setTypeface(null, android.graphics.Typeface.BOLD)
            setTextColor(0xFF333333.toInt())
            setPadding(0, 30, 0, 8)
        }
        root.addView(tvScaleHeader)

        val currentScale = prefs.getInt("pet_scale_px", 280)
        tvScaleVal = TextView(this).apply {
            text = "$currentScale px"
            textSize = 13f
            setTextColor(0xFF666666.toInt())
        }
        root.addView(tvScaleVal)

        sbScale = SeekBar(this).apply {
            max = 240 // 160 + 240 = 400
            progress = currentScale - 160
            setOnSeekBarChangeListener(object : SeekBar.OnSeekBarChangeListener {
                override fun onProgressChanged(seekBar: SeekBar?, progress: Int, fromUser: Boolean) {
                    val scalePx = 160 + progress
                    tvScaleVal.text = "$scalePx px"
                    prefs.edit().putInt("pet_scale_px", scalePx).apply()
                }
                override fun onStartTrackingTouch(seekBar: SeekBar?) {}
                override fun onStopTrackingTouch(seekBar: SeekBar?) {}
            })
        }
        root.addView(sbScale)

        // AI Settings
        val tvAiHeader = TextView(this).apply {
            text = "AI Chat Settings (LLM)"
            textSize = 15f
            setTypeface(null, android.graphics.Typeface.BOLD)
            setTextColor(0xFF333333.toInt())
            setPadding(0, 30, 0, 8)
        }
        root.addView(tvAiHeader)

        etAiBase = EditText(this).apply {
            hint = "http://192.168.18.x:20128/v1"
            setText(prefs.getString("ai_base", "http://10.0.2.2:20128/v1"))
            textSize = 13f
        }
        root.addView(etAiBase)

        etAiModel = EditText(this).apply {
            hint = "Model (e.g. 9routerall)"
            setText(prefs.getString("ai_model", "9routerall"))
            textSize = 13f
        }
        root.addView(etAiModel)

        btnSaveAi = Button(this).apply {
            text = "Save AI Config"
            setOnClickListener {
                prefs.edit()
                    .putString("ai_base", etAiBase.text.toString().trim())
                    .putString("ai_model", etAiModel.text.toString().trim())
                    .apply()
                Toast.makeText(this@MainActivity, "AI Config Saved", Toast.LENGTH_SHORT).show()
            }
        }
        root.addView(btnSaveAi)

        val scroll = android.widget.ScrollView(this)
        scroll.addView(root)
        return scroll
    }
}
