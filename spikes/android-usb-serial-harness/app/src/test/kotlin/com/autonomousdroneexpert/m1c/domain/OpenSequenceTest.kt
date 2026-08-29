package com.autonomousdroneexpert.m1c.domain

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * Pins the "no leaked handle" property: any path through the open sequence that does NOT
 * produce a session must close the (already-open) connection, and the success path must
 * transfer ownership without closing.
 */
class OpenSequenceTest {

    @Test
    fun `claim rejected closes the connection and creates no session`() {
        var released = 0
        var closed = 0
        val result = OpenSequence.acquire<String>(
            claim = { false },
            makeSession = { throw AssertionError("must not build a session when claim is rejected") },
            release = { released++ },
            close = { closed++ },
        )
        assertTrue(result is Acquired.ClaimRejected)
        assertEquals("never claimed, so never released", 0, released)
        assertEquals("connection must be closed", 1, closed)
    }

    @Test
    fun `success transfers ownership and does not release or close here`() {
        var released = 0
        var closed = 0
        val result = OpenSequence.acquire(
            claim = { true },
            makeSession = { "session" },
            release = { released++ },
            close = { closed++ },
        )
        assertTrue(result is Acquired.Session)
        assertEquals("session", (result as Acquired.Session).session)
        assertEquals("ownership transferred: no release here", 0, released)
        assertEquals("ownership transferred: no close here", 0, closed)
    }

    @Test
    fun `a failure after a successful claim but before the session releases and closes`() {
        var released = 0
        var closed = 0
        var thrown = false
        try {
            OpenSequence.acquire<String>(
                claim = { true },
                makeSession = { throw IllegalStateException("session ctor blew up") },
                release = { released++ },
                close = { closed++ },
            )
        } catch (_: IllegalStateException) {
            thrown = true
        }
        assertTrue("the original throwable propagates", thrown)
        assertEquals("claimed, so must release", 1, released)
        assertEquals("must also close", 1, closed)
    }

    @Test
    fun `claim throwing still closes the connection and does not release`() {
        var released = 0
        var closed = 0
        var thrown = false
        try {
            OpenSequence.acquire<String>(
                claim = { throw RuntimeException("claim blew up") },
                makeSession = { "s" },
                release = { released++ },
                close = { closed++ },
            )
        } catch (_: RuntimeException) {
            thrown = true
        }
        assertTrue(thrown)
        assertFalse("claim never succeeded, so no release", released > 0)
        assertEquals("connection must still be closed", 1, closed)
    }

    @Test
    fun `a release failure during cleanup does not prevent the close`() {
        var closed = 0
        var thrown = false
        try {
            OpenSequence.acquire<String>(
                claim = { true },
                makeSession = { throw IllegalStateException("boom") },
                release = { throw RuntimeException("release also fails") },
                close = { closed++ },
            )
        } catch (_: IllegalStateException) {
            thrown = true
        }
        assertTrue("the original throwable, not the release failure, propagates", thrown)
        assertEquals("close still attempted despite the release failure", 1, closed)
    }
}
