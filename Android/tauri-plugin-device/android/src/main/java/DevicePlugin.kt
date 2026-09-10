package com.yakit.lightledger.device

import android.app.Activity
import android.net.Uri
import android.provider.OpenableColumns
import android.security.keystore.KeyGenParameterSpec
import android.security.keystore.KeyProperties
import android.util.Base64
import app.tauri.annotation.Command
import app.tauri.annotation.InvokeArg
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.Invoke
import app.tauri.plugin.JSObject
import app.tauri.plugin.Plugin
import java.io.File
import java.io.InputStream
import java.io.OutputStream
import java.nio.ByteBuffer
import java.nio.charset.CodingErrorAction
import java.security.KeyStore
import java.util.concurrent.Executors
import javax.crypto.Cipher
import javax.crypto.KeyGenerator
import javax.crypto.SecretKey
import javax.crypto.spec.GCMParameterSpec

@InvokeArg
class ProtectArgs { lateinit var data: String; var decrypt: Boolean = false }
@InvokeArg
class FileArgs { lateinit var path: String; var text: String = "" }
@InvokeArg
class CopyArgs { lateinit var source: String; lateinit var target: String }

@TauriPlugin
class DevicePlugin(private val activity: Activity) : Plugin(activity) {
    private val worker = Executors.newSingleThreadExecutor()
    private val alias = "lightledger.local-secrets.v1"

    // 密钥和文件操作放到工作线程，避免 Keystore 或文件提供方卡住界面。
    private fun execute(invoke: Invoke, action: () -> JSObject) {
        worker.execute {
            try { invoke.resolve(action()) }
            catch (error: Exception) { invoke.reject(error.message ?: "Android 设备操作失败") }
        }
    }

    private fun key(decrypt: Boolean): SecretKey {
        val store = KeyStore.getInstance("AndroidKeyStore").apply { load(null) }
        if (store.containsAlias(alias)) return store.getKey(alias, null) as SecretKey
        check(!decrypt) { "Android Keystore 密钥已丢失，无法解封本地数据" }
        return KeyGenerator.getInstance(KeyProperties.KEY_ALGORITHM_AES, "AndroidKeyStore").apply {
            init(KeyGenParameterSpec.Builder(alias, KeyProperties.PURPOSE_ENCRYPT or KeyProperties.PURPOSE_DECRYPT)
                .setBlockModes(KeyProperties.BLOCK_MODE_GCM)
                .setEncryptionPaddings(KeyProperties.ENCRYPTION_PADDING_NONE)
                .setKeySize(256).build())
        }.generateKey()
    }

    @Command
    fun protect(invoke: Invoke) = execute(invoke) {
        val args = invoke.parseArgs(ProtectArgs::class.java)
        val bytes = Base64.decode(args.data, Base64.NO_WRAP)
        val cipher = Cipher.getInstance("AES/GCM/NoPadding")
        val output = if (args.decrypt) {
            require(bytes.size >= 29 && bytes[0] == 1.toByte()) { "密钥保护文件无效" }
            cipher.init(Cipher.DECRYPT_MODE, key(true), GCMParameterSpec(128, bytes.copyOfRange(1, 13)))
            cipher.doFinal(bytes, 13, bytes.size - 13)
        } else {
            cipher.init(Cipher.ENCRYPT_MODE, key(false))
            byteArrayOf(1) + cipher.iv + cipher.doFinal(bytes)
        }
        JSObject().put("data", Base64.encodeToString(output, Base64.NO_WRAP))
    }

    private fun input(path: String): InputStream {
        val uri = Uri.parse(path)
        return when (uri.scheme) {
            "content" -> activity.contentResolver.openInputStream(uri) ?: error("无法读取所选文件")
            "file" -> File(requireNotNull(uri.path)).inputStream()
            null -> File(path).inputStream()
            else -> error("不支持的文件地址")
        }
    }

    private fun output(path: String): OutputStream {
        val uri = Uri.parse(path)
        return when (uri.scheme) {
            "content" -> activity.contentResolver.openOutputStream(uri, "wt") ?: error("无法写入所选文件")
            "file" -> File(requireNotNull(uri.path)).outputStream()
            null -> File(path).outputStream()
            else -> error("不支持的文件地址")
        }
    }

    @Command
    fun readText(invoke: Invoke) = execute(invoke) {
        val args = invoke.parseArgs(FileArgs::class.java)
        val bytes = input(args.path).use { stream ->
            val output = java.io.ByteArrayOutputStream()
            val buffer = ByteArray(8192)
            while (true) {
                val count = stream.read(buffer)
                if (count < 0) break
                require(output.size() + count <= 20 * 1024 * 1024) { "导入文件不能超过20MB" }
                output.write(buffer, 0, count)
            }
            output.toByteArray()
        }
        val text = Charsets.UTF_8.newDecoder().onMalformedInput(CodingErrorAction.REPORT)
            .onUnmappableCharacter(CodingErrorAction.REPORT).decode(ByteBuffer.wrap(bytes)).toString()
        val uri = Uri.parse(args.path)
        val name = if (uri.scheme == "content") {
            activity.contentResolver.query(uri, arrayOf(OpenableColumns.DISPLAY_NAME), null, null, null)?.use {
                if (it.moveToFirst()) it.getString(0) else null
            }
        } else File(uri.path ?: args.path).name
        JSObject().put("text", text).put("name", name ?: "")
    }

    @Command
    fun writeText(invoke: Invoke) = execute(invoke) {
        val args = invoke.parseArgs(FileArgs::class.java)
        output(args.path).use { it.write(args.text.toByteArray(Charsets.UTF_8)) }
        JSObject().put("saved", true)
    }

    @Command
    fun copyFile(invoke: Invoke) = execute(invoke) {
        val args = invoke.parseArgs(CopyArgs::class.java)
        input(args.source).use { source -> output(args.target).use { target -> source.copyTo(target) } }
        JSObject().put("saved", true)
    }
}
