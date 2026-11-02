package com.fi.focus
import android.app.*
import android.content.Intent
import android.os.Bundle
import android.os.IBinder
class MainActivity : NativeActivity() {
 fun minimizeApp() { runOnUiThread { moveTaskToBack(true) } }
 override fun onCreate(savedInstanceState: Bundle?) { super.onCreate(savedInstanceState); startForegroundService(Intent(this, AudioService::class.java)) }
 override fun onDestroy() { stopService(Intent(this, AudioService::class.java)); super.onDestroy() }
}
class AudioService : Service() {
 override fun onCreate() {
  super.onCreate()
  val id = "fi-focus-audio"
  val manager = getSystemService(NotificationManager::class.java)
  manager.createNotificationChannel(NotificationChannel(id, "Pemutaran noise", NotificationManager.IMPORTANCE_LOW))
  val open = PendingIntent.getActivity(this, 0, Intent(this, MainActivity::class.java), PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT)
  val notification = Notification.Builder(this, id).setContentTitle("fi-focus").setContentText("fi-focus berjalan").setSmallIcon(android.R.drawable.ic_media_play).setContentIntent(open).setOngoing(true).build()
  startForeground(1, notification)
 }
 override fun onBind(intent: Intent?): IBinder? = null
}
