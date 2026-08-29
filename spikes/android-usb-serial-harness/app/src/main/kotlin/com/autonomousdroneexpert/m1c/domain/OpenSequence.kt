package com.autonomousdroneexpert.m1c.domain

/** Result of an [OpenSequence.acquire]: either a live session, or the claim was rejected. */
sealed interface Acquired<out S> {
    data class Session<S>(val session: S) : Acquired<S>
    data object ClaimRejected : Acquired<Nothing>
}

/**
 * Pure, Android-independent resource-ownership guard so the "no leaked handle" property is
 * unit-testable on the JVM.
 *
 * The rule it enforces: a connection is opened by the caller BEFORE [acquire]. If the claim
 * is rejected, or [makeSession] fails/throws -- i.e. **any path that does not produce a
 * session** -- then [release] (only if the claim succeeded) and [close] are invoked so the
 * handle is never leaked. On success, ownership transfers to the session and neither
 * [release] nor [close] is called here (the session owns them). A thrown
 * `CancellationException` propagates untouched (cleanup still runs in `finally`).
 */
object OpenSequence {
    fun <S> acquire(
        claim: () -> Boolean,
        makeSession: () -> S,
        release: () -> Unit,
        close: () -> Unit,
    ): Acquired<S> {
        var claimed = false
        var ownershipTransferred = false
        try {
            claimed = claim()
            if (!claimed) return Acquired.ClaimRejected
            val session = makeSession()
            ownershipTransferred = true
            return Acquired.Session(session)
        } finally {
            if (!ownershipTransferred) {
                if (claimed) {
                    try {
                        release()
                    } catch (_: Throwable) {
                        // best-effort release; must still attempt close below.
                    }
                }
                try {
                    close()
                } catch (_: Throwable) {
                    // best-effort close; a failure here must not mask the original throwable.
                }
            }
        }
    }
}
