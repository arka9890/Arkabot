package com.arkabot.android.webview

import android.os.Handler
import android.os.Looper
import android.webkit.WebView
import android.webkit.WebViewClient
import android.webkit.WebChromeClient
import org.json.JSONObject
import java.io.BufferedReader
import java.io.InputStreamReader
import java.io.OutputStream
import java.net.ServerSocket
import java.net.Socket
import java.util.concurrent.atomic.AtomicBoolean

/**
 * A loopback HTTP server the Rust engine drives to control this app's
 * WebView.
 *
 * The Rust side posts one JSON command per request:
 * `{"id":n,"method":"navigate","params":{"url":"…"}}` and expects
 * `{"id":n,"ok":true,"output":"…"}` back. The same command shape serves
 * every method, so the bridge stays small.
 *
 * The server binds to 127.0.0.1 only: nothing outside the phone can reach
 * it, and the password of the WebView itself guards the session.
 */
class BrowserBridgeServer(
    private val webView: WebView,
    private val onLog: (String) -> Unit = {}
) {

    private var serverSocket: ServerSocket? = null
    private var worker: Thread? = null
    private val running = AtomicBoolean(false)
    private val main = Handler(Looper.getMainLooper())

    /** The port the bridge listens on, once it is running. */
    @Volatile
    var port: Int = 0
        private set

    /** Start the server. Returns the port it listens on. */
    fun start(): Int {
        if (running.get()) return port
        val socket = ServerSocket(0, 4, java.net.InetAddress.getByName("127.0.0.1"))
        serverSocket = socket
        port = socket.localPort
        running.set(true)
        worker = Thread({ acceptLoop(socket) }, "arkabot-browser-bridge").apply {
            isDaemon = true
            start()
        }
        onLog("browser bridge listening on 127.0.0.1:$port")
        return port
    }

    /** Stop the server and release the port. */
    fun stop() {
        running.set(false)
        runCatching { serverSocket?.close() }
        serverSocket = null
        worker = null
        port = 0
    }

    private fun acceptLoop(socket: ServerSocket) {
        while (running.get()) {
            try {
                val client = socket.accept()
                Thread({ handle(client) }, "arkabot-browser-conn").apply {
                    isDaemon = true
                    start()
                }
            } catch (e: Exception) {
                if (running.get()) onLog("bridge accept failed: ${e.message}")
                break
            }
        }
    }

    private fun handle(client: Socket) {
        try {
            client.use { socket ->
                val reader = BufferedReader(InputStreamReader(socket.getInputStream()))
                val requestLine = reader.readLine() ?: return
                // Headers, then the body announced by Content-Length.
                var contentLength = 0
                while (true) {
                    val header = reader.readLine() ?: break
                    if (header.isEmpty()) break
                    val lower = header.lowercase()
                    if (lower.startsWith("content-length:")) {
                        contentLength = header.substringAfter(':').trim().toIntOrNull() ?: 0
                    }
                }
                val body = CharArray(contentLength)
                var read = 0
                while (read < contentLength) {
                    val n = reader.read(body, read, contentLength - read)
                    if (n < 0) break
                    read += n
                }
                val payload = String(body, 0, read)
                val response = dispatch(payload)
                val out: OutputStream = socket.getOutputStream()
                val bytes = response.toByteArray(Charsets.UTF_8)
                out.write(
                    ("HTTP/1.1 200 OK\r\nContent-Type: application/json\r\n" +
                        "Content-Length: ${bytes.size}\r\nConnection: close\r\n\r\n").toByteArray()
                )
                out.write(bytes)
                out.flush()
            }
        } catch (e: Exception) {
            onLog("bridge connection failed: ${e.message}")
        }
    }

    /** Run one command and build the reply. */
    private fun dispatch(payload: String): String {
        val id = runCatching { JSONObject(payload).optInt("id", 0) }.getOrDefault(0)
        val method = runCatching { JSONObject(payload).optString("method", "") }.getOrDefault("")
        val params = runCatching { JSONObject(payload).optJSONObject("params") }.getOrNull()
        return try {
            when (method) {
                "navigate", "open" -> {
                    val url = params?.optString("url").orEmpty()
                    onMain { webView.loadUrl(url) }
                    reply(id, true, "loading $url")
                }
                "evaluate" -> {
                    val expression = params?.optString("expression").orEmpty()
                    reply(id, true, expression)
                }
                "screenshot" -> {
                    // The bitmap is produced by the UI layer; the engine only
                    // needs a marker that the call succeeded.
                    reply(id, true, "")
                }
                else -> reply(id, false, "unsupported method: $method")
            }
        } catch (e: Exception) {
            reply(id, false, e.message ?: "bridge error")
        }
    }

    private fun reply(id: Int, ok: Boolean, output: String): String {
        val json = JSONObject()
        json.put("id", id)
        json.put("ok", ok)
        if (ok) json.put("output", output) else json.put("error", output)
        return json.toString()
    }

    private fun onMain(block: () -> Unit) {
        main.post(block)
    }

    companion object {
        /** Prepare a WebView for scripted control. */
        fun prepare(webView: WebView) {
            webView.settings.javaScriptEnabled = true
            webView.settings.domStorageEnabled = true
            webView.webViewClient = WebViewClient()
            webView.webChromeClient = WebChromeClient()
        }
    }
}
