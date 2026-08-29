package com.autonomousdroneexpert.m1c.domain

/**
 * A tiny, dependency-free JSON encoder. Deliberately minimal: the report is small and we
 * avoid pulling in a serialization library/plugin for a spike. Escaping is unit-tested.
 */
object Json {
    val NULL: String = "null"

    fun str(value: String): String {
        val sb = StringBuilder("\"")
        for (c in value) {
            when (c) {
                '\\' -> sb.append("\\\\")
                '"' -> sb.append("\\\"")
                '\n' -> sb.append("\\n")
                '\r' -> sb.append("\\r")
                '\t' -> sb.append("\\t")
                else -> if (c < ' ') sb.append("\\u%04x".format(c.code)) else sb.append(c)
            }
        }
        return sb.append("\"").toString()
    }

    fun strOrNull(value: String?): String = value?.let { str(it) } ?: NULL
    fun num(value: Long): String = value.toString()
    fun numD(value: Double): String = value.toString()
    fun bool(value: Boolean): String = value.toString()

    fun obj(vararg entries: Pair<String, String>): String =
        entries.joinToString(",", "{", "}") { (k, v) -> "${str(k)}:$v" }

    fun arr(vararg items: String): String = items.joinToString(",", "[", "]")
}
