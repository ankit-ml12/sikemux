package com.nodelike.sikemux.notify

import org.json.JSONException
import org.json.JSONObject

/** The version of the card format this app reads. */
const val CARD_VERSION = 1L

/** Null when the host sealed a version this app does not know, or something that is not a card from `host`. */
fun parseCard(plaintext: String, host: String): NotificationCard? {
  val json = try {
    JSONObject(plaintext)
  } catch (_: JSONException) {
    return null
  }
  return NotificationCard.of(json)?.takeIf { it.v == CARD_VERSION && it.hostKey == host }
}

val NotificationCard.answerable: Boolean
  get() = kind == NotificationKind.PERMISSION && requestId != null && allowOptionId != null && rejectOptionId != null

/** How long the card has left to show at `now`, or null once its life is over. */
fun NotificationCard.timeLeft(now: Long): Long? = (expiresAt - now).takeIf { it > 0 }
