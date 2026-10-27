package org.vpet.mobile

import android.graphics.Bitmap

object PetBridge {
    init {
        System.loadLibrary("vpet_mobile")
    }

    external fun nativeInit(
        contentRoot: String,
        saveDir: String,
        targetPx: Int,
        aiBase: String,
        aiKey: String,
        aiModel: String,
        aiPrompt: String,
        aiTimeout: Int
    ): Long

    external fun nativeDestroy(enginePtr: Long)

    external fun nativeTick(enginePtr: Long, dtSeconds: Float): Boolean

    external fun nativeRender(enginePtr: Long, bitmap: Bitmap): Boolean

    external fun nativeTouch(
        enginePtr: Long,
        eventType: String,
        relX: Float,
        relY: Float
    ): String

    external fun nativeGetStateJson(enginePtr: Long): String

    external fun nativeGetFoodListJson(enginePtr: Long): String

    external fun nativeGetWorkListJson(enginePtr: Long): String

    external fun nativeFeed(enginePtr: Long, foodName: String): String

    external fun nativeToggleSleep(enginePtr: Long): Boolean

    external fun nativeStartWork(enginePtr: Long, workName: String): String

    external fun nativeCancelWork(enginePtr: Long): Boolean

    external fun nativeAskAi(enginePtr: Long, prompt: String): Boolean

    external fun nativeSay(enginePtr: Long, text: String)

    external fun nativeSave(enginePtr: Long)

    external fun nativeSetPos(enginePtr: Long, x: Int, y: Int)

    external fun nativeGetPosX(enginePtr: Long): Int

    external fun nativeGetPosY(enginePtr: Long): Int

    external fun nativeSetScreenBounds(enginePtr: Long, width: Int, height: Int)

    external fun nativeSetAutoWalk(enginePtr: Long, enabled: Boolean)

    external fun nativeSetTargetPx(enginePtr: Long, targetPx: Int)

    external fun nativeUpdateAiConfig(
        enginePtr: Long,
        aiBase: String,
        aiKey: String,
        aiModel: String,
        aiPrompt: String,
        aiTimeout: Int
    )
}
