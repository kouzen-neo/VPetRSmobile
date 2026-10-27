package org.vpet.mobile

import android.content.Context
import android.graphics.Bitmap
import android.graphics.Canvas
import android.graphics.Paint
import android.view.GestureDetector
import android.view.MotionEvent
import android.view.View
import kotlin.math.abs

class PetView(
    context: Context,
    var targetPx: Int,
    var bubblePadPx: Int,
    private val onMoveDelta: (dx: Int, dy: Int) -> Unit,
    private val onDragStart: () -> Unit,
    private val onDragEnd: () -> Unit,
    private val onActionTouch: (type: String, x: Float, y: Float) -> Unit,
    private val onLongPressAction: () -> Unit
) : View(context) {

    var bitmapWidth = targetPx
        private set
    var bitmapHeight = targetPx + bubblePadPx
        private set

    var renderBitmap: Bitmap = Bitmap.createBitmap(
        bitmapWidth,
        bitmapHeight,
        Bitmap.Config.ARGB_8888
    )
        private set

    fun updateTargetSize(newTargetPx: Int) {
        if (newTargetPx == targetPx) return
        targetPx = newTargetPx
        bubblePadPx = newTargetPx / 3
        bitmapWidth = targetPx
        bitmapHeight = targetPx + bubblePadPx
        try {
            renderBitmap.recycle()
        } catch (_: Exception) {}
        renderBitmap = Bitmap.createBitmap(bitmapWidth, bitmapHeight, Bitmap.Config.ARGB_8888)
        requestLayout()
        invalidate()
    }

    private val paint = Paint(Paint.ANTI_ALIAS_FLAG or Paint.FILTER_BITMAP_FLAG)

    private var initialTouchX = 0f
    private var initialTouchY = 0f
    private var isDragging = false
    private val touchSlop = 10f * resources.displayMetrics.density

    private val gestureDetector = GestureDetector(context, object : GestureDetector.SimpleOnGestureListener() {
        override fun onSingleTapUp(e: MotionEvent): Boolean {
            if (!isDragging) {
                onActionTouch("tap", e.x, e.y)
            }
            return true
        }

        override fun onLongPress(e: MotionEvent) {
            if (!isDragging) {
                onLongPressAction()
            }
        }
    })

    override fun onMeasure(widthMeasureSpec: Int, heightMeasureSpec: Int) {
        setMeasuredDimension(bitmapWidth, bitmapHeight)
    }

    override fun onDraw(canvas: Canvas) {
        super.onDraw(canvas)
        canvas.drawBitmap(renderBitmap, 0f, 0f, paint)
    }

    override fun onTouchEvent(event: MotionEvent): Boolean {
        gestureDetector.onTouchEvent(event)

        when (event.actionMasked) {
            MotionEvent.ACTION_DOWN -> {
                initialTouchX = event.rawX
                initialTouchY = event.rawY
                isDragging = false
                onActionTouch("down", event.x, event.y)
                return true
            }

            MotionEvent.ACTION_MOVE -> {
                val dx = (event.rawX - initialTouchX).toInt()
                val dy = (event.rawY - initialTouchY).toInt()

                if (!isDragging && (abs(dx) > touchSlop || abs(dy) > touchSlop)) {
                    isDragging = true
                    onDragStart()
                    onActionTouch("drag", event.x, event.y)
                }

                if (isDragging) {
                    onMoveDelta(dx, dy)
                    initialTouchX = event.rawX
                    initialTouchY = event.rawY
                }
                return true
            }

            MotionEvent.ACTION_UP, MotionEvent.ACTION_CANCEL -> {
                if (isDragging) {
                    isDragging = false
                    onDragEnd()
                    onActionTouch("drop", event.x, event.y)
                }
                return true
            }
        }

        return super.onTouchEvent(event)
    }
}
